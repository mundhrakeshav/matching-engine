use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct NodeId(usize);

impl NodeId {
    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub(super) struct Arena<T> {
    slots: Vec<Option<T>>,
    free: Vec<NodeId>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ArenaError {
    #[error("order arena capacity exhausted")]
    Exhausted,
    #[error("invalid or released arena node")]
    InvalidNode,
}

impl<T> Arena<T> {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free: Vec::new(),
        }
    }

    pub(super) fn allocate(&mut self, value: T, capacity: usize) -> Result<NodeId, ArenaError> {
        if let Some(id) = self.free.pop() {
            self.slots[id.index()] = Some(value);
            return Ok(id);
        }
        if self.slots.len() == capacity {
            return Err(ArenaError::Exhausted);
        }
        let id = NodeId(self.slots.len());
        self.slots.push(Some(value));
        Ok(id)
    }

    pub(super) fn get(&self, id: NodeId) -> Result<&T, ArenaError> {
        self.slots
            .get(id.index())
            .and_then(Option::as_ref)
            .ok_or(ArenaError::InvalidNode)
    }

    pub(super) fn get_mut(&mut self, id: NodeId) -> Result<&mut T, ArenaError> {
        self.slots
            .get_mut(id.index())
            .and_then(Option::as_mut)
            .ok_or(ArenaError::InvalidNode)
    }

    pub(super) fn release(&mut self, id: NodeId) -> Result<T, ArenaError> {
        let value = self
            .slots
            .get_mut(id.index())
            .and_then(Option::take)
            .ok_or(ArenaError::InvalidNode)?;
        self.free.push(id);
        Ok(value)
    }
}
