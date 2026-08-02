use tokio::sync::{mpsc, oneshot};

use crate::domain::{Order, OrderId, Sequence};

use super::{
    Book, BookError, CommandQueueError, EngineCommand, EngineFault, EngineReply, RejectionReport,
    SubmitOutcome,
};

const DEFAULT_COMMAND_QUEUE_CAPACITY: usize = 1_024;

#[derive(Debug)]
struct CommandEnvelope {
    command: EngineCommand,
    reply: oneshot::Sender<EngineReply>,
}

/// A shared Tokio MPSC command gateway for one instrument's single-writer engine.
///
/// Handlers may clone this value and enqueue concurrently. The matching book
/// and sequence are owned exclusively by the worker thread.
#[derive(Debug, Clone)]
pub struct Engine {
    command_sender: mpsc::Sender<CommandEnvelope>,
}

impl Engine {
    pub fn new(book_capacity: usize) -> Self {
        Self::new_with_queue_capacity(book_capacity, DEFAULT_COMMAND_QUEUE_CAPACITY)
    }

    /// Creates one bounded command queue and starts one matching worker.
    ///
    /// # Panics
    ///
    /// Panics when `queue_capacity` is zero or when called outside an active
    /// Tokio runtime.
    pub fn new_with_queue_capacity(book_capacity: usize, queue_capacity: usize) -> Self {
        assert!(
            queue_capacity > 0,
            "command queue capacity must be positive"
        );
        let (command_sender, command_receiver) = mpsc::channel(queue_capacity);
        start_worker(book_capacity, command_receiver);
        Self { command_sender }
    }

    /// Enqueues a command immediately and returns its async reply receiver.
    ///
    /// # Errors
    ///
    /// Returns an error when the queue is full, the worker is stopped, or a
    /// submitted order fails basic validation.
    pub fn enqueue(
        &self,
        command: EngineCommand,
    ) -> Result<oneshot::Receiver<EngineReply>, CommandQueueError> {
        command.validate_basic()?;
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.command_sender
            .try_send(CommandEnvelope {
                command,
                reply: reply_sender,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => CommandQueueError::Full,
                mpsc::error::TrySendError::Closed(_) => CommandQueueError::WorkerStopped,
            })?;
        Ok(reply_receiver)
    }

    /// Enqueues a command and asynchronously waits for its result.
    ///
    /// This is the method intended for async HTTP handlers.
    ///
    /// # Errors
    ///
    /// Returns an error when the queue is full, the worker is stopped, or
    /// basic command validation fails.
    pub async fn execute(&self, command: EngineCommand) -> Result<EngineReply, CommandQueueError> {
        let reply = self.enqueue(command)?.await;
        reply.map_err(|_| CommandQueueError::WorkerStopped)
    }

