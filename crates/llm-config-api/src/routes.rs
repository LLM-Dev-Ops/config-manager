//! REST API routes

use agentics_span::{ExecutionContextExtractor, ExecutionEnvelope, SpanTreeBuilder};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use llm_config_core::{ConfigEntry, ConfigManager, ConfigValue, Environment};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// API state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub manager: Arc<ConfigManager>,
}

/// Standard API error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
        };

        let body = Json(ErrorResponse {
            error: status.canonical_reason().unwrap_or("Unknown").to_string(),
            message: error_message,
        });

        (status, body).into_response()
    }
}

/// API error types
#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    InternalError(String),
    Unauthorized(String),
}

impl From<llm_config_core::ConfigError> for ApiError {
    fn from(err: llm_config_core::ConfigError) -> Self {
        ApiError::InternalError(err.to_string())
    }
}

/// Query parameters for get config
#[derive(Debug, Deserialize)]
pub struct GetConfigQuery {
    #[serde(default)]
    env: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // Reserved for future use
    with_overrides: bool,
}

/// Request body for set config
#[derive(Debug, Deserialize)]
pub struct SetConfigRequest {
    pub value: serde_json::Value,
    pub env: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default)]
    pub secret: bool,
}

fn default_user() -> String {
    "api-user".to_string()
}

/// Response for config operations
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub id: String,
    pub namespace: String,
    pub key: String,
    pub value: serde_json::Value,
    pub environment: String,
    pub version: u64,
    pub metadata: ConfigMetadataResponse,
}

#[derive(Debug, Serialize)]
pub struct ConfigMetadataResponse {
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
    pub updated_by: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

impl From<ConfigEntry> for ConfigResponse {
    fn from(entry: ConfigEntry) -> Self {
        Self {
            id: entry.id.to_string(),
            namespace: entry.namespace,
            key: entry.key,
            value: config_value_to_json(&entry.value),
            environment: entry.environment.to_string(),
            version: entry.version,
            metadata: ConfigMetadataResponse {
                created_at: entry.metadata.created_at.to_rfc3339(),
                created_by: entry.metadata.created_by,
                updated_at: entry.metadata.updated_at.to_rfc3339(),
                updated_by: entry.metadata.updated_by,
                tags: entry.metadata.tags,
                description: entry.metadata.description,
            },
        }
    }
}

fn config_value_to_json(value: &ConfigValue) -> serde_json::Value {
    match value {
        ConfigValue::String(s) => serde_json::Value::String(s.clone()),
        ConfigValue::Integer(i) => serde_json::Value::Number((*i).into()),
        ConfigValue::Float(f) => serde_json::Value::Number(
            serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0)),
        ),
        ConfigValue::Boolean(b) => serde_json::Value::Bool(*b),
        ConfigValue::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(config_value_to_json).collect())
        }
        ConfigValue::Object(map) => {
            let obj: HashMap<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), config_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj.into_iter().collect())
        }
        ConfigValue::Secret(_) => serde_json::Value::String("<encrypted>".to_string()),
    }
}

fn json_to_config_value(value: &serde_json::Value) -> Result<ConfigValue, ApiError> {
    Ok(match value {
        serde_json::Value::String(s) => ConfigValue::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ConfigValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                ConfigValue::Float(f)
            } else {
                return Err(ApiError::BadRequest("Invalid number format".to_string()));
            }
        }
        serde_json::Value::Bool(b) => ConfigValue::Boolean(*b),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<_>, _> = arr.iter().map(json_to_config_value).collect();
            ConfigValue::Array(values?)
        }
        serde_json::Value::Object(map) => {
            let mut config_map = HashMap::new();
            for (k, v) in map {
                config_map.insert(k.clone(), json_to_config_value(v)?);
            }
            ConfigValue::Object(config_map)
        }
        serde_json::Value::Null => ConfigValue::String(String::new()),
    })
}

