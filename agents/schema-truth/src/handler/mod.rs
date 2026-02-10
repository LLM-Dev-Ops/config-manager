//! HTTP handler for Schema Truth Agent
//!
//! Edge function entry point for Cloud Run deployment.

use agentics_span::{ExecutionContextExtractor, ExecutionEnvelope, SpanTreeBuilder};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::contracts::*;
use crate::engine::SchemaValidationEngine;
use crate::telemetry::TelemetryEmitter;

/// Application state
pub struct AppState {
    pub engine: SchemaValidationEngine,
    pub telemetry: TelemetryEmitter,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: SchemaValidationEngine::new(),
            telemetry: TelemetryEmitter::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the router
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/schema/validate", post(validate_schema))
        .route("/api/v1/schema/check", post(check_schema))
        // Instrumented execution endpoint (requires X-Parent-Span-Id header)
        .route(
            "/api/v1/execution/schema/validate",
            post(validate_schema_instrumented),
        )
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        agent_id: SchemaViolationSignal::AGENT_ID.to_string(),
        agent_version: SchemaViolationSignal::AGENT_VERSION.to_string(),
    })
}

/// Validate schema endpoint
async fn validate_schema(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ValidateSchemaRequest>,
) -> Result<Json<ApiResponse<SchemaValidationOutput>>, (StatusCode, Json<ApiError>)> {
    // Create input
    let input = match SchemaValidationEngine::create_input(
        request.schema,
        request.requested_by.unwrap_or_else(|| "anonymous".to_string()),
    ) {
        Ok(input) => input,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: "InvalidInput".to_string(),
                    message: e,
                    request_id: None,
                }),
            ));
        }
    };

    let request_id = input.request_id;
    let inputs_hash = SchemaValidationEngine::compute_inputs_hash(&input);

    // Validate
    let output = state.engine.validate(&input).await;

    // Emit telemetry
    let signal = SchemaViolationSignal::from_validation(
        inputs_hash,
        &output,
        request_id.to_string(),
    );
    if let Err(e) = state.telemetry.emit(signal).await {
        tracing::warn!("Failed to emit telemetry: {}", e);
    }

    Ok(Json(ApiResponse {
        success: output.is_valid,
        data: output,
        request_id,
    }))
}

/// Quick schema check endpoint (no telemetry)
async fn check_schema(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ValidateSchemaRequest>,
) -> Result<Json<CheckResponse>, (StatusCode, Json<ApiError>)> {
    let input = match SchemaValidationEngine::create_input(
        request.schema,
        request.requested_by.unwrap_or_else(|| "anonymous".to_string()),
    ) {
        Ok(input) => input,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError {
                    error: "InvalidInput".to_string(),
                    message: e,
                    request_id: None,
                }),
            ));
        }
    };

    let output = state.engine.validate(&input).await;

    Ok(Json(CheckResponse {
        is_valid: output.is_valid,
        violation_count: output.violations.len(),
        warning_count: output.warnings.len(),
        coverage: output.coverage,
        duration_ms: output.duration_ms,
    }))
}

/// Instrumented schema validation endpoint.
///
/// Requires `X-Parent-Span-Id` header (rejects with 400 if missing).
/// Creates repo-level and agent-level execution spans, attaches the
/// validation output as an artifact, and returns the span tree in
/// an `ExecutionEnvelope`.
async fn validate_schema_instrumented(
    exec_ctx: ExecutionContextExtractor,
    State(state): State<Arc<AppState>>,
    Json(request): Json<ValidateSchemaRequest>,
) -> Result<
    Json<ExecutionEnvelope<SchemaValidationOutput>>,
    (StatusCode, Json<serde_json::Value>),
> {
    let ctx = exec_ctx.0;
    let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
    let mut agent_span = tree.start_agent_span("schema-truth");

    // Create input
    let input = match SchemaValidationEngine::create_input(
        request.schema,
        request.requested_by.unwrap_or_else(|| "anonymous".to_string()),
    ) {
        Ok(input) => input,
        Err(e) => {
            agent_span.fail(e.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(e.clone());
            return Ok(Json(ExecutionEnvelope::failure(e, span_tree)));
        }
    };

    let request_id = input.request_id;
    let inputs_hash = SchemaValidationEngine::compute_inputs_hash(&input);

    // Validate
    let output = state.engine.validate(&input).await;

    // Emit existing ruvector telemetry (preserved)
    let signal = SchemaViolationSignal::from_validation(
        inputs_hash,
        &output,
        request_id.to_string(),
    );
    if let Err(e) = state.telemetry.emit(signal).await {
        tracing::warn!("Failed to emit telemetry: {}", e);
    }

    // Attach output as artifact to agent span
    if let Ok(artifact) = serde_json::to_value(&output) {
        agent_span.attach_artifact(artifact);
    }

    // Agent span is Completed even if schema is invalid —
    // the agent itself ran successfully, the finding is an artifact.
    agent_span.complete();
    tree.add_completed_agent_span(agent_span);
    let span_tree = tree.finalize();

    Ok(Json(ExecutionEnvelope::success(output, span_tree)))
}

/// Health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub agent_id: String,
    pub agent_version: String,
}

/// Validate schema request
#[derive(Debug, Deserialize)]
pub struct ValidateSchemaRequest {
    pub schema: serde_json::Value,
    pub requested_by: Option<String>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    pub request_id: Uuid,
}

/// Check response (lightweight)
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub is_valid: bool,
    pub violation_count: usize,
    pub warning_count: usize,
    pub coverage: f64,
    pub duration_ms: u64,
}

/// API error
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub request_id: Option<Uuid>,
}