    /// Submits an order through the worker task.
    ///
    /// # Errors
    ///
    /// Returns an engine fault when the queue or worker is unavailable, or
    /// when the matching core encounters an internal fault.
    pub fn submit(&self, order: Order) -> Result<SubmitOutcome, EngineFault> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.submit_async(order))
        })
    }

    /// Submits an order without blocking the Tokio worker pool.
    ///
    /// # Errors
    ///
    /// Returns an engine fault when the queue or worker is unavailable, or
    /// when the matching core encounters an internal fault.
    pub async fn submit_async(&self, order: Order) -> Result<SubmitOutcome, EngineFault> {
        match self
            .execute_unchecked(EngineCommand::Submit(order))
            .await
            .map_err(EngineFault::from)?
        {
            EngineReply::Submit(result) => result,
            EngineReply::Cancel(_) => Err(EngineFault::Invariant(
                "submit received a cancellation reply",
            )),
        }
    }

    /// Cancels an order through the worker task.
    ///
    /// # Errors
    ///
    /// Returns an engine fault when the queue or worker is unavailable, or
    /// when the matching core encounters an internal fault.
    pub fn cancel(&self, order_id: OrderId) -> Result<Order, EngineFault> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.cancel_async(order_id))
        })
    }

    /// Cancels an order without blocking the Tokio worker pool.
    ///
    /// # Errors
    ///
    /// Returns an engine fault when the queue or worker is unavailable, or
    /// when the matching core encounters an internal fault.
    pub async fn cancel_async(&self, order_id: OrderId) -> Result<Order, EngineFault> {
        match self
            .execute_unchecked(EngineCommand::Cancel(order_id))
            .await
            .map_err(EngineFault::from)?
        {
            EngineReply::Cancel(result) => result,
            EngineReply::Submit(_) => {
                Err(EngineFault::Invariant("cancel received a submission reply"))
            }
        }
    }

    #[must_use]
    pub fn queued_commands(&self) -> usize {
        self.command_sender.max_capacity() - self.command_sender.capacity()
    }

    #[must_use]
    pub fn queue_capacity(&self) -> usize {
        self.command_sender.max_capacity()
    }

    async fn execute_unchecked(
        &self,
        command: EngineCommand,
    ) -> Result<EngineReply, CommandQueueError> {
        let reply = self.enqueue_unchecked(command)?.await;
        reply.map_err(|_| CommandQueueError::WorkerStopped)
    }

    fn enqueue_unchecked(
        &self,
        command: EngineCommand,
    ) -> Result<oneshot::Receiver<EngineReply>, CommandQueueError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.command_sender
            .try_send(CommandEnvelope {
                command,
                reply: reply_sender,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => CommandQueueError::Full,
                mpsc::error::TrySendError::Closed(_) => CommandQueueError::WorkerStopped,
            })?;
        Ok(reply_receiver)
    }
}

fn start_worker(book_capacity: usize, mut command_receiver: mpsc::Receiver<CommandEnvelope>) {
    tokio::spawn(async move {
        let mut core = EngineCore::new(book_capacity);
        while let Some(envelope) = command_receiver.recv().await {
            core.process(envelope);
        }
    });
}

#[derive(Debug)]
struct EngineCore {
    book: Book,
    sequence: Sequence,
}

impl EngineCore {
    fn new(book_capacity: usize) -> Self {
        Self {
            book: Book::new(book_capacity),
            sequence: Sequence::from(0),
        }
    }

    fn process(&mut self, envelope: CommandEnvelope) {
        let reply = match envelope.command {
            EngineCommand::Submit(order) => EngineReply::Submit(self.submit(order)),
            EngineCommand::Cancel(order_id) => EngineReply::Cancel(self.cancel(order_id)),
        };
        let _ = envelope.reply.send(reply);
    }

    fn submit(&mut self, order: Order) -> Result<SubmitOutcome, EngineFault> {
        if let Err(error) = self.validate_submission(&order) {
            return Ok(SubmitOutcome::Rejected(
                RejectionReport::from_admission_error(order.resting.id, &error)?,
            ));
        }

        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EngineFault::SequenceExhausted)?;

        match self.book.submit(order, next_sequence) {
            Ok(report) => {
                self.sequence = next_sequence;
                Ok(SubmitOutcome::Accepted(report))
            }
            Err(error) => Err(EngineFault::from_post_admission(error)),
        }
    }

    fn cancel(&mut self, order_id: OrderId) -> Result<Order, EngineFault> {
        let next_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(EngineFault::SequenceExhausted)?;
        let cancelled = self
            .book
            .cancel(order_id)
            .map_err(EngineFault::from_cancel_error)?;
        self.sequence = next_sequence;
        Ok(cancelled)
    }

    fn validate_submission(&self, order: &Order) -> Result<(), BookError> {
        order.validate()?;
        self.book.validate_order_constraints(order)?;
        self.book.validate_order_id_available(order.resting.id)?;
        self.book.validate_resting_capacity(order)
    }
}

impl From<CommandQueueError> for EngineFault {
    fn from(error: CommandQueueError) -> Self {
        match error {
            CommandQueueError::Full => Self::QueueFull,
            CommandQueueError::WorkerStopped => Self::WorkerStopped,
            CommandQueueError::InvalidOrder(_) => {
                Self::Invariant("synchronous command submission received an invalid order error")
            }
        }
    }
}
