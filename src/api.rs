use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use disruptor::{
    MultiProducer, Producer, RingBufferFull, SingleConsumerBarrier, Sleep, build_multi_producer,
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    domain::{Order, OrderId, Symbol},
    matching::{BookSnapshot, CancelReport, Engine, Exchange, ExecutionReport, PriceLevelView},
};

const MINIMUM_RING_CAPACITY: usize = 64;
const ENGINE_WAIT: Duration = Duration::from_millis(1);

type CommandProducer = MultiProducer<CommandSlot, SingleConsumerBarrier>;

/// Routes requests to the preconfigured, independently owned symbol engines.
#[derive(Clone)]
pub struct ExchangeClient {
    engines: Arc<HashMap<Symbol, EngineClient>>,
}

impl fmt::Debug for ExchangeClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExchangeClient")
            .field("symbols", &self.engines.len())
            .finish_non_exhaustive()
    }
}

/// Publishes commands into one symbol's multi-producer Disruptor ring.
pub struct EngineClient {
    producer: CommandProducer,
}

impl Clone for EngineClient {
    fn clone(&self) -> Self {
        Self {
            producer: self.producer.clone(),
        }
    }
}

impl fmt::Debug for EngineClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EngineClient(..)")
    }
}

#[derive(Debug, Default)]
struct CommandSlot {
    command: Mutex<Option<EngineCommand>>,
}

#[derive(Debug)]
enum EngineCommand {
    Book {
        depth: BookDepth,
        reply: oneshot::Sender<Result<BookResponse, String>>,
    },
    Submit {
        order: Order,
        reply: oneshot::Sender<Result<ExecutionReport, String>>,
    },
    Cancel {
        order_id: OrderId,
        reply: oneshot::Sender<Result<CancelReport, String>>,
    },
}

#[derive(Debug, Clone, Copy)]
enum BookDepth {
    Top,
    Levels(usize),
    Full,
}

#[derive(Debug)]
struct EngineState {
    symbol: Symbol,
    engine: Engine,
}

