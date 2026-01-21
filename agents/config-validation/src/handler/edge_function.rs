//! Google Cloud Edge Function entry point
//!
//! This module provides the async handler function for Google Cloud Edge Functions.
//! It handles request parsing, input validation, response serialization, and error
//! handling with proper HTTP status codes.
//!
//! ## Design
//!
//! The edge function is designed for stateless, deterministic execution:
//! - No persistent state between invocations
//! - Same input always produces same output
//! - All errors are properly categorized with HTTP codes
//! - Telemetry is emitted for observability

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    Router,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use thiserror::Error;
use tower::ServiceExt;
use uuid::Uuid;

use super::{create_router, ApiResponse, ErrorInfo, HandlerState, MiddlewareState};

/// Configuration for the Edge Function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeFunctionConfig {
    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
    /// Request timeout in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Whether to enable detailed error messages
    #[serde(default)]
    pub debug_mode: bool,
    /// Whether to emit telemetry
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
    /// Custom telemetry endpoint (if different from default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry_endpoint: Option<String>,
}

fn default_max_body_size() -> usize {
    1024 * 1024 // 1MB
}

fn default_timeout_ms() -> u64 {
    30000 // 30 seconds
}

fn default_true() -> bool {
    true
}

impl Default for EdgeFunctionConfig {
    fn default() -> Self {
        Self {
            max_body_size: default_max_body_size(),
            timeout_ms: default_timeout_ms(),
            debug_mode: false,
            telemetry_enabled: true,
            telemetry_endpoint: None,
        }
    }
}

/// Errors that can occur during edge function execution
#[derive(Debug, Error)]
pub enum EdgeFunctionError {
    #[error("Request parsing failed: {0}")]
    ParseError(String),

    #[error("Request body too large: {size} bytes exceeds limit of {limit} bytes")]
    BodyTooLarge { size: usize, limit: usize },

    #[error("Request timeout after {0}ms")]
    Timeout(u64),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl EdgeFunctionError {
    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            EdgeFunctionError::ParseError(_) => StatusCode::BAD_REQUEST,
            EdgeFunctionError::BodyTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            EdgeFunctionError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            EdgeFunctionError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            EdgeFunctionError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            EdgeFunctionError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Get the error code for programmatic handling
    pub fn error_code(&self) -> &'static str {
        match self {
            EdgeFunctionError::ParseError(_) => "PARSE_ERROR",
            EdgeFunctionError::BodyTooLarge { .. } => "BODY_TOO_LARGE",
            EdgeFunctionError::Timeout(_) => "TIMEOUT",
            EdgeFunctionError::InvalidRequest(_) => "INVALID_REQUEST",
            EdgeFunctionError::InternalError(_) => "INTERNAL_ERROR",
            EdgeFunctionError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        }
    }

    /// Convert to ErrorInfo for API response
    pub fn to_error_info(&self) -> ErrorInfo {
        ErrorInfo::new(self.error_code(), self.to_string())
    }
}

/// Main entry point for the Google Cloud Edge Function
///
/// This function handles incoming HTTP requests and routes them to the
/// appropriate handler. It provides:
/// - Request ID generation for tracing
/// - Request body size validation
/// - Timeout handling
/// - Error response formatting
/// - Telemetry emission
///
/// # Arguments
///
/// * `request` - The incoming HTTP request
/// * `config` - Edge function configuration
///
/// # Returns
///
/// An HTTP response with appropriate status code and JSON body
pub async fn handle_request(
    request: Request<Body>,
    config: EdgeFunctionConfig,
) -> Response<Body> {
    let start_time = Instant::now();
    let request_id = generate_request_id();

    // Log incoming request
    tracing::info!(
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
        "Processing edge function request"
    );

    // Validate request body size from Content-Length header
    if let Some(content_length) = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        if content_length > config.max_body_size {
            return build_error_response(
                EdgeFunctionError::BodyTooLarge {
                    size: content_length,
                    limit: config.max_body_size,
                },
                &request_id,
                start_time.elapsed().as_millis() as u64,
            );
        }
    }

    // Create the router with handler state
    let handler_state = HandlerState::new();
    let middleware_state = MiddlewareState::new(config.telemetry_enabled);
    let router = create_router(handler_state, middleware_state);

    // Execute the request through the router
    match execute_with_timeout(router, request, config.timeout_ms).await {
        Ok(response) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            tracing::info!(
                request_id = %request_id,
                status = %response.status(),
                duration_ms = duration_ms,
                "Edge function request completed"
            );
            response
        }
        Err(err) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            tracing::error!(
                request_id = %request_id,
                error = %err,
                duration_ms = duration_ms,
                "Edge function request failed"
            );
            build_error_response(err, &request_id, duration_ms)
        }
    }
}

