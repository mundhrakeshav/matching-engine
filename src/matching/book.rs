use std::collections::{BTreeMap, HashMap};

use thiserror::Error;

use crate::domain::{
    Order, OrderError, OrderId, OrderKind, OrderSide, Price, Quantity, Sequence, Trade,
};

use super::arena::{Arena, ArenaError, NodeId};

#[derive(Debug, Clone)]
struct OrderNode {
    order: Order,
    price: Price,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

#[derive(Debug, Clone, Default)]
struct PriceLevel {
    quantity: Quantity,
    head: Option<NodeId>,
    tail: Option<NodeId>,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExecutionReport {
    pub order_id: OrderId,
    pub remaining_quantity: Quantity,
    pub trades: Vec<Trade>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BookError {
    #[error(transparent)]
    InvalidOrder(#[from] OrderError),
    #[error("duplicate active order ID: {0}")]
    DuplicateOrder(OrderId),
    #[error("order book has no room to rest the unfilled remainder")]
    Full,
    #[error("aggregate quantity exceeds the supported range")]
    QuantityOverflow,
    #[error("order book invariant violated: {0}")]
    Invariant(&'static str),
    #[error("engine sequence exhausted")]
    SequenceExhausted,
}

/// A single planned trade between the incoming taker and a resting maker,
/// computed by [`Book::plan`] against a read-only view of the book.
#[derive(Debug, Clone, Copy)]
struct PlannedFill {
    maker_node_id: NodeId,
    price: Price,
    quantity: Quantity,
}

/// Canonical, single-instrument state. Methods require `&mut self`, enforcing a
/// single writer without internal locking or timing-dependent behavior.
#[derive(Debug, Clone)]
pub struct Book {
    arena: Arena<OrderNode>,
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
    locations: HashMap<OrderId, NodeId>,
}

impl Book {
    pub fn new(capacity: usize) -> Self {
        Self {
            arena: Arena::with_capacity(capacity),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            locations: HashMap::new(),
        }
    }

    /// Matches an order and rests any eligible unfilled limit remainder.
    ///
    /// `submit` owns the matching loop. Each iteration computes one read-only
    /// fill plan, validates it against current state, and applies only that
    /// fill. An eligible limit remainder is rested after matching stops.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or duplicate orders, an exhausted engine
    /// sequence, or a book with no room to rest the unfilled remainder.
    pub fn submit(
        &mut self,
        mut order: Order,
        sequence: Sequence,
    ) -> Result<ExecutionReport, BookError> {
        order.validate()?;

        if self.locations.contains_key(&order.resting.id) {
            return Err(BookError::DuplicateOrder(order.resting.id));
        }

        // Allocate 5 once to void re allocating if more needed
        let mut trades = Vec::with_capacity(5);

        while order.resting.open_qty > Quantity::from(0) {
            let Some(fill) = self.plan(&order)? else {
                break;
            };
            self.validate(&order, &fill)?;
            trades.push(self.apply(&mut order, fill, sequence)?);
        }

        if order.resting.open_qty > Quantity::from(0) && order.kind == OrderKind::Limit {
            self.rest(order.clone())?;
        }

        Ok(ExecutionReport {
            order_id: order.resting.id,
            remaining_quantity: order.resting.open_qty,
            trades,
        })
    }

    /// Returns the next fill without mutating the book or taker.
    fn plan(&self, taker_order: &Order) -> Result<Option<PlannedFill>, BookError> {
        let best = match taker_order.side {
            OrderSide::Buy => self.asks.first_key_value(),
            OrderSide::Sell => self.bids.last_key_value(),
        };

        // Return if no price level
        let Some((&price, level)) = best else {
            return Ok(None);
        };

        // Wen limit order and not crossing then return
        if taker_order.kind == OrderKind::Limit && !taker_order.crosses(price) {
            return Ok(None);
        }

        let maker_node_id = level
            .head
            .ok_or(BookError::Invariant("active price level has no head"))?;

        let maker_order = self
            .arena
            .get(maker_node_id)
            .map_err(|_| BookError::Invariant("price level head was released"))?;

        Ok(Some(PlannedFill {
            maker_node_id,
            price,
            quantity: taker_order
                .resting
                .open_qty
                .min(maker_order.order.resting.open_qty),
        }))
    }

    /// Validates one planned fill against the current book and taker.
    fn validate(&self, taker_order: &Order, fill: &PlannedFill) -> Result<(), BookError> {
        if fill.quantity == Quantity::from(0) {
            return Err(BookError::Invariant("planned fill has zero quantity"));
        }
        if taker_order.resting.open_qty < fill.quantity {
            return Err(BookError::Invariant(
                "planned fill exceeds taker open quantity",
            ));
        }

        let maker_side = match taker_order.side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        let maker_order = self
            .arena
            .get(fill.maker_node_id)
            .map_err(|_| BookError::Invariant("planned maker node was released"))?;

        if maker_order.order.side != maker_side || maker_order.price != fill.price {
            return Err(BookError::Invariant(
                "planned maker does not belong to its price level",
            ));
        }
        if self.locations.get(&maker_order.order.resting.id) != Some(&fill.maker_node_id) {
            return Err(BookError::Invariant(
                "planned maker location does not reference its node",
            ));
        }
        if maker_order.order.resting.open_qty < fill.quantity {
            return Err(BookError::Invariant(
                "planned fill exceeds maker open quantity",
            ));
        }

        let level = self
            .levels(maker_side)
            .get(&fill.price)
            .ok_or(BookError::Invariant("planned maker price level is missing"))?;

        level
            .quantity
            .checked_sub(fill.quantity)
            .ok_or(BookError::Invariant(
                "planned fill exceeds aggregate level quantity",
            ))?;

        if maker_order.order.resting.open_qty == fill.quantity {
            self.validate_removal_links(
                fill.maker_node_id,
                maker_order.prev,
                maker_order.next,
                level,
            )?;
        }
        Ok(())
    }

    /// Applies exactly one validated fill.
    fn apply(
        &mut self,
        taker: &mut Order,
        fill: PlannedFill,
        sequence: Sequence,
    ) -> Result<Trade, BookError> {
        let maker = self
            .arena
            .get(fill.maker_node_id)
            .map_err(|_| BookError::Invariant("planned maker node was released"))?;

        let trade = Trade {
            sequence,
            maker_order_id: maker.order.resting.id,
            taker_order_id: taker.resting.id,
            maker_id: maker.order.resting.user_id,
            taker_id: taker.resting.user_id,
            taker_side: taker.side,
            quantity: fill.quantity,
            price: fill.price,
        };
        taker.resting.open_qty =
            taker
                .resting
                .open_qty
                .checked_sub(fill.quantity)
                .ok_or(BookError::Invariant(
                    "planned fill exceeds the taker quantity during apply",
                ))?;
        self.apply_maker_fill(taker.side, fill.maker_node_id, fill.price, fill.quantity)?;
        Ok(trade)
    }

    fn rest(&mut self, order: Order) -> Result<(), BookError> {
        let price = order
            .limit_price
            .ok_or(BookError::Invariant("only validated limit orders may rest"))?;
        let side = order.side;
        let order_id = order.resting.id;
        let quantity = order.resting.open_qty;
        let existing_level = self.levels(side).get(&price);
        let tail = existing_level.and_then(|level| level.tail);
        let next_count = existing_level.map_or(Ok(1), |level| {
            level
                .count
                .checked_add(1)
                .ok_or(BookError::Invariant("price level count overflow"))
        })?;
        let next_quantity = existing_level
            .map_or_else(Quantity::default, |level| level.quantity)
            .checked_add(quantity)
            .ok_or(BookError::QuantityOverflow)?;

        if let Some(tail_id) = tail {
            let tail_node = self
                .arena
                .get(tail_id)
                .map_err(|_| BookError::Invariant("price level tail was released"))?;
            if tail_node.next.is_some() || tail_node.order.side != side || tail_node.price != price
            {
                return Err(BookError::Invariant("price level tail is inconsistent"));
            }
        } else if existing_level.is_some() {
            return Err(BookError::Invariant("active price level has no tail"));
        }

        let node_id = self
            .arena
            .allocate(OrderNode {
                order,
                price,
                prev: None,
                next: None,
            })
            .map_err(|error| match error {
                ArenaError::Exhausted => BookError::Full,
                ArenaError::InvalidNode => {
                    BookError::Invariant("arena free list contains an invalid node")
                }
            })?;

        if let Some(tail) = tail {
            self.arena
                .get_mut(tail)
                .map_err(|_| BookError::Invariant("validated price level tail was released"))?
                .next = Some(node_id);
            self.arena
                .get_mut(node_id)
                .map_err(|_| BookError::Invariant("newly allocated order node is missing"))?
                .prev = Some(tail);
        }
        let level = self.level_mut(side, price);
        if tail.is_none() {
            level.head = Some(node_id);
        }
        level.tail = Some(node_id);
        level.count = next_count;
        level.quantity = next_quantity;
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
        let maker_open_quantity = self
            .arena
            .get(node_id)
            .map_err(|_| BookError::Invariant("planned maker node was released"))?
            .order
            .resting
            .open_qty;

        let next_maker_quantity =
            maker_open_quantity
                .checked_sub(quantity)
                .ok_or(BookError::Invariant(
                    "fill quantity exceeds maker open quantity",
                ))?;

        let fully_filled = next_maker_quantity == Quantity::from(0);

        let maker_side = match taker_side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        let next_level_quantity = self
            .levels(maker_side)
            .get(&price)
            .ok_or(BookError::Invariant("maker price level is missing"))?
            .quantity
            .checked_sub(quantity)
            .ok_or(BookError::Invariant(
                "fill exceeds aggregate price level quantity",
            ))?;

        self.arena
            .get_mut(node_id)
            .map_err(|_| BookError::Invariant("validated maker node was released"))?
            .order
            .resting
            .open_qty = next_maker_quantity;
        self.levels_mut(maker_side)
            .get_mut(&price)
            .ok_or(BookError::Invariant(
                "validated maker price level is missing",
            ))?
            .quantity = next_level_quantity;
        if fully_filled {
            self.remove_resting(node_id)?;
        }
        Ok(())
    }

    fn remove_resting(&mut self, node_id: NodeId) -> Result<(), BookError> {
        let (order_id, side, price, previous, next) = {
            let node = self
                .arena
                .get(node_id)
                .map_err(|_| BookError::Invariant("removed order node was released"))?;
            (
                node.order.resting.id,
                node.order.side,
                node.price,
                node.prev,
                node.next,
            )
        };
        let remaining = self
            .arena
            .get(node_id)
            .map_err(|_| BookError::Invariant("removed order node was released"))?
            .order
            .resting
            .open_qty;
        let level = self
            .levels(side)
            .get(&price)
            .ok_or(BookError::Invariant("removed order price level is missing"))?;
        let next_count = level.count.checked_sub(1).ok_or(BookError::Invariant(
            "price level count underflow while removing an order",
        ))?;
        let next_quantity = level
            .quantity
            .checked_sub(remaining)
            .ok_or(BookError::Invariant(
                "price level quantity underflow while removing an order",
            ))?;
        if self.locations.get(&order_id) != Some(&node_id) {
            return Err(BookError::Invariant(
                "removed order location does not reference its node",
            ));
        }
        self.validate_removal_links(node_id, previous, next, level)?;

        if let Some(previous) = previous {
            self.arena
                .get_mut(previous)
                .map_err(|_| BookError::Invariant("validated previous node was released"))?
                .next = next;
        }
        if let Some(next) = next {
            self.arena
                .get_mut(next)
                .map_err(|_| BookError::Invariant("validated next node was released"))?
                .prev = previous;
        }
        let level = self
            .levels_mut(side)
            .get_mut(&price)
            .ok_or(BookError::Invariant(
                "validated removed order price level is missing",
            ))?;
        if level.head == Some(node_id) {
            level.head = next;
        }
        if level.tail == Some(node_id) {
            level.tail = previous;
        }
        level.count = next_count;
        level.quantity = next_quantity;
        let empty = level.count == 0;
        if empty {
            self.levels_mut(side).remove(&price);
        }
        self.locations.remove(&order_id);
        self.arena
            .release(node_id)
            .map_err(|_| BookError::Invariant("validated removed node could not be released"))?;
        Ok(())
    }

    fn validate_removal_links(
        &self,
        node_id: NodeId,
        previous: Option<NodeId>,
        next: Option<NodeId>,
        level: &PriceLevel,
    ) -> Result<(), BookError> {
        if previous.is_none() != (level.head == Some(node_id)) {
            return Err(BookError::Invariant(
                "removed order predecessor and level head disagree",
            ));
        }
        if next.is_none() != (level.tail == Some(node_id)) {
            return Err(BookError::Invariant(
                "removed order successor and level tail disagree",
            ));
        }
        if let Some(previous_id) = previous {
            let previous_node = self
                .arena
                .get(previous_id)
                .map_err(|_| BookError::Invariant("previous order node was released"))?;
            if previous_node.next != Some(node_id) {
                return Err(BookError::Invariant(
                    "previous order does not link to removed order",
                ));
            }
        }
        if let Some(next_id) = next {
            let next_node = self
                .arena
                .get(next_id)
                .map_err(|_| BookError::Invariant("next order node was released"))?;
            if next_node.prev != Some(node_id) {
                return Err(BookError::Invariant(
                    "next order does not link to removed order",
                ));
            }
        }
        Ok(())
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

    fn level_mut(&mut self, side: OrderSide, price: Price) -> &mut PriceLevel {
        self.levels_mut(side).entry(price).or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{OrderId, RestingOrder, UserId};

    fn order(
        id: u64,
        user: u64,
        side: OrderSide,
        kind: OrderKind,
        price: Option<i64>,
        qty: u64,
    ) -> Order {
        Order {
            resting: RestingOrder {
                id: OrderId::from(id),
                user_id: UserId::from(user),
                original_qty: Quantity::from(qty),
                open_qty: Quantity::from(qty),
                accepted_sequence: Sequence::from(0),
            },
            limit_price: price.map(Price::from),
            kind,
            side,
        }
    }

    fn limit(id: u64, side: OrderSide, price: i64, qty: u64) -> Order {
        order(id, id, side, OrderKind::Limit, Some(price), qty)
    }

    fn market(id: u64, side: OrderSide, qty: u64) -> Order {
        order(id, id, side, OrderKind::Market, None, qty)
    }

    /// Submits at an explicit sequence and asserts the book's structural
    /// invariants still hold afterwards, regardless of success or failure.
    fn submit(
        book: &mut Book,
        incoming: Order,
        sequence: u64,
    ) -> Result<ExecutionReport, BookError> {
        let result = book.submit(incoming, Sequence::from(sequence));
        assert_invariants(book);
        result
    }

    /// Verifies every cross-index the book maintains agrees with the intrusive
    /// linked lists: level aggregates, head/tail links, the `locations` map, and
    /// the arena's live-node count.
    fn assert_invariants(book: &Book) {
        let mut live = 0usize;
        for (side, levels) in [(OrderSide::Buy, &book.bids), (OrderSide::Sell, &book.asks)] {
            for (&price, level) in levels {
                assert!(level.count > 0, "empty level retained at {price:?}");
                assert!(
                    level.head.is_some() && level.tail.is_some(),
                    "active level missing head/tail at {price:?}"
                );

                let mut cursor = level.head;
                let mut prev: Option<NodeId> = None;
                let mut walked = 0usize;
                let mut aggregate = Quantity::from(0);
                while let Some(node_id) = cursor {
                    let node = book.arena.get(node_id).expect("linked node must be live");
                    assert_eq!(node.price, price, "node price disagrees with its level");
                    assert_eq!(node.order.side, side, "node side disagrees with its level");
                    assert_eq!(node.prev, prev, "node predecessor link is broken");
                    assert_eq!(
                        book.locations.get(&node.order.resting.id),
                        Some(&node_id),
                        "locations does not reference the node"
                    );
                    aggregate = aggregate
                        .checked_add(node.order.resting.open_qty)
                        .expect("aggregate overflow while summing a level");
                    prev = Some(node_id);
                    cursor = node.next;
                    walked += 1;
                }

                assert_eq!(prev, level.tail, "walk did not terminate at the tail");
                assert_eq!(walked, level.count, "level count disagrees with the walk");
                assert_eq!(
                    aggregate, level.quantity,
                    "level quantity disagrees with the walk"
                );
                live += walked;
            }
        }

        assert_eq!(
            live,
            book.locations.len(),
            "locations size disagrees with live nodes"
        );
        assert_eq!(
            live,
            book.arena.live_count(),
            "arena live count disagrees with live nodes"
        );
    }

    /// A convenience projection of a report's fills for order-sensitive asserts.
    fn fills(report: &ExecutionReport) -> Vec<(OrderId, u64, i64)> {
        report
            .trades
            .iter()
            .map(|trade| {
                (
                    trade.maker_order_id,
                    trade.quantity.into_inner(),
                    trade.price.into_inner(),
                )
            })
            .collect()
    }

    #[test]
    fn empty_book_rests_an_unmatched_limit_order() {
        let mut book = Book::new(4);

        let report = submit(&mut book, limit(1, OrderSide::Buy, 100, 5), 1).unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(report.remaining_quantity, Quantity::from(5));
        assert_eq!(book.locations.len(), 1);
        assert_eq!(
            book.bids.get(&Price::from(100)).unwrap().quantity,
            Quantity::from(5)
        );
    }

    #[test]
    fn crossing_limit_fully_consumes_a_single_maker() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 5), 1).unwrap();

        let report = submit(&mut book, limit(2, OrderSide::Buy, 100, 5), 2).unwrap();

        assert_eq!(report.remaining_quantity, Quantity::from(0));
        assert_eq!(fills(&report), vec![(OrderId::from(1), 5, 100)]);
        // Maker fully filled and the taker fully filled: nothing rests.
        assert!(book.asks.is_empty() && book.bids.is_empty());
        assert_eq!(book.locations.len(), 0);
    }

    #[test]
    fn taker_remainder_rests_after_exhausting_liquidity() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 3), 1).unwrap();

        let report = submit(&mut book, limit(2, OrderSide::Buy, 100, 8), 2).unwrap();

        assert_eq!(fills(&report), vec![(OrderId::from(1), 3, 100)]);
        assert_eq!(report.remaining_quantity, Quantity::from(5));
        // The 5-lot remainder rests on the bid side at its own limit price.
        assert!(book.asks.is_empty());
        assert_eq!(
            book.bids.get(&Price::from(100)).unwrap().quantity,
            Quantity::from(5)
        );
    }