impl EngineState {
    fn process(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Book { depth, reply } => {
                let _ = reply.send(Ok(self.book_response(depth)));
            }
            EngineCommand::Submit { order, reply } => {
                let _ = reply.send(self.engine.submit(order).map_err(|error| error.to_string()));
            }
            EngineCommand::Cancel { order_id, reply } => {
                let _ = reply.send(
                    self.engine
                        .cancel(order_id)
                        .map_err(|error| error.to_string()),
                );
            }
        }
    }

    fn book_response(&self, depth: BookDepth) -> BookResponse {
        match depth {
            BookDepth::Top => {
                let top = self.engine.top_of_book();
                BookResponse {
                    symbol: self.symbol.to_string(),
                    depth: "top".to_owned(),
                    bids: top.bid.into_iter().collect(),
                    asks: top.ask.into_iter().collect(),
                }
            }
            BookDepth::Levels(levels) => {
                snapshot_response(self.symbol, levels.to_string(), self.engine.depth(levels))
            }
            BookDepth::Full => {
                snapshot_response(self.symbol, "full".to_owned(), self.engine.snapshot())
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BookQuery {
    symbol: String,
    depth: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SymbolQuery {
    symbol: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitRequest {
    symbol: String,
    #[serde(flatten)]
    order: Order,
}

#[derive(Debug, Serialize)]
pub struct BookResponse {
    pub symbol: String,
    pub depth: String,
    pub bids: Vec<PriceLevelView>,
    pub asks: Vec<PriceLevelView>,
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    Overloaded,
    Unavailable,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Overloaded => (
                StatusCode::SERVICE_UNAVAILABLE,
                "matching engine ring is full".to_owned(),
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "matching engine is unavailable".to_owned(),
            ),
        };
        (status, message).into_response()
    }
}

/// Starts one multi-producer command ring and matching consumer per configured symbol.
///
/// # Errors
///
/// Returns an error when `ring_capacity` is smaller than 64 or is not a power of two.
pub fn spawn_exchange(
    exchange: Exchange,
    ring_capacity: usize,
) -> Result<ExchangeClient, &'static str> {
    validate_ring_capacity(ring_capacity)?;
    let engines = exchange
        .into_books()
        .into_iter()
        .map(|(symbol, engine)| (symbol, spawn_engine(symbol, engine, ring_capacity)))
        .collect();
    Ok(ExchangeClient {
        engines: Arc::new(engines),
    })
}

fn spawn_engine(symbol: Symbol, engine: Engine, ring_capacity: usize) -> EngineClient {
    let producer =
        build_multi_producer(ring_capacity, CommandSlot::default, Sleep::new(ENGINE_WAIT))
            .handle_events_and_state_with(
                |state: &mut EngineState, slot: &CommandSlot, _, _| {
                    let command = slot
                        .command
                        .lock()
                        .expect("command slot mutex poisoned")
                        .take();
                    if let Some(command) = command {
                        state.process(command);
                    }
                },
                move || EngineState { symbol, engine },
            )
            .build();
    EngineClient { producer }
}

impl ExchangeClient {
    fn engine(&self, symbol: Symbol) -> Result<&EngineClient, ApiError> {
        self.engines
            .get(&symbol)
            .ok_or_else(|| ApiError::BadRequest(format!("unknown symbol: {symbol}")))
    }
}

impl EngineClient {
    fn publish(&self, command: EngineCommand) -> Result<(), ApiError> {
        let mut producer = self.producer.clone();
        producer
            .try_publish(|slot| {
                *slot.command.lock().expect("command slot mutex poisoned") = Some(command);
            })
            .map(|_| ())
            .map_err(map_publish_error)
    }
}

pub fn router(client: ExchangeClient) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/matching/book", get(book))
        .route("/v1/matching/order", post(submit))
        .route("/v1/matching/order/{id}", delete(cancel))
        .with_state(Arc::new(client))
}

async fn healthz() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn book(
    State(client): State<Arc<ExchangeClient>>,
    Query(query): Query<BookQuery>,
) -> Result<Json<BookResponse>, ApiError> {
    let symbol = parse_symbol(&query.symbol)?;
    let depth = parse_depth(query.depth.as_deref().unwrap_or("top"))?;
    let (reply, response) = oneshot::channel();
    client
        .engine(symbol)?
        .publish(EngineCommand::Book { depth, reply })?;
    response
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(Json)
        .map_err(ApiError::BadRequest)
}

async fn submit(
    State(client): State<Arc<ExchangeClient>>,
    Json(request): Json<SubmitRequest>,
) -> Result<Json<ExecutionReport>, ApiError> {
    let symbol = parse_symbol(&request.symbol)?;
    let (reply, response) = oneshot::channel();
    client.engine(symbol)?.publish(EngineCommand::Submit {
        order: request.order,
        reply,
    })?;
    response
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(Json)
        .map_err(ApiError::BadRequest)
}

async fn cancel(
    State(client): State<Arc<ExchangeClient>>,
    Path(id): Path<u64>,
    Query(query): Query<SymbolQuery>,
) -> Result<Json<CancelReport>, ApiError> {
    let symbol = parse_symbol(&query.symbol)?;
    let (reply, response) = oneshot::channel();
    client.engine(symbol)?.publish(EngineCommand::Cancel {
        order_id: OrderId::from(id),
        reply,
    })?;
    response
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(Json)
        .map_err(ApiError::BadRequest)
}

fn snapshot_response(symbol: Symbol, depth: String, snapshot: BookSnapshot) -> BookResponse {
    BookResponse {
        symbol: symbol.to_string(),
        depth,
        bids: snapshot.bids,
        asks: snapshot.asks,
    }
}

fn parse_symbol(value: &str) -> Result<Symbol, ApiError> {
    Symbol::parse(value).map_err(|_| ApiError::BadRequest("invalid symbol".to_owned()))
}

fn parse_depth(value: &str) -> Result<BookDepth, ApiError> {
    match value {
        "top" => Ok(BookDepth::Top),
        "full" => Ok(BookDepth::Full),
        value => value.parse::<usize>().map(BookDepth::Levels).map_err(|_| {
            ApiError::BadRequest("depth must be top, full, or a non-negative integer".to_owned())
        }),
    }
}

fn validate_ring_capacity(capacity: usize) -> Result<(), &'static str> {
    if capacity < MINIMUM_RING_CAPACITY || !capacity.is_power_of_two() {
        return Err("ring capacity must be a power of two and at least 64");
    }
    Ok(())
}