/// Execute request with timeout
async fn execute_with_timeout(
    router: Router,
    request: Request<Body>,
    timeout_ms: u64,
) -> Result<Response<Body>, EdgeFunctionError> {
    let timeout = std::time::Duration::from_millis(timeout_ms);

    match tokio::time::timeout(timeout, router.oneshot(request)).await {
        Ok(result) => result.map_err(|e| EdgeFunctionError::InternalError(e.to_string())),
        Err(_) => Err(EdgeFunctionError::Timeout(timeout_ms)),
    }
}

/// Generate a unique request ID
fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

/// Build an error response
fn build_error_response(
    error: EdgeFunctionError,
    request_id: &str,
    duration_ms: u64,
) -> Response<Body> {
    let status = error.status_code();
    let error_info = error.to_error_info();
    let mut response = ApiResponse::<()>::error(error_info, request_id.to_string());
    response.metadata.duration_ms = Some(duration_ms);

    let body = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"success":false,"error":{"code":"SERIALIZATION_ERROR","message":"Failed to serialize error response"}}"#.to_string()
    });

    Response::builder()
        .status(status)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header("X-Request-ID", request_id)
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        })
}

/// Initialize the edge function runtime
///
/// This function should be called once at cold start to initialize
/// any required resources.
pub fn initialize() -> Result<(), EdgeFunctionError> {
    // Initialize tracing subscriber for logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .json()
        .try_init()
        .ok(); // Ignore error if already initialized

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Config Validation Agent initialized"
    );

    Ok(())
}

/// Shutdown hook for the edge function
///
/// Called when the function instance is being terminated.
/// Use this to flush any pending telemetry or clean up resources.
pub async fn shutdown() {
    tracing::info!("Config Validation Agent shutting down");
    // Flush any pending telemetry
    // Note: In a real implementation, this would flush telemetry buffers
}

/// Parse and validate the incoming request body
pub async fn parse_request_body<T>(
    body: Body,
    max_size: usize,
) -> Result<T, EdgeFunctionError>
where
    T: serde::de::DeserializeOwned,
{
    // Collect body bytes with size limit
    let bytes = match axum::body::to_bytes(body, max_size).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(EdgeFunctionError::BodyTooLarge {
                size: max_size + 1, // Exceeded
                limit: max_size,
            })
        }
    };

    // Parse JSON
    serde_json::from_slice(&bytes)
        .map_err(|e| EdgeFunctionError::ParseError(format!("Invalid JSON: {}", e)))
}

/// Validate common request headers
pub fn validate_request_headers(
    request: &Request<Body>,
) -> Result<(), EdgeFunctionError> {
    // Check Content-Type for POST/PUT requests
    if matches!(
        request.method(),
        &axum::http::Method::POST | &axum::http::Method::PUT
    ) {
        let content_type = request
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.contains("application/json") {
            return Err(EdgeFunctionError::InvalidRequest(
                "Content-Type must be application/json".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_function_config_defaults() {
        let config = EdgeFunctionConfig::default();
        assert_eq!(config.max_body_size, 1024 * 1024);
        assert_eq!(config.timeout_ms, 30000);
        assert!(!config.debug_mode);
        assert!(config.telemetry_enabled);
    }

    #[test]
    fn test_edge_function_error_status_codes() {
        assert_eq!(
            EdgeFunctionError::ParseError("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            EdgeFunctionError::BodyTooLarge {
                size: 100,
                limit: 50
            }
            .status_code(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            EdgeFunctionError::Timeout(1000).status_code(),
            StatusCode::GATEWAY_TIMEOUT
        );
        assert_eq!(
            EdgeFunctionError::InvalidRequest("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            EdgeFunctionError::InternalError("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            EdgeFunctionError::ServiceUnavailable("test".to_string()).status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn test_edge_function_error_codes() {
        assert_eq!(
            EdgeFunctionError::ParseError("test".to_string()).error_code(),
            "PARSE_ERROR"
        );
        assert_eq!(
            EdgeFunctionError::BodyTooLarge {
                size: 100,
                limit: 50
            }
            .error_code(),
            "BODY_TOO_LARGE"
        );
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        assert_ne!(id1, id2);
        assert!(Uuid::parse_str(&id1).is_ok());
    }

    #[test]
    fn test_edge_function_error_to_error_info() {
        let error = EdgeFunctionError::InvalidRequest("Missing field".to_string());
        let info = error.to_error_info();
        assert_eq!(info.code, "INVALID_REQUEST");
        assert!(info.message.contains("Missing field"));
    }
}