/// GET /health - Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "llm-config-manager",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /api/v1/configs/:namespace/:key - Get a configuration value
pub async fn get_config(
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Query(params): Query<GetConfigQuery>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let env: Environment = params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let entry = state
        .manager
        .get(&namespace, &key, env)?
        .ok_or_else(|| ApiError::NotFound(format!("Configuration not found: {}:{}", namespace, key)))?;

    Ok(Json(entry.into()))
}

/// POST /api/v1/configs/:namespace/:key - Set a configuration value
pub async fn set_config(
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Json(req): Json<SetConfigRequest>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let env: Environment = req
        .env
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let entry = if req.secret {
        // Store as encrypted secret
        let value_str = req.value.as_str()
            .ok_or_else(|| ApiError::BadRequest("Secret value must be a string".to_string()))?;
        state
            .manager
            .set_secret(&namespace, &key, value_str.as_bytes(), env, &req.user)?
    } else {
        let config_value = json_to_config_value(&req.value)?;
        state
            .manager
            .set(&namespace, &key, config_value, env, &req.user)?
    };

    Ok(Json(entry.into()))
}

/// GET /api/v1/configs/:namespace - List configurations in a namespace
pub async fn list_configs(
    State(state): State<ApiState>,
    Path(namespace): Path<String>,
    Query(params): Query<GetConfigQuery>,
) -> Result<Json<Vec<ConfigResponse>>, ApiError> {
    let env: Environment = params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let entries = state.manager.list(&namespace, env)?;
    let responses: Vec<ConfigResponse> = entries.into_iter().map(|e| e.into()).collect();

    Ok(Json(responses))
}

/// DELETE /api/v1/configs/:namespace/:key - Delete a configuration
pub async fn delete_config(
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Query(params): Query<GetConfigQuery>,
) -> Result<StatusCode, ApiError> {
    let env: Environment = params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let deleted = state.manager.delete(&namespace, &key, env)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Configuration not found: {}:{}", namespace, key)))
    }
}

/// GET /api/v1/configs/:namespace/:key/history - Get version history
pub async fn get_history(
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Query(params): Query<GetConfigQuery>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let env: Environment = params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let history = state.manager.get_history(&namespace, &key, env)?;

    let response: Vec<serde_json::Value> = history
        .into_iter()
        .map(|v| {
            serde_json::json!({
                "version": v.version,
                "value": config_value_to_json(&v.value),
                "created_at": v.created_at.to_rfc3339(),
                "created_by": v.created_by,
                "change_description": v.change_description,
            })
        })
        .collect();

    Ok(Json(response))
}

/// POST /api/v1/configs/:namespace/:key/rollback/:version - Rollback to a specific version
#[derive(Debug, Deserialize)]
pub struct RollbackQuery {
    env: Option<String>,
}

pub async fn rollback_config(
    State(state): State<ApiState>,
    Path((namespace, key, version)): Path<(String, String, u64)>,
    Query(params): Query<RollbackQuery>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let env: Environment = params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
        .map_err(|e| ApiError::BadRequest(e))?;

    let entry = state
        .manager
        .rollback(&namespace, &key, env, version)?
        .ok_or_else(|| ApiError::NotFound(format!("Version {} not found", version)))?;

    Ok(Json(entry.into()))
}

// ─── Instrumented execution endpoints ────────────────────────────────

