//! HTTP handler for Integration Health Agent
//!
//! Edge function entry point for Cloud Run deployment.

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
use crate::engine::HealthCheckEngine;
use crate::telemetry::TelemetryEmitter;

/// Application state
pub struct AppState {
    pub engine: HealthCheckEngine,
    pub telemetry: TelemetryEmitter,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: HealthCheckEngine::new(),
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
        .route("/api/v1/integration/check", post(check_health))
        .route("/api/v1/integration/probe", post(probe_adapter))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        agent_id: IntegrationHealthSignal::AGENT_ID.to_string(),
        agent_version: IntegrationHealthSignal::AGENT_VERSION.to_string(),
    })
}

/// Check health of adapters
async fn check_health(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CheckHealthRequest>,
) -> Result<Json<ApiResponse<IntegrationHealthOutput>>, (StatusCode, Json<ApiError>)> {
    // Validate adapters
    if request.adapters.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "InvalidInput".to_string(),
                message: "At least one adapter must be specified".to_string(),
                request_id: None,
            }),
        ));
    }

    // Create input
    let mut input = HealthCheckEngine::create_input(
        request.adapters,
        request.requested_by.unwrap_or_else(|| "anonymous".to_string()),
    );

    if let Some(opts) = request.options {
        input.options = opts;
    }

    let request_id = input.request_id;
    let inputs_hash = HealthCheckEngine::compute_inputs_hash(&input);

    // Run health checks
    let output = state.engine.check(&input).await;

    // Emit telemetry
    let signal = IntegrationHealthSignal::from_health_check(
        inputs_hash,
        &output,
        request_id.to_string(),
    );
    if let Err(e) = state.telemetry.emit(signal).await {
        tracing::warn!("Failed to emit telemetry: {}", e);
    }

    Ok(Json(ApiResponse {
        success: output.is_healthy,
        data: output,
        request_id,
    }))
}

/// Quick probe of a single adapter (no telemetry)
async fn probe_adapter(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ProbeRequest>,
) -> Result<Json<ProbeResponse>, (StatusCode, Json<ApiError>)> {
    let input = HealthCheckEngine::create_input(vec![request.adapter], "probe".to_string());

    let output = state.engine.check(&input).await;

    let result = output.adapter_results.first();

    Ok(Json(ProbeResponse {
        adapter_id: result.map(|r| r.adapter_id.clone()).unwrap_or_default(),
        status: result.map(|r| r.status).unwrap_or(HealthStatus::Unknown),
        latency_ms: result.map(|r| r.latency_ms).unwrap_or(0),
        error: result.and_then(|r| r.error.clone()),
    }))
}

/// Health response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub agent_id: String,
    pub agent_version: String,
}

/// Check health request
#[derive(Debug, Deserialize)]
pub struct CheckHealthRequest {
    pub adapters: Vec<AdapterConfig>,
    pub options: Option<HealthCheckOptions>,
    pub requested_by: Option<String>,
}

/// Probe request
#[derive(Debug, Deserialize)]
pub struct ProbeRequest {
    pub adapter: AdapterConfig,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    pub request_id: Uuid,
}

/// Probe response
#[derive(Debug, Serialize)]
pub struct ProbeResponse {
    pub adapter_id: String,
    pub status: HealthStatus,
    pub latency_ms: u64,
    pub error: Option<String>,
}

/// API error
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub request_id: Option<Uuid>,
}
