//! Route definitions for Config Validation Agent
//!
//! This module defines the HTTP routes for the validation agent:
//! - POST /validate - Full configuration validation
//! - POST /inspect - Quick schema inspection
//! - GET /health - Health check endpoint
//! - GET /schema - Return validation schemas
//!
//! All routes return machine-readable JSON responses and emit telemetry
//! compatible with LLM-Observatory.

use agentics_span::{ExecutionContextExtractor, ExecutionEnvelope, SpanTreeBuilder};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use super::{
    ApiResponse, ComponentHealth, ConfigStructure, ErrorInfo, FieldInfo, HealthResponse,
    HealthStatus, InspectionRequest, InspectionResult, MiddlewareState, SchemaSuggestion,
    ValidationError, ValidationOptions, ValidationRequest, ValidationResult, ValidationStats,
    ValidationWarning,
};

/// Handler state shared across all routes
#[derive(Clone)]
pub struct HandlerState {
    /// Available validation schemas
    pub schemas: Arc<HashMap<String, ValidationSchema>>,
    /// Start time for uptime calculation
    pub start_time: Instant,
}

impl HandlerState {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(Self::load_default_schemas()),
            start_time: Instant::now(),
        }
    }

    fn load_default_schemas() -> HashMap<String, ValidationSchema> {
        let mut schemas = HashMap::new();

        // LLM Configuration Schema
        schemas.insert(
            "llm-config-v1".to_string(),
            ValidationSchema {
                id: "llm-config-v1".to_string(),
                name: "LLM Configuration Schema v1".to_string(),
                version: "1.0.0".to_string(),
                description: "Schema for LLM configuration entries".to_string(),
                fields: vec![
                    SchemaField {
                        path: "namespace".to_string(),
                        field_type: "string".to_string(),
                        required: true,
                        pattern: Some(r"^[a-z][a-z0-9-/]*$".to_string()),
                        description: "Configuration namespace".to_string(),
                    },
                    SchemaField {
                        path: "key".to_string(),
                        field_type: "string".to_string(),
                        required: true,
                        pattern: Some(r"^[a-zA-Z][a-zA-Z0-9_-]*$".to_string()),
                        description: "Configuration key".to_string(),
                    },
                    SchemaField {
                        path: "value".to_string(),
                        field_type: "any".to_string(),
                        required: true,
                        pattern: None,
                        description: "Configuration value".to_string(),
                    },
                    SchemaField {
                        path: "environment".to_string(),
                        field_type: "string".to_string(),
                        required: false,
                        pattern: Some(
                            r"^(development|staging|production|base)$".to_string(),
                        ),
                        description: "Target environment".to_string(),
                    },
                ],
            },
        );

        // Provider Configuration Schema
        schemas.insert(
            "provider-config-v1".to_string(),
            ValidationSchema {
                id: "provider-config-v1".to_string(),
                name: "Provider Configuration Schema v1".to_string(),
                version: "1.0.0".to_string(),
                description: "Schema for external provider configurations".to_string(),
                fields: vec![
                    SchemaField {
                        path: "provider_type".to_string(),
                        field_type: "string".to_string(),
                        required: true,
                        pattern: Some(r"^(vault|aws|gcp|azure|env)$".to_string()),
                        description: "Provider type".to_string(),
                    },
                    SchemaField {
                        path: "endpoint".to_string(),
                        field_type: "string".to_string(),
                        required: false,
                        pattern: Some(r"^https?://".to_string()),
                        description: "Provider endpoint URL".to_string(),
                    },
                    SchemaField {
                        path: "auth".to_string(),
                        field_type: "object".to_string(),
                        required: false,
                        pattern: None,
                        description: "Authentication configuration".to_string(),
                    },
                ],
            },
        );

        schemas
    }
}

impl Default for HandlerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSchema {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub fields: Vec<SchemaField>,
}

/// Schema field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub path: String,
    pub field_type: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub description: String,
}

/// API error types
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    InternalError(String),
    ValidationFailed(Vec<ValidationError>),
}