    #[test]
    fn maker_remainder_stays_resting_when_taker_is_smaller() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 10), 1).unwrap();

        let report = submit(&mut book, limit(2, OrderSide::Buy, 100, 4), 2).unwrap();

        assert_eq!(fills(&report), vec![(OrderId::from(1), 4, 100)]);
        assert_eq!(report.remaining_quantity, Quantity::from(0));
        assert_eq!(
            book.asks.get(&Price::from(100)).unwrap().quantity,
            Quantity::from(6)
        );
        assert!(book.bids.is_empty());
    }

    #[test]
    fn price_time_priority_is_fifo_within_a_level() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 5), 1).unwrap();
        submit(&mut book, limit(2, OrderSide::Sell, 100, 5), 2).unwrap();

        let report = submit(&mut book, limit(3, OrderSide::Buy, 100, 7), 3).unwrap();

        // Oldest resting order (id 1) is filled before id 2.
        assert_eq!(
            fills(&report),
            vec![(OrderId::from(1), 5, 100), (OrderId::from(2), 2, 100)]
        );
        assert_eq!(
            book.asks.get(&Price::from(100)).unwrap().quantity,
            Quantity::from(3)
        );
    }

    #[test]
    fn best_price_is_selected_and_replanned_across_levels() {
        let mut book = Book::new(6);
        submit(&mut book, limit(1, OrderSide::Sell, 101, 3), 1).unwrap();
        submit(&mut book, limit(2, OrderSide::Sell, 100, 2), 2).unwrap();

        let report = submit(&mut book, limit(3, OrderSide::Buy, 101, 4), 3).unwrap();

        // Best (lowest) ask 100 is taken first, then the book re-plans onto 101.
        assert_eq!(
            fills(&report),
            vec![(OrderId::from(2), 2, 100), (OrderId::from(1), 2, 101)]
        );
        assert_eq!(
            book.asks.get(&Price::from(101)).unwrap().quantity,
            Quantity::from(1)
        );
    }

    #[test]
    fn seller_matches_highest_bid_first() {
        let mut book = Book::new(6);
        submit(&mut book, limit(1, OrderSide::Buy, 99, 2), 1).unwrap();
        submit(&mut book, limit(2, OrderSide::Buy, 100, 2), 2).unwrap();

        let report = submit(&mut book, limit(3, OrderSide::Sell, 99, 3), 3).unwrap();

        assert_eq!(
            fills(&report),
            vec![(OrderId::from(2), 2, 100), (OrderId::from(1), 1, 99)]
        );
    }

    #[test]
    fn non_crossing_limit_does_not_match_and_rests() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 101, 5), 1).unwrap();

        let report = submit(&mut book, limit(2, OrderSide::Buy, 100, 5), 2).unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(report.remaining_quantity, Quantity::from(5));
        assert_eq!(
            book.asks.get(&Price::from(101)).unwrap().quantity,
            Quantity::from(5)
        );
        assert_eq!(
            book.bids.get(&Price::from(100)).unwrap().quantity,
            Quantity::from(5)
        );
    }

    #[test]
    fn market_order_ignores_price_and_never_rests_its_remainder() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 3), 1).unwrap();

        let report = submit(&mut book, market(2, OrderSide::Buy, 8), 2).unwrap();

        assert_eq!(fills(&report), vec![(OrderId::from(1), 3, 100)]);
        // Remainder is discarded, not rested: market orders leave no trace.
        assert_eq!(report.remaining_quantity, Quantity::from(5));
        assert!(book.asks.is_empty() && book.bids.is_empty());
        assert_eq!(book.locations.len(), 0);
    }

    #[test]
    fn market_order_against_empty_book_is_dropped_whole() {
        let mut book = Book::new(4);

        let report = submit(&mut book, market(1, OrderSide::Buy, 5), 1).unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(report.remaining_quantity, Quantity::from(5));
        assert_eq!(book.locations.len(), 0);
    }

    #[test]
    fn duplicate_active_order_id_is_rejected() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Buy, 100, 5), 1).unwrap();

        let result = submit(&mut book, limit(1, OrderSide::Buy, 100, 5), 2);

        assert_eq!(result, Err(BookError::DuplicateOrder(OrderId::from(1))));
    }

    #[test]
    fn order_id_may_be_reused_once_fully_filled() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 5), 1).unwrap();
        // id 1 fully fills and leaves the book, freeing its identifier.
        submit(&mut book, limit(2, OrderSide::Buy, 100, 5), 2).unwrap();

        let report = submit(&mut book, limit(1, OrderSide::Sell, 100, 4), 3);

        assert!(report.is_ok());
    }

    #[test]
    fn invalid_order_is_rejected_without_touching_the_book() {
        let mut book = Book::new(4);

        let result = submit(&mut book, limit(1, OrderSide::Buy, 100, 0), 1);

        assert_eq!(
            result,
            Err(BookError::InvalidOrder(OrderError::ZeroQuantity))
        );
        assert_eq!(book.locations.len(), 0);
    }

    #[test]
    fn resting_beyond_capacity_reports_full() {
        let mut book = Book::new(1);
        submit(&mut book, limit(1, OrderSide::Buy, 100, 5), 1).unwrap();

        let result = submit(&mut book, limit(2, OrderSide::Buy, 99, 5), 2);

        assert_eq!(result, Err(BookError::Full));
        // The rejected order left no residue behind.
        assert_eq!(book.locations.len(), 1);
        assert!(!book.bids.contains_key(&Price::from(99)));
    }

    #[test]
    fn released_capacity_is_reusable() {
        let mut book = Book::new(1);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 1), 1).unwrap();
        submit(&mut book, market(2, OrderSide::Buy, 1), 2).unwrap();

        // The single slot is free again after id 1 was consumed.
        let report = submit(&mut book, limit(3, OrderSide::Sell, 100, 1), 3).unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(book.locations.len(), 1);
    }

    #[test]
    fn aggregate_level_quantity_overflow_is_rejected_atomically() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, u64::MAX), 1).unwrap();

        let result = submit(&mut book, limit(2, OrderSide::Sell, 100, 1), 2);

        assert_eq!(result, Err(BookError::QuantityOverflow));
        // The overflowing order did not allocate or mutate the level.
        assert_eq!(book.asks.get(&Price::from(100)).unwrap().count, 1);
        assert_eq!(book.locations.len(), 1);
    }

    #[test]
    fn removing_the_head_preserves_the_rest_of_the_queue() {
        let mut book = Book::new(6);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 2), 1).unwrap();
        submit(&mut book, limit(2, OrderSide::Sell, 100, 2), 2).unwrap();
        submit(&mut book, limit(3, OrderSide::Sell, 100, 2), 3).unwrap();

        // Consume the head (id 1) exactly.
        submit(&mut book, limit(4, OrderSide::Buy, 100, 2), 4).unwrap();

        let level = book.asks.get(&Price::from(100)).unwrap();
        assert_eq!(level.count, 2);
        assert_eq!(level.quantity, Quantity::from(4));
        // Remaining queue still matches in time priority: id 2 then id 3.
        let report = submit(&mut book, limit(5, OrderSide::Buy, 100, 4), 5).unwrap();
        assert_eq!(
            fills(&report),
            vec![(OrderId::from(2), 2, 100), (OrderId::from(3), 2, 100)]
        );
    }

    #[test]
    fn trade_carries_sequence_parties_and_taker_side() {
        let mut book = Book::new(4);
        submit(&mut book, limit(1, OrderSide::Sell, 100, 5), 1).unwrap();

        let report = submit(&mut book, limit(2, OrderSide::Buy, 100, 5), 42).unwrap();

        let trade = &report.trades[0];
        assert_eq!(trade.sequence, Sequence::from(42));
        assert_eq!(trade.maker_order_id, OrderId::from(1));
        assert_eq!(trade.taker_order_id, OrderId::from(2));
        assert_eq!(trade.maker_id, UserId::from(1));
        assert_eq!(trade.taker_id, UserId::from(2));
        assert_eq!(trade.taker_side, OrderSide::Buy);
        assert_eq!(trade.price, Price::from(100));
    }

    #[test]
    fn self_trade_is_currently_permitted() {
        // Documents present behavior: the book has no self-trade prevention,
        // so a user may match against their own resting order.
        let mut book = Book::new(4);
        submit(
            &mut book,
            order(1, 7, OrderSide::Sell, OrderKind::Limit, Some(100), 5),
            1,
        )
        .unwrap();

        let report = submit(
            &mut book,
            order(2, 7, OrderSide::Buy, OrderKind::Limit, Some(100), 5),
            2,
        )
        .unwrap();

        assert_eq!(report.trades.len(), 1);
        assert_eq!(report.trades[0].maker_id, report.trades[0].taker_id);
    }
}
