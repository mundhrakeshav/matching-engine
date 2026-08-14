use std::sync::{Arc, RwLock};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::matching::{Engine, PriceLevelView};

#[derive(Clone)]
pub struct AppState {
    engine: Arc<RwLock<Engine>>,
}

#[derive(Debug, Deserialize)]
pub struct BookQuery {
    depth: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookResponse {
    pub depth: String,
    pub bids: Vec<PriceLevelView>,
    pub asks: Vec<PriceLevelView>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: self.0 }),
        )
            .into_response()
    }
}

pub fn router(engine: Engine) -> Router {
    let state = AppState {
        engine: Arc::new(RwLock::new(engine)),
    };

    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/matching/book", get(book))
        .with_state(state)
}

async fn healthz() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn book(
    State(state): State<AppState>,
    Query(query): Query<BookQuery>,
) -> Result<Json<BookResponse>, ApiError> {
    let requested_depth = query.depth.as_deref().unwrap_or("top");
    let engine = state
        .engine
        .read()
        .map_err(|_| ApiError("engine lock poisoned".to_owned()))?;

    let response = match requested_depth {
        "top" => {
            let top = engine.top_of_book();
            BookResponse {
                depth: "top".to_owned(),
                bids: top.bid.into_iter().collect(),
                asks: top.ask.into_iter().collect(),
            }
        }
        "full" => {
            let snapshot = engine.snapshot();
            BookResponse {
                depth: "full".to_owned(),
                bids: snapshot.bids,
                asks: snapshot.asks,
            }
        }
        value => {
            let levels = value
                .parse::<usize>()
                .map_err(|_| ApiError("depth must be top, full, or a non-negative integer".to_owned()))?;
            let snapshot = engine.depth(levels);
            BookResponse {
                depth: value.to_owned(),
                bids: snapshot.bids,
                asks: snapshot.asks,
            }
        }
    };

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Order, OrderId, OrderKind, OrderSide, Price, Quantity, RestingOrder, Sequence, UserId};
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

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

    #[tokio::test]
    async fn book_endpoint_returns_requested_depth() {
        let mut engine = Engine::new(4);
        engine.submit(limit(1, OrderSide::Buy, 99, 2)).unwrap();
        engine.submit(limit(2, OrderSide::Buy, 100, 3)).unwrap();
        engine.submit(limit(3, OrderSide::Sell, 101, 4)).unwrap();

        let response = router(engine)
            .oneshot(
                Request::builder()
                    .uri("/v1/matching/book?depth=top")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["depth"], "top");
        assert_eq!(json["bids"][0]["price"], 100);
        assert_eq!(json["asks"][0]["price"], 101);
    }

    #[tokio::test]
    async fn book_endpoint_rejects_invalid_depth() {
        let response = router(Engine::new(1))
            .oneshot(
                Request::builder()
                    .uri("/v1/matching/book?depth=invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