impl ApiError {
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::InternalError(_) => "INTERNAL_ERROR",
            ApiError::ValidationFailed(_) => "VALIDATION_FAILED",
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ValidationFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_info = match &self {
            ApiError::BadRequest(msg) => ErrorInfo::new(self.error_code(), msg),
            ApiError::NotFound(msg) => ErrorInfo::new(self.error_code(), msg),
            ApiError::InternalError(msg) => ErrorInfo::new(self.error_code(), msg),
            ApiError::ValidationFailed(errors) => {
                ErrorInfo::new(self.error_code(), "Configuration validation failed")
                    .with_details(serde_json::json!({ "errors": errors }))
            }
        };

        let response = ApiResponse::<()>::error(error_info, uuid::Uuid::new_v4().to_string());

        (status, Json(response)).into_response()
    }
}

/// Create the router with all routes
pub fn create_router(handler_state: HandlerState, middleware_state: MiddlewareState) -> Router {
    Router::new()
        // Validation endpoints
        .route("/validate", post(validate_config))
        .route("/inspect", post(inspect_config))
        // Health and schema endpoints
        .route("/health", get(health_check))
        .route("/schema", get(validation_schema))
        .route("/schema/:schema_id", get(get_schema_by_id))
        // Instrumented execution endpoint (requires X-Parent-Span-Id header)
        .route("/execution/validate", post(validate_config_instrumented))
        // Add state
        .with_state((handler_state, middleware_state))
}

/// Query parameters for schema endpoint
#[derive(Debug, Deserialize)]
pub struct SchemaQuery {
    #[serde(default)]
    pub include_fields: bool,
}

/// POST /validate - Full configuration validation
///
/// Validates a configuration against a schema and returns detailed results.
/// This endpoint is deterministic and stateless.
pub async fn validate_config(
    State((state, middleware_state)): State<(HandlerState, MiddlewareState)>,
    Json(request): Json<ValidationRequest>,
) -> Result<Json<ApiResponse<ValidationResult>>, ApiError> {
    let start_time = Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();

    // Emit telemetry for request start
    middleware_state.emit_validation_start(&request_id, &request);

    // Determine which schema to use
    let schema_id = request.schema.as_deref().unwrap_or("llm-config-v1");
    let schema = state
        .schemas
        .get(schema_id)
        .ok_or_else(|| ApiError::NotFound(format!("Schema '{}' not found", schema_id)))?;

    // Perform validation
    let (errors, warnings) = validate_against_schema(&request.config, schema, &request.options);

    let duration_us = start_time.elapsed().as_micros() as u64;

    let result = ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
        schema_used: schema_id.to_string(),
        stats: ValidationStats {
            fields_validated: count_fields(&request.config),
            rules_applied: schema.fields.len(),
            duration_us,
        },
    };

    // Emit telemetry for validation complete
    middleware_state.emit_validation_complete(&request_id, &result);

    let response = ApiResponse::success(result, request_id);
    Ok(Json(response))
}

/// POST /execution/validate - Instrumented configuration validation.
///
/// Requires `X-Parent-Span-Id` header (rejects with 400 if missing).
/// Creates repo-level and agent-level execution spans, attaches the
/// validation result as an artifact, and returns the span tree in
/// an `ExecutionEnvelope`.
pub async fn validate_config_instrumented(
    exec_ctx: ExecutionContextExtractor,
    State((state, middleware_state)): State<(HandlerState, MiddlewareState)>,
    Json(request): Json<ValidationRequest>,
) -> Result<Json<ExecutionEnvelope<ValidationResult>>, ApiError> {
    let ctx = exec_ctx.0;
    let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
    let mut agent_span = tree.start_agent_span("config-validation");

    let start_time = Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();

    // Emit existing telemetry (preserved)
    middleware_state.emit_validation_start(&request_id, &request);

    // Determine which schema to use
    let schema_id = request.schema.as_deref().unwrap_or("llm-config-v1");
    let schema = match state.schemas.get(schema_id) {
        Some(s) => s,
        None => {
            let err = format!("Schema '{}' not found", schema_id);
            agent_span.fail(err.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(err.clone());
            return Ok(Json(ExecutionEnvelope::failure(err, span_tree)));
        }
    };

    // Perform validation
    let (errors, warnings) = validate_against_schema(&request.config, schema, &request.options);

    let duration_us = start_time.elapsed().as_micros() as u64;

    let result = ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
        schema_used: schema_id.to_string(),
        stats: ValidationStats {
            fields_validated: count_fields(&request.config),
            rules_applied: schema.fields.len(),
            duration_us,
        },
    };

    // Emit existing telemetry (preserved)
    middleware_state.emit_validation_complete(&request_id, &result);

    // Attach result as artifact to agent span
    if let Ok(artifact) = serde_json::to_value(&result) {
        agent_span.attach_artifact(artifact);
    }

    // Agent span is Completed even if validation found errors —
    // the agent itself ran successfully, the finding is an artifact.
    agent_span.complete();
    tree.add_completed_agent_span(agent_span);
    let span_tree = tree.finalize();

    Ok(Json(ExecutionEnvelope::success(result, span_tree)))
}

