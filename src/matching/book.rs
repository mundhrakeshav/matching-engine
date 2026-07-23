use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use thiserror::Error;

use crate::domain::{
    Order, OrderError, OrderId, OrderKind, OrderSide, Price, Quantity, Sequence, Trade,
};

use super::arena::{Arena, ArenaError, NodeId};

#[derive(Debug)]
struct OrderNode {
    order: Order,
    price: Price,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

#[derive(Debug, Default)]
struct PriceLevel {
    quantity: Quantity,
    head: Option<NodeId>,
    tail: Option<NodeId>,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelSnapshot {
    pub price: Price,
    pub quantity: Quantity,
    pub order_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderBookSnapshot {
    pub sequence: Sequence,
    pub bids: Vec<LevelSnapshot>,
    pub asks: Vec<LevelSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionReport {
    pub order_id: OrderId,
    pub remaining_quantity: Quantity,
    pub trades: Vec<Trade>,
}

#[derive(Debug, Error)]
pub enum BookError {
    #[error(transparent)]
    InvalidOrder(#[from] OrderError),
    #[error("duplicate active order ID: {0}")]
    DuplicateOrder(OrderId),
    #[error("order not found: {0}")]
    OrderNotFound(OrderId),
    #[error("matching storage failure")]
    Arena,
    #[error("order book invariant violated: {0}")]
    Invariant(&'static str),
    #[error("engine sequence exhausted")]
    SequenceExhausted,
}

impl From<ArenaError> for BookError {
    fn from(_: ArenaError) -> Self {
        Self::Arena
    }
}

/// Canonical, single-instrument state. Methods require `&mut self`, enforcing a
/// single writer without internal locking or timing-dependent behavior.
#[derive(Debug)]
pub struct Book {
    capacity: usize,
    arena: Arena<OrderNode>,
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
    locations: HashMap<OrderId, NodeId>,
}

impl Book {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            arena: Arena::with_capacity(capacity),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            locations: HashMap::new(),
        }
    }

    /// Matches an order and rests any eligible unfilled limit remainder.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or duplicate orders, exhausted storage, or
    /// an internal invariant violation.
    pub fn submit(
        &mut self,
        mut taker: Order,
        sequence: Sequence,
    ) -> Result<ExecutionReport, BookError> {
        taker.validate()?;
        if self.locations.contains_key(&taker.resting.id) {
            return Err(BookError::DuplicateOrder(taker.resting.id));
        }

        let mut trades = Vec::new();
        while taker.resting.open_qty > 0 {
            let Some(price) = self.best_opposite_price(taker.side) else {
                break;
            };
            if taker.kind == OrderKind::Limit && !taker.crosses(price) {
                break;
            }
            let maker_id = self
                .level(taker.side, price)?
                .head
                .ok_or(BookError::Invariant("non-empty level has no head"))?;
            let (maker_order, maker_price, maker_quantity) = {
                let maker = self.arena.get(maker_id)?;
                (
                    maker.order.clone(),
                    maker.price,
                    maker.order.resting.open_qty,
                )
            };
            let quantity = taker.resting.open_qty.min(maker_quantity);
            let trade = Trade {
                sequence,
                maker_order_id: maker_order.resting.id,
                taker_order_id: taker.resting.id,
                maker_id: maker_order.resting.user_id,
                taker_id: taker.resting.user_id,
                taker_side: taker.side,
                quantity,
                price: maker_price,
            };
            taker.resting.open_qty -= quantity;
            self.apply_maker_fill(taker.side, maker_id, price, quantity)?;
            trades.push(trade);
        }

        if taker.resting.open_qty > 0 && taker.kind == OrderKind::Limit {
            self.rest(taker.clone())?;
        }
        self.check_invariants()?;
        Ok(ExecutionReport {
            order_id: taker.resting.id,
            remaining_quantity: taker.resting.open_qty,
            trades,
        })
    }

    /// Removes a resting order from its FIFO level.
    ///
    /// # Errors
    ///
    /// Returns an error when the order is not active or book invariants fail.
    pub fn cancel(&mut self, order_id: OrderId) -> Result<(), BookError> {
        let node_id = self
            .locations
            .get(&order_id)
            .copied()
            .ok_or(BookError::OrderNotFound(order_id))?;
        self.remove_resting(node_id)?;
        self.check_invariants()
    }

    pub fn snapshot(&self, sequence: Sequence) -> OrderBookSnapshot {
        let bids = self
            .bids
            .iter()
            .rev()
            .map(|(&price, level)| LevelSnapshot {
                price,
                quantity: level.quantity,
                order_count: level.count,
            })
            .collect();
        let asks = self
            .asks
            .iter()
            .map(|(&price, level)| LevelSnapshot {
                price,
                quantity: level.quantity,
                order_count: level.count,
            })
            .collect();
        OrderBookSnapshot {
            sequence,
            bids,
            asks,
        }
    }

    fn rest(&mut self, order: Order) -> Result<(), BookError> {
        let price = order
            .limit_price
            .ok_or(BookError::Invariant("limit order has no price"))?;
        let side = order.side;
        let order_id = order.resting.id;
        let quantity = order.resting.open_qty;
        let node_id = self.arena.allocate(
            OrderNode {
                order,
                price,
                prev: None,
                next: None,
            },
            self.capacity,
        )?;
        let tail = self.levels(side).get(&price).and_then(|level| level.tail);
        if let Some(tail) = tail {
            self.arena.get_mut(tail)?.next = Some(node_id);
            self.arena.get_mut(node_id)?.prev = Some(tail);
        }
        let level = self.level_mut(side, price);
        if tail.is_none() {
            level.head = Some(node_id);
        }
        level.tail = Some(node_id);
        level.count += 1;
        level.quantity += quantity;
        self.locations.insert(order_id, node_id);
        Ok(())
    }

    fn apply_maker_fill(
        &mut self,
        taker_side: OrderSide,
        node_id: NodeId,
        price: Price,
        quantity: Quantity,
    ) -> Result<(), BookError> {
        {
            let maker = self.arena.get_mut(node_id)?;
            maker.order.resting.open_qty -= quantity;
        }
        let fully_filled = self.arena.get(node_id)?.order.resting.open_qty == 0;
        let maker_side = match taker_side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };
        self.level_mut(maker_side, price).quantity -= quantity;
        if fully_filled {
            self.remove_resting(node_id)?;
        }
        Ok(())
    }

    fn remove_resting(&mut self, node_id: NodeId) -> Result<(), BookError> {
        let (order_id, side, price, previous, next) = {
            let node = self.arena.get(node_id)?;
            (
                node.order.resting.id,
                node.order.side,
                node.price,
                node.prev,
                node.next,
            )
        };
        let remaining = self.arena.get(node_id)?.order.resting.open_qty;
        if let Some(previous) = previous {
            self.arena.get_mut(previous)?.next = next;
        }
        if let Some(next) = next {
            self.arena.get_mut(next)?.prev = previous;
        }
        let level = self.level_mut(side, price);
        if level.head == Some(node_id) {
            level.head = next;
        }
        if level.tail == Some(node_id) {
            level.tail = previous;
        }
        level.count = level
            .count
            .checked_sub(1)
            .ok_or(BookError::Invariant("level count underflow"))?;
        level.quantity = level
            .quantity
            .checked_sub(remaining)
            .ok_or(BookError::Invariant("level quantity underflow"))?;
        let empty = level.count == 0;
        if empty {
            self.levels_mut(side).remove(&price);
        }
        self.locations.remove(&order_id);
        self.arena.release(node_id)?;
        Ok(())
    }

    fn best_opposite_price(&self, side: OrderSide) -> Option<Price> {
        match side {
            OrderSide::Buy => self.asks.first_key_value().map(|(&price, _)| price),
            OrderSide::Sell => self.bids.last_key_value().map(|(&price, _)| price),
        }
    }

    fn levels(&self, side: OrderSide) -> &BTreeMap<Price, PriceLevel> {
        match side {
            OrderSide::Buy => &self.bids,
            OrderSide::Sell => &self.asks,
        }
    }

    fn levels_mut(&mut self, side: OrderSide) -> &mut BTreeMap<Price, PriceLevel> {
        match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        }
    }

    fn level(&self, taker_side: OrderSide, price: Price) -> Result<&PriceLevel, BookError> {
        let maker_side = match taker_side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };
        self.levels(maker_side)
            .get(&price)
            .ok_or(BookError::Invariant("best level disappeared"))
    }

