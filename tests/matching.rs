use ob::{
    domain::{
        Order, OrderId, OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, UserId,
    },
    matching::Engine,
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
        allow_partial: true,
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
        allow_partial: true,
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
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.asks.len(), 1);
    assert_eq!(snapshot.asks[0].quantity, Quantity::from(3));
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
    assert!(engine.snapshot().bids.is_empty());
}

#[test]
fn cancel_removes_order_and_reuses_capacity() {
    let mut engine = Engine::new(1);
    engine.submit(limit(1, OrderSide::Buy, 99, 4)).unwrap();
    engine.cancel(OrderId::from(1)).unwrap();
    engine.submit(limit(2, OrderSide::Buy, 99, 4)).unwrap();
    assert_eq!(engine.snapshot().bids[0].quantity, Quantity::from(4));
}
