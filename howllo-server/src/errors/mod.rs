use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use derive_more::Display;
use serde_json::json;

use crate::http;

#[derive(Debug, Display)]
pub enum AppError {
    #[display(fmt = "Unauthorized")]
    Unauthorized,
    #[display(fmt = "Forbidden")]
    Forbidden,
    #[display(fmt = "Not Found")]
    NotFound,
    #[display(fmt = "Internal Server Error")]
    InternalServerError,
    #[display(fmt = "Bad Request: {_0}")]
    BadRequest(String),
    #[display(fmt = "Validation Error: {_0}")]
    Validation(String),
    #[display(fmt = "Too Many Requests: {_0}")]
    TooManyRequests(String),
}

impl AppError {
    /// Log `detail` at error level and return `self` unchanged. Lets callers
    /// record the internal cause while returning a generic error to the client.
    pub fn with_log(self, detail: String) -> Self {
        tracing::error!(error = %detail, "{}", self);
        self
    }

    fn code(&self) -> &'static str {
        match self {
            AppError::Unauthorized => "unauthorized",
            AppError::Forbidden => "forbidden",
            AppError::NotFound => "not_found",
            AppError::InternalServerError => "internal_server_error",
            AppError::BadRequest(_) => "bad_request",
            AppError::Validation(_) => "validation_error",
            AppError::TooManyRequests(_) => "too_many_requests",
        }
    }

    fn message(&self) -> String {
        match self {
            AppError::Unauthorized => "Unauthorized".to_string(),
            AppError::Forbidden => "Forbidden".to_string(),
            AppError::NotFound => "Not Found".to_string(),
            AppError::InternalServerError => "Internal Server Error".to_string(),
            AppError::BadRequest(msg)
            | AppError::Validation(msg)
            | AppError::TooManyRequests(msg) => msg.clone(),
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({
            "error": {
                "code": self.code(),
                "message": self.message(),
                "request_id": http::current_request_id().unwrap_or_else(|| "unknown".to_string()),
            }
        }))
    }
}
