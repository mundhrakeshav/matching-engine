use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::service::ServiceError;

#[derive(Debug)]
pub enum ApiError {
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

impl From<ServiceError> for ApiError {
    fn from(error: ServiceError) -> Self {
        match error {
            ServiceError::UnknownSymbol(symbol) => {
                Self::BadRequest(format!("unknown symbol: {symbol}"))
            }
            ServiceError::Overloaded => Self::Overloaded,
            ServiceError::Unavailable => Self::Unavailable,
            ServiceError::Book(error) => Self::BadRequest(error.to_string()),
        }
    }
}
