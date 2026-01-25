//! Client for invoking Schema Truth Agent remotely
//!
//! Used by other services to validate schemas.

use crate::contracts::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Schema Truth Agent client
pub struct SchemaTruthClient {
    base_url: String,
    client: reqwest::Client,
    timeout: Duration,
}

impl SchemaTruthClient {
    /// Create new client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            timeout: Duration::from_millis(1500), // Match MAX_LATENCY_MS
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Validate a schema
    pub async fn validate(
        &self,
        schema: serde_json::Value,
        requested_by: Option<String>,
    ) -> Result<ClientResponse<SchemaValidationOutput>, ClientError> {
        let url = format!("{}/api/v1/schema/validate", self.base_url);

        let request = ValidateRequest {
            schema,
            requested_by,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| ClientError::Network(e.to_string()))?;

        if response.status().is_success() {
            let data: ApiResponse<SchemaValidationOutput> = response
                .json()
                .await
                .map_err(|e| ClientError::Parse(e.to_string()))?;

            Ok(ClientResponse {
                success: data.success,
                data: data.data,
                request_id: data.request_id,
            })
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                status: status.as_u16(),
                message: error_text,
            })
        }
    }

    /// Quick check (no telemetry)
    pub async fn check(&self, schema: serde_json::Value) -> Result<CheckResult, ClientError> {
        let url = format!("{}/api/v1/schema/check", self.base_url);

        let request = ValidateRequest {
            schema,
            requested_by: None,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| ClientError::Network(e.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| ClientError::Parse(e.to_string()))
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                status: status.as_u16(),
                message: error_text,
            })
        }
    }
}

#[derive(Debug, Serialize)]
struct ValidateRequest {
    schema: serde_json::Value,
    requested_by: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: T,
    request_id: uuid::Uuid,
}

/// Client response
#[derive(Debug)]
pub struct ClientResponse<T> {
    pub success: bool,
    pub data: T,
    pub request_id: uuid::Uuid,
}

/// Quick check result
#[derive(Debug, Deserialize)]
pub struct CheckResult {
    pub is_valid: bool,
    pub violation_count: usize,
    pub warning_count: usize,
    pub coverage: f64,
    pub duration_ms: u64,
}

/// Client errors
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Server error {status}: {message}")]
    Server { status: u16, message: String },
}
