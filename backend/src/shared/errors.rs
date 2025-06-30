use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Main application error type
#[derive(Debug, Error)]
pub enum PeerPowerError {
    #[error("Database error: {message}")]
    Database { message: String },

    #[error("Provider not available: {carrier}")]
    ProviderUnavailable { carrier: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Validation error: {field} - {message}")]
    ValidationError { field: String, message: String },

    #[error("Payment processing failed: {reason}")]
    PaymentFailed { reason: String },

    #[error("External service error: {service} - {message}")]
    ExternalService { service: String, message: String },

    #[error("Rate limit exceeded: {resource}")]
    RateLimitExceeded { resource: String },

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Internal server error: {message}")]
    Internal { message: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Blockchain transaction failed: {reason}")]
    BlockchainError { reason: String },

    #[error("SMS delivery failed: {reason}")]
    SmsDeliveryFailed { reason: String },
}

impl PeerPowerError {
    /// Get HTTP status code for the error
    pub fn status_code(&self) -> StatusCode {
        match self {
            PeerPowerError::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            PeerPowerError::ProviderUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
            PeerPowerError::AuthenticationFailed { .. } => StatusCode::UNAUTHORIZED,
            PeerPowerError::ValidationError { .. } => StatusCode::BAD_REQUEST,
            PeerPowerError::PaymentFailed { .. } => StatusCode::PAYMENT_REQUIRED,
            PeerPowerError::ExternalService { .. } => StatusCode::BAD_GATEWAY,
            PeerPowerError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            PeerPowerError::NotFound { .. } => StatusCode::NOT_FOUND,
            PeerPowerError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            PeerPowerError::Configuration { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            PeerPowerError::BlockchainError { .. } => StatusCode::BAD_GATEWAY,
            PeerPowerError::SmsDeliveryFailed { .. } => StatusCode::BAD_GATEWAY,
        }
    }

    /// Get error code for client identification
    pub fn error_code(&self) -> &'static str {
        match self {
            PeerPowerError::Database { .. } => "DATABASE_ERROR",
            PeerPowerError::ProviderUnavailable { .. } => "PROVIDER_UNAVAILABLE",
            PeerPowerError::AuthenticationFailed { .. } => "AUTHENTICATION_FAILED",
            PeerPowerError::ValidationError { .. } => "VALIDATION_ERROR",
            PeerPowerError::PaymentFailed { .. } => "PAYMENT_FAILED",
            PeerPowerError::ExternalService { .. } => "EXTERNAL_SERVICE_ERROR",
            PeerPowerError::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            PeerPowerError::NotFound { .. } => "NOT_FOUND",
            PeerPowerError::Internal { .. } => "INTERNAL_ERROR",
            PeerPowerError::Configuration { .. } => "CONFIGURATION_ERROR",
            PeerPowerError::BlockchainError { .. } => "BLOCKCHAIN_ERROR",
            PeerPowerError::SmsDeliveryFailed { .. } => "SMS_DELIVERY_FAILED",
        }
    }
}

/// Convert error to HTTP response
impl IntoResponse for PeerPowerError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();

        let body = Json(json!({
            "error": {
                "code": self.error_code(),
                "message": self.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        }));

        (status_code, body).into_response()
    }
}

/// Application result type
pub type Result<T> = std::result::Result<T, PeerPowerError>;

/// Convert from common error types
impl From<mongodb::error::Error> for PeerPowerError {
    fn from(err: mongodb::error::Error) -> Self {
        PeerPowerError::Database {
            message: err.to_string(),
        }
    }
}

impl From<redis::RedisError> for PeerPowerError {
    fn from(err: redis::RedisError) -> Self {
        PeerPowerError::ExternalService {
            service: "Redis".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for PeerPowerError {
    fn from(err: reqwest::Error) -> Self {
        PeerPowerError::ExternalService {
            service: "HTTP".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for PeerPowerError {
    fn from(err: serde_json::Error) -> Self {
        PeerPowerError::ValidationError {
            field: "json".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<validator::ValidationErrors> for PeerPowerError {
    fn from(err: validator::ValidationErrors) -> Self {
        let message = err
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|e| {
                        e.message
                            .as_ref()
                            .unwrap_or(&std::borrow::Cow::Borrowed("invalid"))
                            .to_string()
                    })
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<_>>()
            .join("; ");

        PeerPowerError::ValidationError {
            field: "request".to_string(),
            message,
        }
    }
}
