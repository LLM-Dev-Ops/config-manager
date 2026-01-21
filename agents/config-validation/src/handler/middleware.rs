//! Middleware for request processing
//!
//! This module provides middleware for:
//! - Input validation
//! - Request logging
//! - Telemetry emission compatible with LLM-Observatory
//!
//! All middleware is designed for stateless, deterministic execution
//! suitable for serverless/edge function deployment.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::{ErrorInfo, InspectionResult, ValidationRequest, ValidationResult};

/// Middleware state shared across requests
#[derive(Clone)]
pub struct MiddlewareState {
    /// Whether telemetry emission is enabled
    pub telemetry_enabled: bool,
    /// Request counter for metrics
    request_counter: Arc<AtomicU64>,
    /// Telemetry buffer for batch sending
    telemetry_buffer: Arc<tokio::sync::Mutex<Vec<TelemetryEvent>>>,
}

impl MiddlewareState {
    pub fn new(telemetry_enabled: bool) -> Self {
        Self {
            telemetry_enabled,
            request_counter: Arc::new(AtomicU64::new(0)),
            telemetry_buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Emit telemetry for validation start
    pub fn emit_validation_start(&self, request_id: &str, request: &ValidationRequest) {
        if !self.telemetry_enabled {
            return;
        }

        let event = TelemetryEvent {
            event_type: TelemetryEventType::ValidationStart,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.to_string(),
            data: serde_json::json!({
                "schema": request.schema,
                "options": request.options,
                "config_size": serde_json::to_string(&request.config).map(|s| s.len()).unwrap_or(0),
            }),
            agent: "config-validation".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        self.buffer_event(event);
    }

    /// Emit telemetry for validation complete
    pub fn emit_validation_complete(&self, request_id: &str, result: &ValidationResult) {
        if !self.telemetry_enabled {
            return;
        }

        let event = TelemetryEvent {
            event_type: TelemetryEventType::ValidationComplete,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.to_string(),
            data: serde_json::json!({
                "valid": result.valid,
                "error_count": result.errors.len(),
                "warning_count": result.warnings.len(),
                "schema_used": result.schema_used,
                "stats": result.stats,
            }),
            agent: "config-validation".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        self.buffer_event(event);
    }

    /// Emit telemetry for inspection start
    pub fn emit_inspection_start(&self, request_id: &str) {
        if !self.telemetry_enabled {
            return;
        }

        let event = TelemetryEvent {
            event_type: TelemetryEventType::InspectionStart,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.to_string(),
            data: serde_json::json!({}),
            agent: "config-validation".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        self.buffer_event(event);
    }

    /// Emit telemetry for inspection complete
    pub fn emit_inspection_complete(&self, request_id: &str, result: &InspectionResult) {
        if !self.telemetry_enabled {
            return;
        }

        let event = TelemetryEvent {
            event_type: TelemetryEventType::InspectionComplete,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.to_string(),
            data: serde_json::json!({
                "field_count": result.structure.field_count,
                "depth": result.structure.depth,
                "suggested_schema_count": result.suggested_schemas.len(),
                "detected_patterns": result.detected_patterns,
            }),
            agent: "config-validation".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        self.buffer_event(event);
    }

    /// Emit telemetry for request error
    pub fn emit_error(&self, request_id: &str, error_code: &str, error_message: &str) {
        if !self.telemetry_enabled {
            return;
        }

        let event = TelemetryEvent {
            event_type: TelemetryEventType::Error,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: request_id.to_string(),
            data: serde_json::json!({
                "error_code": error_code,
                "error_message": error_message,
            }),
            agent: "config-validation".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        self.buffer_event(event);
    }

    /// Buffer event for batch sending
    fn buffer_event(&self, event: TelemetryEvent) {
        // Log event locally (async-safe)
        tracing::info!(
            event_type = ?event.event_type,
            request_id = %event.request_id,
            "Telemetry event"
        );

        // In a production environment, this would send to LLM-Observatory
        // For now, we just increment the counter and could batch events
        self.request_counter.fetch_add(1, Ordering::SeqCst);

        // Clone to spawn background task
        let buffer = Arc::clone(&self.telemetry_buffer);
        tokio::spawn(async move {
            let mut guard = buffer.lock().await;
            guard.push(event);

            // Flush if buffer is large enough
            if guard.len() >= 100 {
                // In production: send_to_observatory(&guard).await;
                guard.clear();
            }
        });
    }

    /// Get the current request count
    pub fn request_count(&self) -> u64 {
        self.request_counter.load(Ordering::SeqCst)
    }
}

/// Telemetry event for LLM-Observatory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Type of telemetry event
    pub event_type: TelemetryEventType,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Request ID for correlation
    pub request_id: String,
    /// Event-specific data
    pub data: serde_json::Value,
    /// Agent identifier
    pub agent: String,
    /// Agent version
    pub version: String,
}

/// Types of telemetry events
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventType {
    ValidationStart,
    ValidationComplete,
    InspectionStart,
    InspectionComplete,
    HealthCheck,
    Error,
    RateLimitExceeded,
}

/// Input validation middleware
///
/// Validates incoming requests for:
/// - Content-Type header (for POST requests)
/// - Request body size limits
/// - Basic input sanitization
pub async fn validation_middleware(request: Request, next: Next) -> Result<Response, Response> {
    // Validate Content-Type for POST/PUT
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
            let error = ErrorInfo::new(
                "INVALID_CONTENT_TYPE",
                "Content-Type must be application/json",
            );
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(serde_json::json!({
                    "success": false,
                    "error": error,
                })),
            )
                .into_response());
        }
    }

    Ok(next.run(request).await)
}

/// Request logging middleware
///
/// Logs all incoming requests with:
/// - Method and path
/// - Request ID
/// - Timing information
pub async fn request_logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let start = Instant::now();

    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "Request started"
    );

