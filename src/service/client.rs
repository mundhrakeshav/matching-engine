use std::{collections::HashMap, fmt, sync::Arc, time::Duration};

use disruptor::{
    MultiProducer, Producer, RingBufferFull, SingleConsumerBarrier, Sleep, build_multi_producer,
};
use parking_lot::Mutex;
use thiserror::Error;
use tokio::sync::oneshot;

use crate::{
    domain::{Order, OrderId, Symbol},
    matching::{
        BookError, BookSnapshot, CancelReport, Engine, Exchange, ExecutionReport, TopOfBook,
    },
};

use super::ServiceError;

const MINIMUM_RING_CAPACITY: usize = 64;
const ENGINE_WAIT: Duration = Duration::from_millis(1);

type CommandProducer = MultiProducer<CommandSlot, SingleConsumerBarrier>;

/// Requested book read depth.
#[derive(Debug, Clone, Copy)]
pub enum BookDepth {
    Top,
    Levels(usize),
    Full,
}

/// A book read result, shaped by the requested [`BookDepth`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookView {
    Top(TopOfBook),
    Snapshot(BookSnapshot),
}

/// Failure returned by [`spawn_exchange`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpawnError {
    #[error("ring capacity must be a power of two and at least {MINIMUM_RING_CAPACITY}")]
    InvalidRingCapacity,
}

/// Routes requests to the preconfigured, independently owned symbol engines.
///
/// This is the *only* type outside `crate::matching` permitted to hold an
/// [`Engine`] or [`Exchange`]. HTTP handlers (and any future transport, such
/// as a backtest runner) call the methods below and never see the engine
/// directly.
#[derive(Clone)]
pub struct ExchangeClient {
    engines: Arc<HashMap<Symbol, EngineClient>>,
}

impl fmt::Debug for ExchangeClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExchangeClient")
            .field("symbols", &self.engines.len())
            .finish_non_exhaustive()
    }
}

/// Publishes commands into one symbol's multi-producer Disruptor ring.
struct EngineClient {
    producer: CommandProducer,
}

impl Clone for EngineClient {
    fn clone(&self) -> Self {
        Self {
            producer: self.producer.clone(),
        }
    }
}

impl fmt::Debug for EngineClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EngineClient(..)")
    }
}

#[derive(Debug, Default)]
struct CommandSlot {
    command: Mutex<Option<EngineCommand>>,
}

#[derive(Debug)]
enum EngineCommand {
    Book {
        depth: BookDepth,
        reply: oneshot::Sender<BookView>,
    },
    Submit {
        order: Order,
        reply: oneshot::Sender<Result<ExecutionReport, BookError>>,
    },
    Cancel {
        order_id: OrderId,
        reply: oneshot::Sender<Result<CancelReport, BookError>>,
    },
}

#[derive(Debug)]
struct EngineState {
    engine: Engine,
}

impl EngineState {
    fn process(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Book { depth, reply } => {
                let _ = reply.send(self.book_view(depth));
            }
            EngineCommand::Submit { order, reply } => {
                let _ = reply.send(self.engine.submit(order));
            }
            EngineCommand::Cancel { order_id, reply } => {
                let _ = reply.send(self.engine.cancel(order_id));
            }
        }
    }

    fn book_view(&self, depth: BookDepth) -> BookView {
        match depth {
            BookDepth::Top => BookView::Top(self.engine.top_of_book()),
            BookDepth::Levels(levels) => BookView::Snapshot(self.engine.depth(levels)),
            BookDepth::Full => BookView::Snapshot(self.engine.snapshot()),
        }
    }
}

/// Starts one multi-producer command ring and matching consumer per configured symbol.
///
/// # Errors
///
/// Returns [`SpawnError::InvalidRingCapacity`] when `ring_capacity` is smaller
/// than 64 or is not a power of two.
pub fn spawn_exchange(
    exchange: Exchange,
    ring_capacity: usize,
) -> Result<ExchangeClient, SpawnError> {
    validate_ring_capacity(ring_capacity)?;
    let engines = exchange
        .into_books()
        .into_iter()
        .map(|(symbol, engine)| (symbol, spawn_engine(engine, ring_capacity)))
        .collect();
    Ok(ExchangeClient {
        engines: Arc::new(engines),
    })
}

fn spawn_engine(engine: Engine, ring_capacity: usize) -> EngineClient {
    let producer =
        build_multi_producer(ring_capacity, CommandSlot::default, Sleep::new(ENGINE_WAIT))
            .handle_events_and_state_with(
                |state: &mut EngineState, slot: &CommandSlot, _, _| {
                    let command = slot.command.lock().take();
                    if let Some(command) = command {
                        state.process(command);
                    }
                },
                move || EngineState { engine },
            )
            .build();
    EngineClient { producer }
}

impl ExchangeClient {
    fn engine(&self, symbol: Symbol) -> Result<&EngineClient, ServiceError> {
        self.engines
            .get(&symbol)
            .ok_or(ServiceError::UnknownSymbol(symbol))
    }