fn map_publish_error(_: RingBufferFull) -> ApiError {
    ApiError::Overloaded
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn limit(id: u64, side: crate::domain::OrderSide) -> Order {
        Order {
            resting: crate::domain::RestingOrder {
                id: OrderId::from(id),
                user_id: crate::domain::UserId::from(id),
                original_qty: crate::domain::Quantity::from(1),
                open_qty: crate::domain::Quantity::from(1),
                accepted_sequence: crate::domain::Sequence::from(0),
            },
            limit_price: Some(crate::domain::Price::from(100)),
            kind: crate::domain::OrderKind::Limit,
            side,
        }
    }

    #[tokio::test]
    async fn book_endpoint_routes_by_symbol() {
        let btc = Symbol::parse("BTC-USD").unwrap();
        let mut exchange = Exchange::new(2);
        exchange.prepare_symbols([btc]);

        let response = router(spawn_exchange(exchange, 64).unwrap())
            .oneshot(
                Request::builder()
                    .uri("/v1/matching/book?symbol=BTC-USD&depth=top")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn symbol_rings_are_isolated() {
        let btc = Symbol::parse("BTC-USD").unwrap();
        let eth = Symbol::parse("ETH-USD").unwrap();
        let mut exchange = Exchange::new(2);
        exchange.prepare_symbols([btc, eth]);
        let client = spawn_exchange(exchange, 64).unwrap();

        let (btc_reply, btc_response) = oneshot::channel();
        client
            .engine(btc)
            .unwrap()
            .publish(EngineCommand::Submit {
                order: limit(1, crate::domain::OrderSide::Sell),
                reply: btc_reply,
            })
            .unwrap();
        btc_response.await.unwrap().unwrap();

        let (eth_reply, eth_response) = oneshot::channel();
        client
            .engine(eth)
            .unwrap()
            .publish(EngineCommand::Submit {
                order: limit(1, crate::domain::OrderSide::Buy),
                reply: eth_reply,
            })
            .unwrap();
        let report = eth_response.await.unwrap().unwrap();

        assert!(report.trades.is_empty());
        assert_eq!(report.remaining_quantity, crate::domain::Quantity::from(1));
    }

    #[test]
    fn full_ring_rejects_publication() {
        use std::sync::Barrier;

        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let consumer_entered = Arc::clone(&entered);
        let consumer_release = Arc::clone(&release);
        let mut pause_first_event = true;
        let mut producer = build_multi_producer(64, || (), Sleep::new(ENGINE_WAIT))
            .handle_events_with(move |(), _, _| {
                if pause_first_event {
                    pause_first_event = false;
                    consumer_entered.wait();
                    consumer_release.wait();
                }
            })
            .build();

        producer.try_publish(|()| {}).unwrap();
        entered.wait();
        for _ in 1..64 {
            producer.try_publish(|()| {}).unwrap();
        }
        assert!(matches!(producer.try_publish(|()| {}), Err(RingBufferFull)));

        release.wait();
    }

    #[tokio::test]
    async fn submit_and_cancel_commands_use_symbol_ring() {
        let btc = Symbol::parse("BTC-USD").unwrap();
        let mut exchange = Exchange::new(2);
        exchange.prepare_symbols([btc]);
        let app = router(spawn_exchange(exchange, 64).unwrap());
        let order = serde_json::json!({
            "symbol": "BTC-USD",
            "id": 1,
            "user_id": 7,
            "original_qty": 5,
            "open_qty": 5,
            "accepted_sequence": 0,
            "limit_price": 100,
            "kind": "limit",
            "side": "buy"
        });

        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/matching/order")
                    .header("content-type", "application/json")
                    .body(Body::from(order.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(submit_response.status(), StatusCode::OK);

        let cancel_response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/matching/order/1?symbol=BTC-USD")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cancel_response.status(), StatusCode::OK);
    }

    #[test]
    fn invalid_ring_capacity_is_rejected() {
        assert_eq!(
            validate_ring_capacity(0),
            Err("ring capacity must be a power of two and at least 64")
        );
        assert_eq!(
            validate_ring_capacity(65),
            Err("ring capacity must be a power of two and at least 64")
        );
        assert_eq!(validate_ring_capacity(64), Ok(()));
    }

    #[test]
    fn full_ring_maps_to_overload() {
        assert!(matches!(
            map_publish_error(RingBufferFull),
            ApiError::Overloaded
        ));
    }
}