/// POST /inspect - Quick schema inspection
///
/// Analyzes a configuration and suggests matching schemas without full validation.
pub async fn inspect_config(
    State((state, middleware_state)): State<(HandlerState, MiddlewareState)>,
    Json(request): Json<InspectionRequest>,
) -> Result<Json<ApiResponse<InspectionResult>>, ApiError> {
    let request_id = uuid::Uuid::new_v4().to_string();

    // Emit telemetry
    middleware_state.emit_inspection_start(&request_id);

    // Analyze structure
    let structure = analyze_structure(&request.config);

    // Find matching schemas
    let suggested_schemas = if request.suggest_schemas {
        find_matching_schemas(&request.config, &state.schemas)
    } else {
        vec![]
    };

    // Detect patterns
    let detected_patterns = detect_config_patterns(&request.config);

    let result = InspectionResult {
        structure,
        suggested_schemas,
        detected_patterns,
    };

    middleware_state.emit_inspection_complete(&request_id, &result);

    let response = ApiResponse::success(result, request_id);
    Ok(Json(response))
}

/// GET /health - Health check endpoint
///
/// Returns the health status of the validation agent.
pub async fn health_check(
    State((state, middleware_state)): State<(HandlerState, MiddlewareState)>,
) -> Json<HealthResponse> {
    let uptime_seconds = state.start_time.elapsed().as_secs();

    // Check component health
    let validation_engine = true; // Always available in stateless mode
    let schema_registry = !state.schemas.is_empty();
    let telemetry = middleware_state.telemetry_enabled;

    let status = if validation_engine && schema_registry {
        HealthStatus::Healthy
    } else if validation_engine {
        HealthStatus::Degraded
    } else {
        HealthStatus::Unhealthy
    };

    Json(HealthResponse {
        status,
        components: ComponentHealth {
            validation_engine,
            schema_registry,
            telemetry,
        },
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// GET /schema - Return validation schemas
///
/// Returns the list of available validation schemas.
pub async fn validation_schema(
    State((state, _)): State<(HandlerState, MiddlewareState)>,
    Query(params): Query<SchemaQuery>,
) -> Json<ApiResponse<Vec<SchemaInfo>>> {
    let request_id = uuid::Uuid::new_v4().to_string();

    let schemas: Vec<SchemaInfo> = state
        .schemas
        .values()
        .map(|s| SchemaInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            version: s.version.clone(),
            description: s.description.clone(),
            field_count: s.fields.len(),
            fields: if params.include_fields {
                Some(s.fields.clone())
            } else {
                None
            },
        })
        .collect();

    let response = ApiResponse::success(schemas, request_id);
    Json(response)
}

/// GET /schema/:schema_id - Get specific schema
pub async fn get_schema_by_id(
    State((state, _)): State<(HandlerState, MiddlewareState)>,
    axum::extract::Path(schema_id): axum::extract::Path<String>,
) -> Result<Json<ApiResponse<ValidationSchema>>, ApiError> {
    let request_id = uuid::Uuid::new_v4().to_string();

    let schema = state
        .schemas
        .get(&schema_id)
        .ok_or_else(|| ApiError::NotFound(format!("Schema '{}' not found", schema_id)))?;

    let response = ApiResponse::success(schema.clone(), request_id);
    Ok(Json(response))
}

/// Schema info for list endpoint
#[derive(Debug, Serialize)]
pub struct SchemaInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub field_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<SchemaField>>,
}

// Helper functions

fn validate_against_schema(
    config: &serde_json::Value,
    schema: &ValidationSchema,
    options: &ValidationOptions,
) -> (Vec<ValidationError>, Vec<ValidationWarning>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for field in &schema.fields {
        let value = get_json_path(config, &field.path);

        // Check required fields
        if field.required && value.is_none() {
            errors.push(ValidationError {
                path: field.path.clone(),
                code: "REQUIRED_FIELD_MISSING".to_string(),
                message: format!("Required field '{}' is missing", field.path),
                expected: Some(field.field_type.clone()),
                actual: None,
            });
            continue;
        }

        // Skip optional fields that are not present
        let Some(value) = value else {
            continue;
        };

        // Type validation
        if !validate_type(value, &field.field_type) {
            errors.push(ValidationError {
                path: field.path.clone(),
                code: "TYPE_MISMATCH".to_string(),
                message: format!(
                    "Field '{}' has wrong type",
                    field.path
                ),
                expected: Some(field.field_type.clone()),
                actual: Some(json_type_name(value).to_string()),
            });
            continue;
        }

        // Pattern validation for strings
        if let Some(pattern) = &field.pattern {
            if let Some(s) = value.as_str() {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if !re.is_match(s) {
                        errors.push(ValidationError {
                            path: field.path.clone(),
                            code: "PATTERN_MISMATCH".to_string(),
                            message: format!(
                                "Field '{}' does not match required pattern",
                                field.path
                            ),
                            expected: Some(pattern.clone()),
                            actual: Some(s.to_string()),
                        });
                    }
                }
            }
        }
    }

    // Check for unknown fields in strict mode
    if options.strict {
        let unknown = find_unknown_fields(config, schema);
        for path in unknown {
            warnings.push(ValidationWarning {
                path,
                code: "UNKNOWN_FIELD".to_string(),
                message: "Field not defined in schema".to_string(),
            });
        }
    }

    (errors, warnings)
}