    /// Reads the book for `symbol` at the requested depth.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError::UnknownSymbol`] when `symbol` is not
    /// configured, [`ServiceError::Overloaded`] when the command ring is
    /// full, or [`ServiceError::Unavailable`] when the engine consumer has
    /// stopped.
    pub async fn book(&self, symbol: Symbol, depth: BookDepth) -> Result<BookView, ServiceError> {
        let (reply, response) = oneshot::channel();
        self.engine(symbol)?
            .publish(EngineCommand::Book { depth, reply })?;
        response.await.map_err(|_| ServiceError::Unavailable)
    }

    /// Submits `order` to `symbol`'s engine.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError::UnknownSymbol`], [`ServiceError::Overloaded`],
    /// [`ServiceError::Unavailable`], or [`ServiceError::Book`] when the
    /// engine rejects the order.
    pub async fn submit(
        &self,
        symbol: Symbol,
        order: Order,
    ) -> Result<ExecutionReport, ServiceError> {
        let (reply, response) = oneshot::channel();
        self.engine(symbol)?
            .publish(EngineCommand::Submit { order, reply })?;
        Ok(response.await.map_err(|_| ServiceError::Unavailable)??)
    }

    /// Cancels `order_id` on `symbol`'s engine.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError::UnknownSymbol`], [`ServiceError::Overloaded`],
    /// [`ServiceError::Unavailable`], or [`ServiceError::Book`] when the
    /// order is not active.
    pub async fn cancel(
        &self,
        symbol: Symbol,
        order_id: OrderId,
    ) -> Result<CancelReport, ServiceError> {
        let (reply, response) = oneshot::channel();
        self.engine(symbol)?
            .publish(EngineCommand::Cancel { order_id, reply })?;
        Ok(response.await.map_err(|_| ServiceError::Unavailable)??)
    }
}

impl EngineClient {
    fn publish(&self, command: EngineCommand) -> Result<(), ServiceError> {
        let mut producer = self.producer.clone();
        producer
            .try_publish(|slot| {
                *slot.command.lock() = Some(command);
            })
            .map(|_| ())
            .map_err(|RingBufferFull| ServiceError::Overloaded)
    }
}

fn validate_ring_capacity(capacity: usize) -> Result<(), SpawnError> {
    if capacity < MINIMUM_RING_CAPACITY || !capacity.is_power_of_two() {
        return Err(SpawnError::InvalidRingCapacity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, UserId};

    fn limit(id: u64, side: OrderSide) -> Order {
        Order {
            resting: RestingOrder {
                id: OrderId::from(id),
                user_id: UserId::from(id),
                original_qty: Quantity::from(1),
                open_qty: Quantity::from(1),
                accepted_sequence: Sequence::from(0),
            },
            limit_price: Some(Price::from(100)),
            kind: OrderKind::Limit,
            side,
        }
    }

    #[tokio::test]
    async fn symbol_rings_are_isolated() {
        let btc = Symbol::parse("BTC-USD").unwrap();
        let eth = Symbol::parse("ETH-USD").unwrap();
        let mut exchange = Exchange::new(2);
        exchange.prepare_symbols([btc, eth]);
        let client = spawn_exchange(exchange, 64).unwrap();

        client.submit(btc, limit(1, OrderSide::Sell)).await.unwrap();

        let report = client.submit(eth, limit(1, OrderSide::Buy)).await.unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(report.remaining_quantity, Quantity::from(1));
    }

    #[tokio::test]
    async fn unknown_symbol_is_rejected() {
        let client = spawn_exchange(Exchange::new(2), 64).unwrap();
        let error = client
            .submit(Symbol::parse("BTC-USD").unwrap(), limit(1, OrderSide::Buy))
            .await
            .unwrap_err();
        assert!(matches!(error, ServiceError::UnknownSymbol(_)));
    }

    #[test]
    fn full_ring_rejects_publication() {
        use std::sync::Barrier;

        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let consumer_entered = Arc::clone(&entered);
        let consumer_release = Arc::clone(&release);
        let mut pause_first_event = true;
        let mut producer = build_multi_producer(64, || (), Sleep::new(ENGINE_WAIT))
            .handle_events_with(move |(), _, _| {
                if pause_first_event {
                    pause_first_event = false;
                    consumer_entered.wait();
                    consumer_release.wait();
                }
            })
            .build();

        producer.try_publish(|()| {}).unwrap();
        entered.wait();
        for _ in 1..64 {
            producer.try_publish(|()| {}).unwrap();
        }
        assert!(matches!(producer.try_publish(|()| {}), Err(RingBufferFull)));

        release.wait();
    }

    #[test]
    fn invalid_ring_capacity_is_rejected() {
        assert_eq!(
            validate_ring_capacity(0),
            Err(SpawnError::InvalidRingCapacity)
        );
        assert_eq!(
            validate_ring_capacity(65),
            Err(SpawnError::InvalidRingCapacity)
        );
        assert_eq!(validate_ring_capacity(64), Ok(()));
    }
}
