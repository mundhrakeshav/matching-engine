use std::collections::HashMap;

use thiserror::Error;

use crate::domain::{Instrument, InstrumentId};

#[derive(Debug, Error)]
pub enum InstrumentRegistryError {
    #[error("instrument {0} already registered")]
    DuplicateInstrument(InstrumentId),

    #[error("instrument {0} not found")]
    NotFound(InstrumentId),
}

/// Registry of instruments known to the matching core.
///
/// Every order submitted to the engine must reference a registered
/// instrument. The registry is intentionally minimal — it only tracks
/// which instruments exist and provides a lookup by ID.
#[derive(Debug)]
pub struct InstrumentRegistry {
    registry: HashMap<InstrumentId, Instrument>,
}

impl Default for InstrumentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InstrumentRegistry {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    /// Register a new instrument.
    ///
    /// # Errors
    ///
    /// Returns [`InstrumentRegistryError::DuplicateInstrument`] if an
    /// instrument with the same ID is already registered.
    pub fn register(&mut self, instrument: Instrument) -> Result<(), InstrumentRegistryError> {
        if self.registry.contains_key(&instrument.id) {
            return Err(InstrumentRegistryError::DuplicateInstrument(instrument.id));
        }
        self.registry.insert(instrument.id, instrument);
        Ok(())
    }

    /// Look up an instrument by ID.
    ///
    /// # Errors
    ///
    /// Returns [`InstrumentRegistryError::NotFound`] if the instrument
    /// is not registered.
    pub fn get(&self, id: InstrumentId) -> Result<&Instrument, InstrumentRegistryError> {
        self.registry
            .get(&id)
            .ok_or(InstrumentRegistryError::NotFound(id))
    }

    /// Returns `true` if the instrument is registered.
    pub fn contains(&self, id: InstrumentId) -> bool {
        self.registry.contains_key(&id)
    }

    /// Returns the number of registered instruments.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Returns `true` if no instruments are registered.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }
}
