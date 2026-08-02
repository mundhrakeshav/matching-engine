use thiserror::Error;

use crate::domain::{Order, OrderError, OrderId};

use super::{EngineFault, SubmitOutcome};

/// A command processed by the single engine writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCommand {
    Submit(Order),
    Cancel(OrderId),
}

/// The result produced after processing an [`EngineCommand`].
#[derive(Debug, PartialEq, Eq)]
pub enum EngineReply {
    Submit(Result<SubmitOutcome, EngineFault>),
    Cancel(Result<Order, EngineFault>),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandQueueError {
    #[error("engine command queue is full")]
    Full,
    #[error("engine worker has stopped")]
    WorkerStopped,
    #[error("invalid order: {0}")]
    InvalidOrder(#[from] OrderError),
}

impl EngineCommand {
    pub(super) fn validate_basic(&self) -> Result<(), CommandQueueError> {
        match self {
            Self::Submit(order) => order.validate().map_err(CommandQueueError::from),
            Self::Cancel(_) => Ok(()),
        }
    }
}
