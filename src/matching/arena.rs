use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct NodeId(usize);

impl NodeId {
    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct Arena<T> {
    slots: Vec<Option<T>>,
    free: Vec<NodeId>,
    capacity: usize,
    live_count: usize,
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
            // TODO: Eventually allocate some space to free too
            free: Vec::new(),
            capacity,
            live_count: 0,
        }
    }

    pub(super) fn allocate(&mut self, value: T) -> Result<NodeId, ArenaError> {
        if self.live_count >= self.capacity {
            return Err(ArenaError::Exhausted);
        }

        if let Some(&id) = self.free.last() {
            let slot = self
                .slots
                .get_mut(id.index())
                .ok_or(ArenaError::InvalidNode)?;
            if slot.is_some() {
                return Err(ArenaError::InvalidNode);
            }
            *slot = Some(value);
            self.free.pop();
            self.live_count += 1;
            return Ok(id);
        }

        let id = NodeId(self.slots.len());
        self.slots.push(Some(value));
        self.live_count += 1;
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

    #[cfg(test)]
    pub(super) fn live_count(&self) -> usize {
        self.live_count
    }

    pub(super) fn has_capacity(&self) -> bool {
        self.live_count < self.capacity
    }

    pub(super) fn release(&mut self, id: NodeId) -> Result<T, ArenaError> {
        if self.live_count == 0 {
            return Err(ArenaError::InvalidNode);
        }

        let value = self
            .slots
            .get_mut(id.index())
            .and_then(Option::take)
            .ok_or(ArenaError::InvalidNode)?;
        self.live_count -= 1;
        self.free.push(id);
        Ok(value)
    }
}
