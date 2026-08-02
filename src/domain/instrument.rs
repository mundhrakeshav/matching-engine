use std::fmt;

use serde::{Deserialize, Serialize};

use super::{LotSize, Quantity, TickSize};

/// Unique identifier for an instrument.
///
/// Chosen as `u64` to match the monotonic sequence space used for `OrderId`
/// and `Sequence`, keeping the address space uniform across the matching
/// core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentId(pub u64);

impl From<u64> for InstrumentId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<InstrumentId> for u64 {
    fn from(value: InstrumentId) -> Self {
        value.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "INST-{}", self.0)
    }
}

#[derive(Debug)]
pub struct Instrument {
    pub id: InstrumentId,
    pub max_order_quantity: Quantity,
    pub name: String,
    pub ticker: String,
    pub tick_size: TickSize,
    pub lot_size: LotSize,
    pub book_cap: u64,
}
