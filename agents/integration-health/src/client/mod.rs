//! Client for invoking Integration Health Agent remotely
//!
//! Used by other services to check adapter health.

use crate::contracts::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Integration Health Agent client
pub struct IntegrationHealthClient {
    base_url: String,
    client: reqwest::Client,
    timeout: Duration,
}

impl IntegrationHealthClient {
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

    /// Check health of adapters
    pub async fn check(
        &self,
        adapters: Vec<AdapterConfig>,
        options: Option<HealthCheckOptions>,
        requested_by: Option<String>,
    ) -> Result<ClientResponse<IntegrationHealthOutput>, ClientError> {
        let url = format!("{}/api/v1/integration/check", self.base_url);

        let request = CheckRequest {
            adapters,
            options,
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
            let data: ApiResponse<IntegrationHealthOutput> = response
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

    /// Quick probe of a single adapter
    pub async fn probe(&self, adapter: AdapterConfig) -> Result<ProbeResult, ClientError> {
        let url = format!("{}/api/v1/integration/probe", self.base_url);

        let request = ProbeRequest { adapter };

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
struct CheckRequest {
    adapters: Vec<AdapterConfig>,
    options: Option<HealthCheckOptions>,
    requested_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProbeRequest {
    adapter: AdapterConfig,
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

/// Probe result
#[derive(Debug, Deserialize)]
pub struct ProbeResult {
    pub adapter_id: String,
    pub status: HealthStatus,
    pub latency_ms: u64,
    pub error: Option<String>,
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