fn get_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        current = current.get(part)?;
    }

    Some(current)
}

fn validate_type(value: &serde_json::Value, expected: &str) -> bool {
    match expected {
        "string" => value.is_string(),
        "number" | "integer" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "any" => true,
        "null" => value.is_null(),
        _ => true, // Unknown types pass
    }
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn count_fields(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            map.len() + map.values().map(count_fields).sum::<usize>()
        }
        serde_json::Value::Array(arr) => arr.iter().map(count_fields).sum(),
        _ => 0,
    }
}

fn find_unknown_fields(config: &serde_json::Value, schema: &ValidationSchema) -> Vec<String> {
    let schema_paths: std::collections::HashSet<&str> =
        schema.fields.iter().map(|f| f.path.as_str()).collect();

    let mut unknown = Vec::new();
    collect_paths(config, "", &mut |path| {
        if !schema_paths.contains(path.as_str()) && !path.is_empty() {
            unknown.push(path);
        }
    });

    unknown
}

fn collect_paths<F>(value: &serde_json::Value, prefix: &str, collector: &mut F)
where
    F: FnMut(String),
{
    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            collector(path.clone());
            collect_paths(val, &path, collector);
        }
    }
}

fn analyze_structure(config: &serde_json::Value) -> ConfigStructure {
    let mut fields = Vec::new();
    let depth = calculate_depth(config);
    let field_count = count_fields(config);

    collect_field_info(config, "", &mut fields);

    ConfigStructure {
        root_type: json_type_name(config).to_string(),
        fields,
        depth,
        field_count,
    }
}

fn calculate_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            1 + map.values().map(calculate_depth).max().unwrap_or(0)
        }
        serde_json::Value::Array(arr) => {
            1 + arr.iter().map(calculate_depth).max().unwrap_or(0)
        }
        _ => 0,
    }
}

fn collect_field_info(value: &serde_json::Value, prefix: &str, fields: &mut Vec<FieldInfo>) {
    if let serde_json::Value::Object(map) = value {
        for (key, val) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            let sample = match val {
                serde_json::Value::String(s) if s.len() <= 50 => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            };

            fields.push(FieldInfo {
                path: path.clone(),
                field_type: json_type_name(val).to_string(),
                required: true, // Assume present fields are required
                sample,
            });

            collect_field_info(val, &path, fields);
        }
    }
}

fn find_matching_schemas(
    config: &serde_json::Value,
    schemas: &HashMap<String, ValidationSchema>,
) -> Vec<SchemaSuggestion> {
    let mut suggestions = Vec::new();

    for (id, schema) in schemas {
        let (matching, mismatched) = calculate_schema_match(config, schema);
        let total = schema.fields.len();
        let confidence = if total > 0 {
            matching.len() as f64 / total as f64
        } else {
            0.0
        };

        if confidence > 0.3 {
            suggestions.push(SchemaSuggestion {
                schema_id: id.clone(),
                name: schema.name.clone(),
                confidence,
                matching_fields: matching,
                mismatched_fields: mismatched,
            });
        }
    }

    suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    suggestions
}

