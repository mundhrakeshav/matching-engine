use ob::{
    domain::{Order, OrderKind, OrderSide, RestingOrder},
    matching::Engine,
};

fn limit(id: u64, side: OrderSide, price: i64, quantity: u64) -> Order {
    Order {
        resting: RestingOrder {
            id,
            user_id: id,
            original_qty: quantity,
            open_qty: quantity,
            accepted_sequence: 0,
        },
        limit_price: Some(price),
        kind: OrderKind::Limit,
        side,
        allow_partial: true,
    }
}

fn market(id: u64, side: OrderSide, quantity: u64) -> Order {
    Order {
        resting: RestingOrder {
            id,
            user_id: id,
            original_qty: quantity,
            open_qty: quantity,
            accepted_sequence: 0,
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

    assert_eq!(report.remaining_quantity, 0);
    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| (trade.maker_order_id, trade.quantity, trade.price))
            .collect::<Vec<_>>(),
        vec![(1, 5, 100), (2, 2, 100)]
    );
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.asks.len(), 1);
    assert_eq!(snapshot.asks[0].quantity, 3);
}

#[test]
fn unmatched_limit_order_rests_but_market_order_does_not() {
    let mut engine = Engine::new(10);
    assert_eq!(
        engine
            .submit(limit(1, OrderSide::Buy, 99, 4))
            .unwrap()
            .remaining_quantity,
        4
    );
    assert_eq!(
        engine
            .submit(market(2, OrderSide::Sell, 10))
            .unwrap()
            .remaining_quantity,
        6
    );
    assert!(engine.snapshot().bids.is_empty());
}

#[test]
fn cancel_removes_order_and_reuses_capacity() {
    let mut engine = Engine::new(1);
    engine.submit(limit(1, OrderSide::Buy, 99, 4)).unwrap();
    engine.cancel(1).unwrap();
    engine.submit(limit(2, OrderSide::Buy, 99, 4)).unwrap();
    assert_eq!(engine.snapshot().bids[0].quantity, 4);
}