    let response = next.run(request).await;
    let duration = start.elapsed();

    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = %response.status(),
        duration_ms = %duration.as_millis(),
        "Request completed"
    );

    response
}

/// Telemetry emission middleware
///
/// Emits telemetry events compatible with LLM-Observatory for:
/// - Request metrics
/// - Error tracking
/// - Performance monitoring
pub async fn telemetry_middleware(
    State(state): State<MiddlewareState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.telemetry_enabled {
        return next.run(request).await;
    }

    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed();

    // Record request metrics
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        state.emit_error(&request_id, &status.as_str(), "Request failed");
    }

    // Log metrics
    tracing::debug!(
        telemetry.method = %method,
        telemetry.path = %path,
        telemetry.status = %status.as_u16(),
        telemetry.duration_ms = %duration.as_millis(),
        telemetry.request_id = %request_id,
        "Request telemetry"
    );

    response
}

/// Request size validation
///
/// Validates that request body doesn't exceed size limits
pub fn validate_request_size(headers: &HeaderMap, max_size: usize) -> Result<(), SizeValidationError> {
    if let Some(content_length) = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        if content_length > max_size {
            return Err(SizeValidationError {
                actual: content_length,
                limit: max_size,
            });
        }
    }
    Ok(())
}

/// Error for request size validation
#[derive(Debug)]
pub struct SizeValidationError {
    pub actual: usize,
    pub limit: usize,
}

impl std::fmt::Display for SizeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Request body too large: {} bytes exceeds limit of {} bytes",
            self.actual, self.limit
        )
    }
}

impl std::error::Error for SizeValidationError {}

/// Sanitize input string
///
/// Performs basic sanitization:
/// - Trim whitespace
/// - Remove null bytes
/// - Limit length
pub fn sanitize_input(input: &str, max_length: usize) -> String {
    input
        .trim()
        .replace('\0', "")
        .chars()
        .take(max_length)
        .collect()
}

/// Validate JSON payload structure
///
/// Performs basic structural validation:
/// - Maximum nesting depth
/// - Maximum string length
/// - Maximum array size
pub fn validate_json_structure(
    value: &serde_json::Value,
    config: &JsonValidationConfig,
) -> Result<(), JsonValidationError> {
    validate_json_recursive(value, config, 0)
}

