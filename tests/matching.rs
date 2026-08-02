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

fn accepted(engine: &Engine, order: Order) -> ExecutionReport {
    match engine.submit(order).expect("submission must not fault") {
        SubmitOutcome::Accepted(report) => report,
        SubmitOutcome::Rejected(report) => panic!("order was rejected: {:?}", report.reason),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn price_time_priority_and_maker_price_are_preserved() {
    let engine = Engine::new(10);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 5));
    accepted(&engine, limit(2, OrderSide::Sell, 100, 5));
    let report = accepted(&engine, limit(3, OrderSide::Buy, 101, 7));

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_replans_after_each_fill_across_price_levels() {
    let engine = Engine::new(10);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 2));
    accepted(&engine, limit(2, OrderSide::Sell, 101, 3));

    let report = accepted(&engine, limit(3, OrderSide::Buy, 102, 4));

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn taker_can_remain_partially_filled_after_multiple_fills() {
    let engine = Engine::new(10);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 5));
    accepted(&engine, limit(2, OrderSide::Sell, 100, 5));

    let report = accepted(&engine, limit(3, OrderSide::Buy, 100, 20));

    assert_eq!(report.status, OrderStatus::PartiallyFilled);
    assert_eq!(report.remaining_quantity, Quantity::from(10));
    assert_eq!(
        report
            .trades
            .iter()
            .map(|trade| (trade.maker_order_id, trade.quantity))
            .collect::<Vec<_>>(),
        vec![
            (OrderId::from(1), Quantity::from(5)),
            (OrderId::from(2), Quantity::from(5)),
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn maker_can_remain_partially_filled_by_successive_takers() {
    let engine = Engine::new(10);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 20));
    accepted(&engine, market(2, OrderSide::Buy, 5));
    accepted(&engine, market(3, OrderSide::Buy, 5));

    let maker = engine.cancel(OrderId::from(1)).unwrap();

    assert_eq!(maker.resting.status, OrderStatus::Cancelled);
    assert_eq!(maker.resting.open_qty, Quantity::from(10));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unmatched_limit_order_rests_but_market_order_does_not() {
    let engine = Engine::new(10);
    assert_eq!(
        accepted(&engine, limit(1, OrderSide::Buy, 99, 4)).remaining_quantity,
        Quantity::from(4)
    );
    assert_eq!(
        accepted(&engine, market(2, OrderSide::Sell, 10)).remaining_quantity,
        Quantity::from(6)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_submit_does_not_partially_apply_fills() {
    let engine = Engine::new(0);

    let result = engine.submit(limit(1, OrderSide::Buy, 101, 7));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report))
            if report.status == OrderStatus::Rejected
                && report.reason == RejectReason::BookFull
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_order_is_a_rejected_outcome_not_an_engine_fault() {
    let engine = Engine::new(1);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capacity_remains_enforced_after_reusing_a_released_slot() {
    let engine = Engine::new(1);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 1));
    accepted(&engine, market(2, OrderSide::Buy, 1));
    accepted(&engine, limit(3, OrderSide::Sell, 100, 1));

    let result = engine.submit(limit(4, OrderSide::Sell, 101, 1));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report)) if report.reason == RejectReason::BookFull
    ));

    accepted(&engine, market(5, OrderSide::Buy, 1));
    accepted(&engine, limit(6, OrderSide::Sell, 100, 1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn aggregate_quantity_overflow_is_rejected() {
    let engine = Engine::new(2);
    accepted(&engine, limit(1, OrderSide::Sell, 100, u64::MAX));

    let result = engine.submit(limit(2, OrderSide::Sell, 100, 1));

    assert!(matches!(
        result,
        Ok(SubmitOutcome::Rejected(report))
            if report.reason == RejectReason::QuantityOverflow
    ));

    let report = accepted(&engine, market(3, OrderSide::Buy, u64::MAX));
    assert_eq!(report.trades.len(), 1);
    assert_eq!(report.trades[0].quantity, Quantity::from(u64::MAX));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resting_order_can_be_cancelled_through_engine() {
    let engine = Engine::new(2);
    accepted(&engine, limit(1, OrderSide::Buy, 100, 5));

    let cancelled = engine.cancel(OrderId::from(1)).unwrap();

    assert_eq!(cancelled.resting.id, OrderId::from(1));
    assert_eq!(cancelled.resting.open_qty, Quantity::from(5));
    assert_eq!(cancelled.resting.status, OrderStatus::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partially_filled_order_can_be_cancelled_through_engine() {
    let engine = Engine::new(2);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 5));
    accepted(&engine, limit(2, OrderSide::Buy, 100, 2));

    let cancelled = engine.cancel(OrderId::from(1)).unwrap();

    assert_eq!(cancelled.resting.open_qty, Quantity::from(3));
    assert_eq!(cancelled.resting.status, OrderStatus::Cancelled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_a_missing_order_returns_an_engine_error() {
    let engine = Engine::new(1);

    assert!(matches!(
        engine.cancel(OrderId::from(99)),
        Err(ob::matching::EngineFault::OrderNotFound(OrderId(99)))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_commands_are_processed_in_fifo_order() {
    let engine = Engine::new_with_queue_capacity(4, 4);

    let first = engine
        .enqueue(ob::matching::EngineCommand::Submit(limit(
            1,
            OrderSide::Sell,
            100,
            5,
        )))
        .unwrap();
    let second = engine
        .enqueue(ob::matching::EngineCommand::Submit(limit(
            2,
            OrderSide::Buy,
            100,
            3,
        )))
        .unwrap();

    assert!(matches!(
        first.await.unwrap(),
        ob::matching::EngineReply::Submit(Ok(SubmitOutcome::Accepted(report)))
            if report.order_id == OrderId::from(1)
    ));
    assert!(matches!(
        second.await.unwrap(),
        ob::matching::EngineReply::Submit(Ok(SubmitOutcome::Accepted(report)))
            if report.order_id == OrderId::from(2)
                && report.trades[0].maker_order_id == OrderId::from(1)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_rejects_invalid_commands_before_insertion() {
    let engine = Engine::new_with_queue_capacity(1, 1);

    assert!(matches!(
        engine.enqueue(ob::matching::EngineCommand::Submit(limit(
            4,
            OrderSide::Buy,
            100,
            0,
        ))),
        Err(ob::matching::CommandQueueError::InvalidOrder(
            OrderError::ZeroQuantity
        ))
    ));
    assert_eq!(engine.queued_commands(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_cancel_is_validated_against_book_when_processed() {
    let engine = Engine::new_with_queue_capacity(2, 2);
    accepted(&engine, limit(1, OrderSide::Sell, 100, 5));
    let reply = engine
        .enqueue(ob::matching::EngineCommand::Cancel(OrderId::from(1)))
        .unwrap();

    assert!(matches!(
        reply.await.unwrap(),
        ob::matching::EngineReply::Cancel(Ok(order))
            if order.resting.id == OrderId::from(1)
                && order.resting.status == OrderStatus::Cancelled
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_engine_can_receive_commands_from_concurrent_handlers() {
    let engine = std::sync::Arc::new(Engine::new_with_queue_capacity(8, 16));
    let workers = (1..=4)
        .map(|id| {
            let engine = std::sync::Arc::clone(&engine);
            tokio::spawn(
                async move { engine.submit_async(limit(id, OrderSide::Buy, 100, 1)).await },
            )
        })
        .collect::<Vec<_>>();

    for worker in workers {
        assert!(matches!(
            worker.await.unwrap(),
            Ok(SubmitOutcome::Accepted(_))
        ));
    }
}