    fn level_mut(&mut self, side: OrderSide, price: Price) -> &mut PriceLevel {
        self.levels_mut(side).entry(price).or_default()
    }

    fn check_invariants(&self) -> Result<(), BookError> {
        let mut seen = HashSet::new();
        for (side, levels) in [(OrderSide::Buy, &self.bids), (OrderSide::Sell, &self.asks)] {
            for (&price, level) in levels {
                if (level.count == 0) != (level.head.is_none() && level.tail.is_none()) {
                    return Err(BookError::Invariant("empty level links are inconsistent"));
                }
                let mut total = 0;
                let mut count = 0;
                let mut previous = None;
                let mut current = level.head;
                while let Some(id) = current {
                    if !seen.insert(id) {
                        return Err(BookError::Invariant("node is duplicated or cyclic"));
                    }
                    let node = self.arena.get(id)?;
                    if node.order.side != side || node.price != price || node.prev != previous {
                        return Err(BookError::Invariant("node does not belong to its level"));
                    }
                    if self.locations.get(&node.order.resting.id) != Some(&id) {
                        return Err(BookError::Invariant("location is missing or stale"));
                    }
                    total += node.order.resting.open_qty;
                    count += 1;
                    previous = Some(id);
                    current = node.next;
                }
                if total != level.quantity || count != level.count || previous != level.tail {
                    return Err(BookError::Invariant("level aggregate or tail is incorrect"));
                }
            }
        }
        if seen.len() != self.locations.len() {
            return Err(BookError::Invariant("orphaned order location"));
        }
        Ok(())
    }
}