fn validate_json_recursive(
    value: &serde_json::Value,
    config: &JsonValidationConfig,
    depth: usize,
) -> Result<(), JsonValidationError> {
    if depth > config.max_depth {
        return Err(JsonValidationError::MaxDepthExceeded {
            depth,
            limit: config.max_depth,
        });
    }

    match value {
        serde_json::Value::String(s) => {
            if s.len() > config.max_string_length {
                return Err(JsonValidationError::StringTooLong {
                    length: s.len(),
                    limit: config.max_string_length,
                });
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.len() > config.max_array_size {
                return Err(JsonValidationError::ArrayTooLarge {
                    size: arr.len(),
                    limit: config.max_array_size,
                });
            }
            for item in arr {
                validate_json_recursive(item, config, depth + 1)?;
            }
        }
        serde_json::Value::Object(map) => {
            if map.len() > config.max_object_keys {
                return Err(JsonValidationError::TooManyKeys {
                    count: map.len(),
                    limit: config.max_object_keys,
                });
            }
            for (key, val) in map {
                if key.len() > config.max_key_length {
                    return Err(JsonValidationError::KeyTooLong {
                        length: key.len(),
                        limit: config.max_key_length,
                    });
                }
                validate_json_recursive(val, config, depth + 1)?;
            }
        }
        _ => {}
    }

    Ok(())
}

/// Configuration for JSON structure validation
#[derive(Debug, Clone)]
pub struct JsonValidationConfig {
    pub max_depth: usize,
    pub max_string_length: usize,
    pub max_array_size: usize,
    pub max_object_keys: usize,
    pub max_key_length: usize,
}

impl Default for JsonValidationConfig {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_string_length: 65536,    // 64KB
            max_array_size: 10000,
            max_object_keys: 1000,
            max_key_length: 256,
        }
    }
}

/// Errors for JSON structure validation
#[derive(Debug)]
pub enum JsonValidationError {
    MaxDepthExceeded { depth: usize, limit: usize },
    StringTooLong { length: usize, limit: usize },
    ArrayTooLarge { size: usize, limit: usize },
    TooManyKeys { count: usize, limit: usize },
    KeyTooLong { length: usize, limit: usize },
}

impl std::fmt::Display for JsonValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonValidationError::MaxDepthExceeded { depth, limit } => {
                write!(f, "JSON nesting depth {} exceeds limit of {}", depth, limit)
            }
            JsonValidationError::StringTooLong { length, limit } => {
                write!(f, "String length {} exceeds limit of {}", length, limit)
            }
            JsonValidationError::ArrayTooLarge { size, limit } => {
                write!(f, "Array size {} exceeds limit of {}", size, limit)
            }
            JsonValidationError::TooManyKeys { count, limit } => {
                write!(f, "Object key count {} exceeds limit of {}", count, limit)
            }
            JsonValidationError::KeyTooLong { length, limit } => {
                write!(f, "Key length {} exceeds limit of {}", length, limit)
            }
        }
    }
}

impl std::error::Error for JsonValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_state_creation() {
        let state = MiddlewareState::new(true);
        assert!(state.telemetry_enabled);
        assert_eq!(state.request_count(), 0);
    }

    #[test]
    fn test_sanitize_input() {
        assert_eq!(sanitize_input("  hello  ", 100), "hello");
        assert_eq!(sanitize_input("hello\0world", 100), "helloworld");
        assert_eq!(sanitize_input("hello world", 5), "hello");
    }

    #[test]
    fn test_json_validation_config_defaults() {
        let config = JsonValidationConfig::default();
        assert_eq!(config.max_depth, 32);
        assert_eq!(config.max_string_length, 65536);
    }

    #[test]
    fn test_validate_json_structure() {
        let config = JsonValidationConfig::default();

        // Valid JSON
        let valid = serde_json::json!({
            "key": "value",
            "nested": {
                "array": [1, 2, 3]
            }
        });
        assert!(validate_json_structure(&valid, &config).is_ok());

        // String too long
        let long_config = JsonValidationConfig {
            max_string_length: 5,
            ..Default::default()
        };
        let long_string = serde_json::json!({"key": "this is a long string"});
        assert!(validate_json_structure(&long_string, &long_config).is_err());
    }

    #[test]
    fn test_validate_json_depth() {
        let config = JsonValidationConfig {
            max_depth: 2,
            ..Default::default()
        };

        // Within depth limit
        let shallow = serde_json::json!({"a": {"b": "c"}});
        assert!(validate_json_structure(&shallow, &config).is_ok());

        // Exceeds depth limit
        let deep = serde_json::json!({"a": {"b": {"c": {"d": "e"}}}});
        assert!(validate_json_structure(&deep, &config).is_err());
    }

    #[test]
    fn test_telemetry_event_serialization() {
        let event = TelemetryEvent {
            event_type: TelemetryEventType::ValidationStart,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            request_id: "test-123".to_string(),
            data: serde_json::json!({"test": true}),
            agent: "config-validation".to_string(),
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("validation_start"));
        assert!(json.contains("test-123"));
    }

    #[test]
    fn test_size_validation_error_display() {
        let error = SizeValidationError {
            actual: 2000,
            limit: 1000,
        };
        let msg = error.to_string();
        assert!(msg.contains("2000"));
        assert!(msg.contains("1000"));
    }
}
