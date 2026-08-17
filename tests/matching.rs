use ob::{
    domain::{
        Order, OrderId, OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, Symbol,
        UserId,
    },
    matching::{BookError, Engine, Exchange, ExchangeError},
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
fn top_of_book_returns_best_aggregated_levels() {
    let mut engine = Engine::new(10);
    engine.submit(limit(1, OrderSide::Buy, 99, 2)).unwrap();
    engine.submit(limit(2, OrderSide::Buy, 100, 3)).unwrap();
    engine.submit(limit(3, OrderSide::Sell, 102, 4)).unwrap();
    engine.submit(limit(4, OrderSide::Sell, 101, 5)).unwrap();

    let top = engine.top_of_book();

    assert_eq!(
        top.bid,
        Some(ob::matching::PriceLevelView {
            price: Price::from(100),
            quantity: Quantity::from(3),
            order_count: 1,
        })
    );
    assert_eq!(
        top.ask,
        Some(ob::matching::PriceLevelView {
            price: Price::from(101),
            quantity: Quantity::from(5),
            order_count: 1,
        })
    );
}

#[test]
fn bounded_depth_is_best_first_and_aggregated() {
    let mut engine = Engine::new(10);
    engine.submit(limit(1, OrderSide::Buy, 98, 2)).unwrap();
    engine.submit(limit(2, OrderSide::Buy, 100, 3)).unwrap();
    engine.submit(limit(3, OrderSide::Buy, 99, 4)).unwrap();
    engine.submit(limit(4, OrderSide::Sell, 103, 5)).unwrap();
    engine.submit(limit(5, OrderSide::Sell, 101, 6)).unwrap();
    engine.submit(limit(6, OrderSide::Sell, 102, 7)).unwrap();

    let depth = engine.depth(2);

    assert_eq!(
        depth.bids,
        vec![
            ob::matching::PriceLevelView {
                price: Price::from(100),
                quantity: Quantity::from(3),
                order_count: 1,
            },
            ob::matching::PriceLevelView {
                price: Price::from(99),
                quantity: Quantity::from(4),
                order_count: 1,
            },
        ]
    );
    assert_eq!(
        depth.asks,
        vec![
            ob::matching::PriceLevelView {
                price: Price::from(101),
                quantity: Quantity::from(6),
                order_count: 1,
            },
            ob::matching::PriceLevelView {
                price: Price::from(102),
                quantity: Quantity::from(7),
                order_count: 1,
            },
        ]
    );
}

#[test]
fn full_snapshot_contains_all_levels() {
    let mut engine = Engine::new(10);
    engine.submit(limit(1, OrderSide::Buy, 98, 2)).unwrap();
    engine.submit(limit(2, OrderSide::Buy, 100, 3)).unwrap();
    engine.submit(limit(3, OrderSide::Buy, 99, 4)).unwrap();
    engine.submit(limit(4, OrderSide::Sell, 103, 5)).unwrap();
    engine.submit(limit(5, OrderSide::Sell, 101, 6)).unwrap();
    engine.submit(limit(6, OrderSide::Sell, 102, 7)).unwrap();

    let snapshot = engine.snapshot();

    assert_eq!(snapshot.bids.len(), 3);
    assert_eq!(snapshot.asks.len(), 3);
    assert_eq!(snapshot.bids[2].price, Price::from(98));
    assert_eq!(snapshot.asks[2].price, Price::from(103));
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

#[test]
fn canceling_head_preserves_tail_and_remaining_fifo_order() {
    let mut engine = Engine::new(3);
    engine.submit(limit(1, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(2, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(3, OrderSide::Sell, 100, 1)).unwrap();
    engine.cancel(OrderId::from(1)).unwrap();

    let report = engine.submit(limit(4, OrderSide::Buy, 100, 2)).unwrap();

    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| trade.maker_order_id)
            .collect::<Vec<_>>(),
        vec![OrderId::from(2), OrderId::from(3)]
    );
}

#[test]
fn canceling_tail_preserves_head_and_remaining_fifo_order() {
    let mut engine = Engine::new(3);
    engine.submit(limit(1, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(2, OrderSide::Sell, 100, 1)).unwrap();
    engine.submit(limit(3, OrderSide::Sell, 100, 1)).unwrap();
    engine.cancel(OrderId::from(3)).unwrap();

    let report = engine.submit(limit(4, OrderSide::Buy, 100, 2)).unwrap();

    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| trade.maker_order_id)
            .collect::<Vec<_>>(),
        vec![OrderId::from(1), OrderId::from(2)]
    );
}

#[test]
fn exchange_keeps_instruments_isolated() {
    let btc = Symbol::parse("BTC-USD").unwrap();
    let eth = Symbol::parse("ETH-USD").unwrap();
    let mut exchange = Exchange::new(4);
    exchange.prepare_symbols([btc, eth]);

    exchange
        .submit(btc, limit(1, OrderSide::Sell, 100, 2))
        .unwrap();
    let report = exchange
        .submit(eth, limit(2, OrderSide::Buy, 100, 2))
        .unwrap();

    assert!(report.trades.is_empty());
    assert_eq!(report.remaining_quantity, Quantity::from(2));
    assert_eq!(
        exchange.top_of_book(btc).unwrap().ask.unwrap().quantity,
        Quantity::from(2)
    );
    assert!(exchange.top_of_book(eth).unwrap().bid.is_some());
}

#[test]
fn exchange_rejects_unknown_instrument() {
    let symbol = Symbol::parse("BTC-USD").unwrap();
    let mut exchange = Exchange::new(1);

    let result = exchange.submit(symbol, limit(1, OrderSide::Buy, 100, 1));

    assert_eq!(result, Err(ExchangeError::UnknownSymbol(symbol)));
}