fn calculate_schema_match(
    config: &serde_json::Value,
    schema: &ValidationSchema,
) -> (Vec<String>, Vec<String>) {
    let mut matching = Vec::new();
    let mut mismatched = Vec::new();

    for field in &schema.fields {
        if let Some(value) = get_json_path(config, &field.path) {
            if validate_type(value, &field.field_type) {
                matching.push(field.path.clone());
            } else {
                mismatched.push(field.path.clone());
            }
        } else if field.required {
            mismatched.push(field.path.clone());
        }
    }

    (matching, mismatched)
}

fn detect_config_patterns(config: &serde_json::Value) -> Vec<String> {
    let mut patterns = Vec::new();

    if let serde_json::Value::Object(map) = config {
        // Detect common configuration patterns
        if map.contains_key("namespace") && map.contains_key("key") {
            patterns.push("llm-config-entry".to_string());
        }
        if map.contains_key("provider_type") || map.contains_key("endpoint") {
            patterns.push("provider-config".to_string());
        }
        if map.contains_key("environment") {
            patterns.push("environment-aware".to_string());
        }
        if map.contains_key("version") {
            patterns.push("versioned".to_string());
        }
        if map.contains_key("auth") || map.contains_key("credentials") {
            patterns.push("authenticated".to_string());
        }
    }

    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_state_creation() {
        let state = HandlerState::new();
        assert!(!state.schemas.is_empty());
        assert!(state.schemas.contains_key("llm-config-v1"));
    }

    #[test]
    fn test_validate_type() {
        assert!(validate_type(&serde_json::json!("test"), "string"));
        assert!(validate_type(&serde_json::json!(42), "number"));
        assert!(validate_type(&serde_json::json!(true), "boolean"));
        assert!(validate_type(&serde_json::json!([1, 2, 3]), "array"));
        assert!(validate_type(&serde_json::json!({}), "object"));
        assert!(validate_type(&serde_json::json!(null), "any"));
    }

    #[test]
    fn test_validate_against_schema() {
        let state = HandlerState::new();
        let schema = state.schemas.get("llm-config-v1").unwrap();
        let options = ValidationOptions::default();

        // Valid config
        let valid_config = serde_json::json!({
            "namespace": "test/namespace",
            "key": "test_key",
            "value": "test_value"
        });

        let (errors, _) = validate_against_schema(&valid_config, schema, &options);
        assert!(errors.is_empty());

        // Missing required field
        let invalid_config = serde_json::json!({
            "namespace": "test/namespace"
        });

        let (errors, _) = validate_against_schema(&invalid_config, schema, &options);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.code == "REQUIRED_FIELD_MISSING"));
    }

    #[test]
    fn test_json_path_extraction() {
        let value = serde_json::json!({
            "a": {
                "b": {
                    "c": "value"
                }
            }
        });

        assert_eq!(
            get_json_path(&value, "a.b.c"),
            Some(&serde_json::json!("value"))
        );
        assert!(get_json_path(&value, "a.b.d").is_none());
    }

    #[test]
    fn test_analyze_structure() {
        let config = serde_json::json!({
            "namespace": "test",
            "key": "mykey",
            "nested": {
                "field": "value"
            }
        });

        let structure = analyze_structure(&config);
        assert_eq!(structure.root_type, "object");
        assert!(structure.field_count > 0);
        assert!(structure.depth > 1);
    }

    #[test]
    fn test_detect_config_patterns() {
        let config = serde_json::json!({
            "namespace": "test",
            "key": "mykey",
            "environment": "production",
            "version": "1.0.0"
        });

        let patterns = detect_config_patterns(&config);
        assert!(patterns.contains(&"llm-config-entry".to_string()));
        assert!(patterns.contains(&"environment-aware".to_string()));
        assert!(patterns.contains(&"versioned".to_string()));
    }

    #[test]
    fn test_api_error_responses() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(error.error_code(), "BAD_REQUEST");

        let error = ApiError::NotFound("Resource not found".to_string());
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }
}
