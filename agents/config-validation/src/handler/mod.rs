//! Handler module for Config Validation Agent
//!
//! This module provides the HTTP handler infrastructure for the Config Validation
//! Agent deployed as a Google Cloud Edge Function. It follows Axum patterns
//! from the llm-config-api crate with adaptations for serverless execution.
//!
//! ## Architecture
//!
//! The handler module is organized into:
//! - `edge_function`: Google Cloud Edge Function entry point and lifecycle
//! - `routes`: Route definitions for validation endpoints
//! - `middleware`: Request processing, validation, and telemetry emission
//!
//! ## Design Principles
//!
//! - **Stateless Execution**: No state persisted between invocations
//! - **Deterministic Behavior**: Same input produces same output
//! - **Machine-Readable Responses**: JSON format for all responses
//! - **Telemetry Compatible**: Emits events compatible with LLM-Observatory
//! - **No Enforcement**: Validation only, no workflow triggering

pub mod edge_function;
pub mod middleware;
pub mod routes;

pub use edge_function::{handle_request, EdgeFunctionConfig, EdgeFunctionError};
pub use middleware::{
    request_logging_middleware, telemetry_middleware, validation_middleware, MiddlewareState,
};
pub use routes::{
    create_router, health_check, inspect_config, validate_config, validation_schema, ApiError,
    HandlerState,
};

use serde::{Deserialize, Serialize};

/// Standard API response wrapper for validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Whether the operation was successful
    pub success: bool,
    /// Response data (present on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error information (present on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    /// Request metadata for tracing
    pub metadata: ResponseMetadata,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T, request_id: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: ResponseMetadata::new(request_id),
        }
    }

    /// Create an error response
    pub fn error(error: ErrorInfo, request_id: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(error),
            metadata: ResponseMetadata::new(request_id),
        }
    }
}

/// Error information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code for programmatic handling
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorInfo {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Response metadata for tracing and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    /// Unique request identifier
    pub request_id: String,
    /// Timestamp of response generation (ISO 8601)
    pub timestamp: String,
    /// Agent version
    pub version: String,
    /// Processing duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl ResponseMetadata {
    pub fn new(request_id: String) -> Self {
        Self {
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            duration_ms: None,
        }
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Configuration validation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    /// Configuration data to validate
    pub config: serde_json::Value,
    /// Schema to validate against (optional, uses default if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Validation options
    #[serde(default)]
    pub options: ValidationOptions,
}

/// Options for validation behavior
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationOptions {
    /// Whether to validate strictly (fail on unknown fields)
    #[serde(default)]
    pub strict: bool,
    /// Maximum depth for nested validation
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// Whether to collect all errors or fail fast
    #[serde(default)]
    pub collect_all_errors: bool,
    /// Custom validation rules to apply
    #[serde(default)]
    pub custom_rules: Vec<String>,
}

fn default_max_depth() -> usize {
    32
}

/// Validation result returned by the validate endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the configuration is valid
    pub valid: bool,
    /// List of validation errors (if any)
    #[serde(default)]
    pub errors: Vec<ValidationError>,
    /// List of validation warnings (if any)
    #[serde(default)]
    pub warnings: Vec<ValidationWarning>,
    /// Schema used for validation
    pub schema_used: String,
    /// Validation statistics
    pub stats: ValidationStats,
}

/// Individual validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON path to the invalid field
    pub path: String,
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Expected value or type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Actual value or type found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

/// Validation warning (non-fatal issues)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// JSON path to the field with warning
    pub path: String,
    /// Warning code
    pub code: String,
    /// Warning message
    pub message: String,
}

/// Statistics about the validation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStats {
    /// Number of fields validated
    pub fields_validated: usize,
    /// Number of rules applied
    pub rules_applied: usize,
    /// Validation duration in microseconds
    pub duration_us: u64,
}

/// Schema inspection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionRequest {
    /// Configuration data to inspect
    pub config: serde_json::Value,
    /// Whether to include type inference
    #[serde(default = "default_true")]
    pub infer_types: bool,
    /// Whether to suggest matching schemas
    #[serde(default = "default_true")]
    pub suggest_schemas: bool,
}

fn default_true() -> bool {
    true
}

/// Schema inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    /// Inferred structure of the configuration
    pub structure: ConfigStructure,
    /// Suggested schemas that might match
    #[serde(default)]
    pub suggested_schemas: Vec<SchemaSuggestion>,
    /// Detected configuration patterns
    #[serde(default)]
    pub detected_patterns: Vec<String>,
}

/// Structure of a configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStructure {
    /// Root type of the configuration
    pub root_type: String,
    /// Field definitions
    pub fields: Vec<FieldInfo>,
    /// Nesting depth
    pub depth: usize,
    /// Total field count
    pub field_count: usize,
}

/// Information about a configuration field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    /// Field path
    pub path: String,
    /// Inferred type
    pub field_type: String,
    /// Whether the field appears required
    pub required: bool,
    /// Sample value (if simple type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
}

/// Schema suggestion from inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSuggestion {
    /// Schema identifier
    pub schema_id: String,
    /// Human-readable schema name
    pub name: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Fields that match
    pub matching_fields: Vec<String>,
    /// Fields that don't match
    pub mismatched_fields: Vec<String>,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status
    pub status: HealthStatus,
    /// Component-level health
    pub components: ComponentHealth,
    /// Timestamp of health check
    pub timestamp: String,
    /// Agent version
    pub version: String,
}

/// Health status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component-level health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Validation engine status
    pub validation_engine: bool,
    /// Schema registry status
    pub schema_registry: bool,
    /// Telemetry emitter status
    pub telemetry: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response: ApiResponse<String> =
            ApiResponse::success("test data".to_string(), "req-123".to_string());
        assert!(response.success);
        assert_eq!(response.data, Some("test data".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let error = ErrorInfo::new("VALIDATION_FAILED", "Invalid configuration");
        let response = ApiResponse::<()>::error(error, "req-456".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_validation_options_defaults() {
        let options = ValidationOptions::default();
        assert!(!options.strict);
        assert_eq!(options.max_depth, 32);
        assert!(!options.collect_all_errors);
        assert!(options.custom_rules.is_empty());
    }

    #[test]
    fn test_error_info_with_details() {
        let error = ErrorInfo::new("TEST_ERROR", "Test message")
            .with_details(serde_json::json!({"key": "value"}));
        assert!(error.details.is_some());
    }

    #[test]
    fn test_response_metadata() {
        let metadata = ResponseMetadata::new("req-789".to_string()).with_duration(42);
        assert_eq!(metadata.request_id, "req-789");
        assert_eq!(metadata.duration_ms, Some(42));
    }
}
