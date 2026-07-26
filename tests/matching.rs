use ob::{
    domain::{
        Order, OrderError, OrderId, OrderKind, OrderSide, OrderStatus, Price, Quantity,
        RestingOrder, Sequence, UserId,
    },
    matching::{Engine, ExecutionReport, RejectReason, SubmitOutcome},
};

fn limit(id: u64, side: OrderSide, price: i64, quantity: u64) -> Order {
    Order {
        resting: RestingOrder {
            id: OrderId::from(id),
            user_id: UserId::from(id),
            original_qty: Quantity::from(quantity),
            open_qty: Quantity::from(quantity),
            accepted_sequence: Sequence::from(0),
            status: OrderStatus::New,
        },
        limit_price: Some(Price::from(price)),
        kind: OrderKind::Limit,
        side,
    }
}

fn market(id: u64, side: OrderSide, quantity: u64) -> Order {
    Order {
        resting: RestingOrder {
            id: OrderId::from(id),
            user_id: UserId::from(id),
            original_qty: Quantity::from(quantity),
            open_qty: Quantity::from(quantity),
            accepted_sequence: Sequence::from(0),
            status: OrderStatus::New,
        },
        limit_price: None,
        kind: OrderKind::Market,
        side,
    }
}

fn accepted(engine: &mut Engine, order: Order) -> ExecutionReport {
    match engine.submit(order).expect("submission must not fault") {
        SubmitOutcome::Accepted(report) => report,
        SubmitOutcome::Rejected(report) => panic!("order was rejected: {:?}", report.reason),
    }
}

#[test]
fn price_time_priority_and_maker_price_are_preserved() {
    let mut engine = Engine::new(10);
    accepted(&mut engine, limit(1, OrderSide::Sell, 100, 5));
    accepted(&mut engine, limit(2, OrderSide::Sell, 100, 5));
    let report = accepted(&mut engine, limit(3, OrderSide::Buy, 101, 7));

    assert_eq!(report.status, OrderStatus::Filled);
    assert_eq!(report.remaining_quantity, Quantity::from(0));
    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| (trade.maker_order_id, trade.quantity, trade.price))
            .collect::<Vec<_>>(),
        vec![
            (OrderId::from(1), Quantity::from(5), Price::from(100)),
            (OrderId::from(2), Quantity::from(2), Price::from(100)),
        ]
    );
    assert!(
        report
            .trades
            .iter()
            .all(|trade| trade.sequence == Sequence::from(3))
    );
}

#[test]
fn submit_replans_after_each_fill_across_price_levels() {
    let mut engine = Engine::new(10);
    accepted(&mut engine, limit(1, OrderSide::Sell, 100, 2));
    accepted(&mut engine, limit(2, OrderSide::Sell, 101, 3));

    let report = accepted(&mut engine, limit(3, OrderSide::Buy, 102, 4));

    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| (trade.maker_order_id, trade.quantity, trade.price))
            .collect::<Vec<_>>(),
        vec![
            (OrderId::from(1), Quantity::from(2), Price::from(100)),
            (OrderId::from(2), Quantity::from(2), Price::from(101)),
        ]
    );
}

#[test]
fn unmatched_limit_order_rests_but_market_order_does_not() {
    let mut engine = Engine::new(10);
    assert_eq!(
        accepted(&mut engine, limit(1, OrderSide::Buy, 99, 4)).remaining_quantity,
        Quantity::from(4)
    );
    assert_eq!(
        accepted(&mut engine, market(2, OrderSide::Sell, 10)).remaining_quantity,
        Quantity::from(6)
    );
}

#[test]
fn rejected_submit_does_not_partially_apply_fills() {
    let mut engine = Engine::new(0);

    let result = engine.submit(limit(1, OrderSide::Buy, 101, 7));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report))
            if report.status == OrderStatus::Rejected
                && report.reason == RejectReason::BookFull
    ));
}

#[test]
fn invalid_order_is_a_rejected_outcome_not_an_engine_fault() {
    let mut engine = Engine::new(1);

    let result = engine.submit(limit(7, OrderSide::Buy, 101, 0));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report))
            if report.order_id == OrderId::from(7)
                && report.status == OrderStatus::Rejected
                && report.reason
                    == RejectReason::InvalidOrder(OrderError::ZeroQuantity)
    ));
}

#[test]
fn capacity_remains_enforced_after_reusing_a_released_slot() {
    let mut engine = Engine::new(1);
    accepted(&mut engine, limit(1, OrderSide::Sell, 100, 1));
    accepted(&mut engine, market(2, OrderSide::Buy, 1));
    accepted(&mut engine, limit(3, OrderSide::Sell, 100, 1));

    let result = engine.submit(limit(4, OrderSide::Sell, 101, 1));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report)) if report.reason == RejectReason::BookFull
    ));

    accepted(&mut engine, market(5, OrderSide::Buy, 1));
    accepted(&mut engine, limit(6, OrderSide::Sell, 100, 1));
}

#[test]
fn aggregate_quantity_overflow_is_rejected() {
    let mut engine = Engine::new(2);
    accepted(&mut engine, limit(1, OrderSide::Sell, 100, u64::MAX));

    let result = engine.submit(limit(2, OrderSide::Sell, 100, 1));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report))
            if report.reason == RejectReason::QuantityOverflow
    ));

    let report = accepted(&mut engine, market(3, OrderSide::Buy, u64::MAX));
    assert_eq!(report.trades.len(), 1);
    assert_eq!(report.trades[0].quantity, Quantity::from(u64::MAX));
}

#[test]
fn resting_order_can_be_cancelled_through_engine() {
    let mut engine = Engine::new(2);
    accepted(&mut engine, limit(1, OrderSide::Buy, 100, 5));

    let cancelled = engine.cancel(OrderId::from(1)).unwrap();

    assert_eq!(cancelled.resting.id, OrderId::from(1));
    assert_eq!(cancelled.resting.open_qty, Quantity::from(5));
    assert_eq!(cancelled.resting.status, OrderStatus::Cancelled);
}

#[test]
fn partially_filled_order_can_be_cancelled_through_engine() {
    let mut engine = Engine::new(2);
    accepted(&mut engine, limit(1, OrderSide::Sell, 100, 5));
    accepted(&mut engine, limit(2, OrderSide::Buy, 100, 2));

    let cancelled = engine.cancel(OrderId::from(1)).unwrap();

    assert_eq!(cancelled.resting.open_qty, Quantity::from(3));
    assert_eq!(cancelled.resting.status, OrderStatus::Cancelled);
}

#[test]
fn cancelling_a_missing_order_returns_an_engine_error() {
    let mut engine = Engine::new(1);

    assert!(matches!(
        engine.cancel(OrderId::from(99)),
        Err(ob::matching::EngineFault::OrderNotFound(OrderId(99)))
    ));
}
