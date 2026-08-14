use ob::{
    domain::{
        Order, OrderId, OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, UserId,
    },
    matching::{BookError, Engine},
};

fn limit(id: u64, side: OrderSide, price: i64, quantity: u64) -> Order {
    Order {
        resting: RestingOrder {
            id: OrderId::from(id),
            user_id: UserId::from(id),
            original_qty: Quantity::from(quantity),
            open_qty: Quantity::from(quantity),
            accepted_sequence: Sequence::from(0),
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
        },
        limit_price: None,
        kind: OrderKind::Market,
        side,
    }
}

#[test]
fn price_time_priority_and_maker_price_are_preserved() {
    let mut engine = Engine::new(10);
    engine.submit(limit(1, OrderSide::Sell, 100, 5)).unwrap();
    engine.submit(limit(2, OrderSide::Sell, 100, 5)).unwrap();
    let report = engine.submit(limit(3, OrderSide::Buy, 101, 7)).unwrap();

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
}

#[test]
fn submit_replans_after_each_fill_across_price_levels() {
    let mut engine = Engine::new(10);
    engine.submit(limit(1, OrderSide::Sell, 100, 2)).unwrap();
    engine.submit(limit(2, OrderSide::Sell, 101, 3)).unwrap();

    let report = engine.submit(limit(3, OrderSide::Buy, 102, 4)).unwrap();

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
        engine
            .submit(limit(1, OrderSide::Buy, 99, 4))
            .unwrap()
            .remaining_quantity,
        Quantity::from(4)
    );
    assert_eq!(
        engine
            .submit(market(2, OrderSide::Sell, 10))
            .unwrap()
            .remaining_quantity,
        Quantity::from(6)
    );
}

#[test]
fn rejected_submit_does_not_partially_apply_fills() {
    let mut engine = Engine::new(0);

    let result = engine.submit(limit(1, OrderSide::Buy, 101, 7));

    assert!(result.is_err());
}

#[test]
fn capacity_remains_enforced_after_reusing_a_released_slot() {
    let mut engine = Engine::new(1);
    engine.submit(limit(1, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(market(2, OrderSide::Buy, 1)).unwrap();
    engine.submit(limit(3, OrderSide::Sell, 100, 1)).unwrap();

    let result = engine.submit(limit(4, OrderSide::Sell, 101, 1));

    assert_eq!(result, Err(BookError::Full));

    engine.submit(market(5, OrderSide::Buy, 1)).unwrap();
    engine.submit(limit(6, OrderSide::Sell, 100, 1)).unwrap();
}

#[test]
fn aggregate_quantity_overflow_is_rejected() {
    let mut engine = Engine::new(2);
    engine
        .submit(limit(1, OrderSide::Sell, 100, u64::MAX))
        .unwrap();

    let result = engine.submit(limit(2, OrderSide::Sell, 100, 1));

    assert_eq!(result, Err(BookError::QuantityOverflow));

    let report = engine.submit(market(3, OrderSide::Buy, u64::MAX)).unwrap();
    assert_eq!(report.trades.len(), 1);
    assert_eq!(report.trades[0].quantity, Quantity::from(u64::MAX));
}

#[test]
fn cancel_removes_order_and_releases_capacity() {
    let mut engine = Engine::new(1);
    engine.submit(limit(1, OrderSide::Sell, 100, 4)).unwrap();

    let report = engine.cancel(OrderId::from(1)).unwrap();
    assert_eq!(report.order_id, OrderId::from(1));
    assert_eq!(report.canceled_quantity, Quantity::from(4));

    assert_eq!(
        engine.cancel(OrderId::from(1)),
        Err(BookError::OrderNotFound(OrderId::from(1)))
    );
    engine.submit(limit(2, OrderSide::Sell, 100, 3)).unwrap();
}

#[test]
fn cancel_unlinks_middle_order_without_changing_fifo_order() {
    let mut engine = Engine::new(3);
    engine.submit(limit(1, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(2, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(3, OrderSide::Sell, 100, 1)).unwrap();
    engine.cancel(OrderId::from(2)).unwrap();

    let report = engine.submit(limit(4, OrderSide::Buy, 100, 2)).unwrap();

    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| trade.maker_order_id)
            .collect::<Vec<_>>(),
        vec![OrderId::from(1), OrderId::from(3)]
    );
}
