use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

use crate::{
    domain::{
        Order, OrderId, OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, UserId,
    },
    matching::{BookError, Engine, ExecutionReport, OrderBookSnapshot},
};

#[derive(Clone, Debug)]
pub struct AppState {
    service_name: Arc<str>,
    engine: Arc<Mutex<Engine>>,
}

impl AppState {
    pub fn new(service_name: impl Into<Arc<str>>, book_capacity: usize) -> Self {
        Self {
            service_name: service_name.into(),
            engine: Arc::new(Mutex::new(Engine::new(book_capacity))),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/healthz", get(health))
        .route("/status/{service_name}", get(service_status))
        .route("/v1/matching/status", get(matching_status))
        .route("/v1/matching/book", get(snapshot))
        .route("/v1/matching/order", post(submit_order))
        .route(
            "/v1/matching/order/{id}",
            get(order_status).patch(replace_order).delete(cancel_order),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn root() -> &'static str {
    "Ok"
}

async fn health() -> Json<ApiResponse<Vec<HealthCheck>>> {
    Json(ApiResponse { data: Vec::new() })
}

async fn service_status(
    State(state): State<AppState>,
    Path(service_name): Path<String>,
) -> impl IntoResponse {
    if service_name == state.service_name.as_ref() {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "message": "service is up" })),
        )
            .into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "message": "service not found" })),
        )
            .into_response()
    }
}

async fn matching_status() -> Json<ApiResponse<Status>> {
    Json(ApiResponse {
        data: Status { status: "ok" },
    })
}

async fn snapshot(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<OrderBookSnapshot>>, ApiError> {
    let engine = state
        .engine
        .lock()
        .map_err(|_| ApiError::internal("matching engine lock poisoned"))?;
    Ok(Json(ApiResponse {
        data: engine.snapshot(),
    }))
}

async fn submit_order(
    State(state): State<AppState>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<(StatusCode, Json<ApiResponse<ExecutionReport>>), ApiError> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| ApiError::internal("matching engine lock poisoned"))?;
    let report = engine
        .submit(request.into_order())
        .map_err(ApiError::from)?;
    Ok((StatusCode::CREATED, Json(ApiResponse { data: report })))
}

async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| ApiError::internal("matching engine lock poisoned"))?;
    engine.cancel(OrderId::from(id)).map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

// These routes retain the existing API surface. Order lookup and amend semantics
// are intentionally deferred until their lifecycle rules are specified.
async fn order_status(Path(_id): Path<u64>) -> Json<ApiResponse<Status>> {
    Json(ApiResponse {
        data: Status { status: "ok" },
    })
}

async fn replace_order(Path(_id): Path<u64>) -> Json<ApiResponse<Status>> {
    Json(ApiResponse {
        data: Status { status: "ok" },
    })
}

#[derive(Debug, Deserialize)]
pub struct SubmitOrderRequest {
    pub id: u64,
    pub user_id: u64,
    pub quantity: u64,
    pub side: OrderSide,
    pub kind: OrderKind,
    pub limit_price: Option<i64>,
    #[serde(default)]
    pub allow_partial: bool,
}

impl SubmitOrderRequest {
    fn into_order(self) -> Order {
        Order {
            resting: RestingOrder {
                id: OrderId::from(self.id),
                user_id: UserId::from(self.user_id),
                original_qty: Quantity::from(self.quantity),
                open_qty: Quantity::from(self.quantity),
                accepted_sequence: Sequence::from(0),
            },
            limit_price: self.limit_price.map(Price::from),
            kind: self.kind,
            side: self.side,
            allow_partial: self.allow_partial,
        }
    }
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, Serialize)]
struct HealthCheck {
    name: String,
    healthy: bool,
}

#[derive(Debug, Serialize)]
struct Status {
    status: &'static str,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn internal(message: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.to_owned(),
        }
    }
}

impl From<BookError> for ApiError {
    fn from(error: BookError) -> Self {
        let status = match error {
            BookError::OrderNotFound(_) => StatusCode::NOT_FOUND,
            BookError::InvalidOrder(_) | BookError::DuplicateOrder(_) | BookError::Arena => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            BookError::Invariant(_) | BookError::SequenceExhausted => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({ "message": self.message })),
        )
            .into_response()
    }
}
