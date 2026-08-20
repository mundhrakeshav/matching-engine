use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};

use crate::{
    domain::{OrderId, Symbol},
    matching::{CancelReport, ExecutionReport},
    service::{BookDepth, ExchangeClient},
};

use super::{
    dto::{BookQuery, BookResponse, SubmitRequest, SymbolQuery},
    error::ApiError,
};

/// Builds the matching HTTP API router over `client`.
///
/// Handlers only ever call `client`'s public methods; they never construct
/// or inspect a [`crate::matching::Engine`] or [`crate::matching::Exchange`].
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
    let view = client.book(symbol, depth).await?;
    Ok(Json(BookResponse::from_view(
        symbol.to_string(),
        depth_label(depth),
        view,
    )))
}

async fn submit(
    State(client): State<Arc<ExchangeClient>>,
    Json(request): Json<SubmitRequest>,
) -> Result<Json<ExecutionReport>, ApiError> {
    let symbol = parse_symbol(&request.symbol)?;
    Ok(Json(client.submit(symbol, request.order).await?))
}

async fn cancel(
    State(client): State<Arc<ExchangeClient>>,
    Path(id): Path<u64>,
    Query(query): Query<SymbolQuery>,
) -> Result<Json<CancelReport>, ApiError> {
    let symbol = parse_symbol(&query.symbol)?;
    Ok(Json(client.cancel(symbol, OrderId::from(id)).await?))
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

fn depth_label(depth: BookDepth) -> String {
    match depth {
        BookDepth::Top => "top".to_owned(),
        BookDepth::Levels(levels) => levels.to_string(),
        BookDepth::Full => "full".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::{matching::Exchange, service::spawn_exchange};

    use super::*;

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
    async fn unknown_symbol_returns_bad_request() {
        let response = router(spawn_exchange(Exchange::new(2), 64).unwrap())
            .oneshot(
                Request::builder()
                    .uri("/v1/matching/book?symbol=BTC-USD&depth=top")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
}