/// GET /api/v1/execution/configs/:namespace/:key - Instrumented config retrieval.
///
/// Requires `X-Parent-Span-Id` header (rejects with 400 if missing).
/// Creates repo-level and agent-level execution spans, attaches the
/// config response as an artifact, and returns the span tree in
/// an `ExecutionEnvelope`.
pub async fn execute_get_config(
    exec_ctx: ExecutionContextExtractor,
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Query(params): Query<GetConfigQuery>,
) -> Result<Json<ExecutionEnvelope<ConfigResponse>>, Json<ExecutionEnvelope<()>>> {
    let ctx = exec_ctx.0;
    let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
    let mut agent_span = tree.start_agent_span("config-manager-api");

    let env: Environment = match params
        .env
        .as_deref()
        .unwrap_or("development")
        .parse()
    {
        Ok(e) => e,
        Err(e) => {
            agent_span.fail(e.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(e.clone());
            return Err(Json(ExecutionEnvelope::failure(e, span_tree)));
        }
    };

    match state.manager.get(&namespace, &key, env) {
        Ok(Some(entry)) => {
            let response: ConfigResponse = entry.into();
            if let Ok(artifact) = serde_json::to_value(&response) {
                agent_span.attach_artifact(artifact);
            }
            agent_span.complete();
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize();
            Ok(Json(ExecutionEnvelope::success(response, span_tree)))
        }
        Ok(None) => {
            let err = format!("Configuration not found: {}:{}", namespace, key);
            agent_span.fail(err.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(err.clone());
            Err(Json(ExecutionEnvelope::failure(err, span_tree)))
        }
        Err(e) => {
            let err = e.to_string();
            agent_span.fail(err.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(err.clone());
            Err(Json(ExecutionEnvelope::failure(err, span_tree)))
        }
    }
}

/// POST /api/v1/execution/configs/:namespace/:key - Instrumented config set.
///
/// Requires `X-Parent-Span-Id` header (rejects with 400 if missing).
/// Creates repo-level and agent-level execution spans, attaches the
/// config response as an artifact, and returns the span tree in
/// an `ExecutionEnvelope`.
pub async fn execute_set_config(
    exec_ctx: ExecutionContextExtractor,
    State(state): State<ApiState>,
    Path((namespace, key)): Path<(String, String)>,
    Json(req): Json<SetConfigRequest>,
) -> Result<Json<ExecutionEnvelope<ConfigResponse>>, Json<ExecutionEnvelope<()>>> {
    let ctx = exec_ctx.0;
    let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
    let mut agent_span = tree.start_agent_span("config-manager-api");

    let env: Environment = match req.env.parse() {
        Ok(e) => e,
        Err(e) => {
            agent_span.fail(e.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(e.clone());
            return Err(Json(ExecutionEnvelope::failure(e, span_tree)));
        }
    };

    let result = if req.secret {
        let value_str = match req.value.as_str() {
            Some(s) => s,
            None => {
                let err = "Secret value must be a string".to_string();
                agent_span.fail(err.clone());
                tree.add_completed_agent_span(agent_span);
                let span_tree = tree.finalize_failed(err.clone());
                return Err(Json(ExecutionEnvelope::failure(err, span_tree)));
            }
        };
        state
            .manager
            .set_secret(&namespace, &key, value_str.as_bytes(), env, &req.user)
    } else {
        match json_to_config_value(&req.value) {
            Ok(config_value) => state
                .manager
                .set(&namespace, &key, config_value, env, &req.user),
            Err(e) => {
                let err = format!("{:?}", e);
                agent_span.fail(err.clone());
                tree.add_completed_agent_span(agent_span);
                let span_tree = tree.finalize_failed(err.clone());
                return Err(Json(ExecutionEnvelope::failure(err, span_tree)));
            }
        }
    };

    match result {
        Ok(entry) => {
            let response: ConfigResponse = entry.into();
            if let Ok(artifact) = serde_json::to_value(&response) {
                agent_span.attach_artifact(artifact);
            }
            agent_span.complete();
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize();
            Ok(Json(ExecutionEnvelope::success(response, span_tree)))
        }
        Err(e) => {
            let err = e.to_string();
            agent_span.fail(err.clone());
            tree.add_completed_agent_span(agent_span);
            let span_tree = tree.finalize_failed(err.clone());
            Err(Json(ExecutionEnvelope::failure(err, span_tree)))
        }
    }
}
