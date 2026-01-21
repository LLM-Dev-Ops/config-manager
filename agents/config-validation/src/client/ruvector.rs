//! ruvector-service HTTP client
//!
//! Provides an HTTP client for persisting DecisionEvents to ruvector-service.
//! Features:
//! - Async, non-blocking requests
//! - Retry logic with exponential backoff
//! - NO direct SQL connections - uses HTTP API only
//!
//! # Design
//!
//! The client follows a fire-and-forget pattern for event emission to ensure
//! validation operations are not blocked by telemetry. Events are queued and
//! emitted asynchronously with automatic retries on transient failures.

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

use crate::contracts::DecisionEvent;
use crate::contracts::decision_event::DecisionEventBatch;
use crate::telemetry::{Result, TelemetryError};

/// Configuration for the ruvector client
#[derive(Debug, Clone)]
pub struct RuvectorClientConfig {
    /// Base URL for ruvector-service
    pub base_url: String,

    /// Request timeout in milliseconds
    pub timeout_ms: u64,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Initial backoff delay in milliseconds
    pub initial_backoff_ms: u64,

    /// Maximum backoff delay in milliseconds
    pub max_backoff_ms: u64,

    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RuvectorClientConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            timeout_ms: 5000,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Response from ruvector-service when persisting events
#[derive(Debug, Deserialize, Serialize)]
pub struct PersistResponse {
    /// Whether the operation succeeded
    pub success: bool,

    /// Event ID that was persisted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Timestamp of persistence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persisted_at: Option<String>,
}

/// Health check response
#[derive(Debug, Deserialize, Serialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,

    /// Service version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Service name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,

    /// Uptime in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<u64>,
}

/// HTTP client for ruvector-service
pub struct RuvectorClient {
    client: Client,
    config: RuvectorClientConfig,
}

impl RuvectorClient {
    /// Create a new ruvector client with default configuration
    pub fn new(base_url: impl Into<String>, timeout_ms: u64) -> Self {
        let config = RuvectorClientConfig {
            base_url: base_url.into(),
            timeout_ms,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create a new ruvector client with custom configuration
    pub fn with_config(config: RuvectorClientConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Persist a DecisionEvent to ruvector-service with retry logic
    pub async fn persist_decision_event(&self, event: &DecisionEvent) -> Result<PersistResponse> {
        let url = format!("{}/api/v1/decisions", self.config.base_url);

        let mut last_error = None;
        let mut backoff_ms = self.config.initial_backoff_ms;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tracing::debug!(
                    attempt = attempt,
                    backoff_ms = backoff_ms,
                    event_id = %event.event_id,
                    "Retrying decision event persistence"
                );
                sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms as f64 * self.config.backoff_multiplier) as u64;
                backoff_ms = backoff_ms.min(self.config.max_backoff_ms);
            }

            match self.send_event(&url, event).await {
                Ok(response) => {
                    tracing::debug!(
                        event_id = %event.event_id,
                        "Successfully persisted decision event"
                    );
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt,
                        error = %e,
                        event_id = %event.event_id,
                        "Failed to persist decision event"
                    );
                    last_error = Some(e);

                    // Don't retry on certain errors
                    if let Some(ref err) = last_error {
                        if is_permanent_error(err) {
                            break;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            TelemetryError::EmissionFailed("Unknown error during event persistence".to_string())
        }))
    }

    /// Send a single event request
    async fn send_event(&self, url: &str, event: &DecisionEvent) -> Result<PersistResponse> {
        let response = self
            .client
            .post(url)
            .json(event)
            .header("Content-Type", "application/json")
            .header("X-Agent-Id", &event.agent_id)
            .header("X-Agent-Version", &event.agent_version)
            .header("X-Event-Id", event.event_id.to_string())
            .header("X-Decision-Type", event.decision_type.as_str())
            .send()
            .await
            .map_err(|e| TelemetryError::HttpError(e.to_string()))?;

        let status = response.status();

        if status.is_success() {
            let persist_response: PersistResponse = response
                .json()
                .await
                .map_err(|e| TelemetryError::HttpError(format!("Failed to parse response: {}", e)))?;
            Ok(persist_response)
        } else if status == StatusCode::BAD_REQUEST {
            let error_text = response.text().await.unwrap_or_default();
            Err(TelemetryError::EmissionFailed(format!(
                "Bad request: {}",
                error_text
            )))
        } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            Err(TelemetryError::EmissionFailed(format!(
                "Authentication failed: {}",
                status
            )))
        } else if status.is_server_error() {
            Err(TelemetryError::HttpError(format!(
                "Server error: {}",
                status
            )))
        } else {
            Err(TelemetryError::HttpError(format!(
                "Unexpected status: {}",
                status
            )))
        }
    }

    /// Batch persist multiple DecisionEvents
    pub async fn persist_batch(&self, batch: &DecisionEventBatch) -> Result<Vec<PersistResponse>> {
        let url = format!("{}/api/v1/decisions/batch", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(batch)
            .header("Content-Type", "application/json")
            .header("X-Batch-Id", batch.batch_id.to_string())
            .header("X-Batch-Size", batch.len().to_string())
            .send()
            .await
            .map_err(|e| TelemetryError::HttpError(e.to_string()))?;

        if response.status().is_success() {
            let responses: Vec<PersistResponse> = response
                .json()
                .await
                .map_err(|e| TelemetryError::HttpError(format!("Failed to parse response: {}", e)))?;
            Ok(responses)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(TelemetryError::EmissionFailed(format!(
                "Batch persist failed: {}",
                error_text
            )))
        }
    }

    /// Health check for ruvector-service
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let health: HealthResponse = response
                        .json()
                        .await
                        .unwrap_or(HealthResponse {
                            status: "unknown".to_string(),
                            version: None,
                            service: None,
                            uptime_seconds: None,
                        });
                    Ok(health.status == "healthy" || health.status == "ok" || health.status == "up")
                } else {
                    Ok(false)
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Health check failed");
                Ok(false)
            }
        }
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Get the timeout in milliseconds
    pub fn timeout_ms(&self) -> u64 {
        self.config.timeout_ms
    }

    /// Get the maximum retry count
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }
}

/// Determine if an error is permanent (should not retry)
fn is_permanent_error(error: &TelemetryError) -> bool {
    match error {
        TelemetryError::EmissionFailed(msg) => {
            msg.contains("Bad request") || msg.contains("Authentication failed")
        }
        TelemetryError::SerializationFailed(_) => true,
        TelemetryError::ConfigError(_) => true,
        _ => false,
    }
}

/// Builder for RuvectorClient
pub struct RuvectorClientBuilder {
    config: RuvectorClientConfig,
}

impl RuvectorClientBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: RuvectorClientConfig::default(),
        }
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.config.base_url = url.into();
        self
    }

    /// Set the request timeout
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.config.timeout_ms = timeout;
        self
    }

    /// Set the maximum retry attempts
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Set the initial backoff delay
    pub fn initial_backoff_ms(mut self, backoff: u64) -> Self {
        self.config.initial_backoff_ms = backoff;
        self
    }

    /// Set the maximum backoff delay
    pub fn max_backoff_ms(mut self, backoff: u64) -> Self {
        self.config.max_backoff_ms = backoff;
        self
    }

    /// Set the backoff multiplier
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.config.backoff_multiplier = multiplier;
        self
    }

    /// Build the client
    pub fn build(self) -> RuvectorClient {
        RuvectorClient::with_config(self.config)
    }
}

impl Default for RuvectorClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::decision_event::{DecisionType, ValidationOutputs};
    use uuid::Uuid;

    #[test]
    fn test_client_config_default() {
        let config = RuvectorClientConfig::default();
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_client_builder() {
        let client = RuvectorClientBuilder::new()
            .base_url("http://ruvector:9090")
            .timeout_ms(10000)
            .max_retries(5)
            .initial_backoff_ms(200)
            .max_backoff_ms(10000)
            .backoff_multiplier(1.5)
            .build();

        assert_eq!(client.base_url(), "http://ruvector:9090");
        assert_eq!(client.timeout_ms(), 10000);
        assert_eq!(client.max_retries(), 5);
    }

    #[test]
    fn test_is_permanent_error() {
        assert!(is_permanent_error(&TelemetryError::EmissionFailed(
            "Bad request: invalid data".to_string()
        )));
        assert!(is_permanent_error(&TelemetryError::EmissionFailed(
            "Authentication failed: 401".to_string()
        )));
        assert!(is_permanent_error(&TelemetryError::SerializationFailed(
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test"
            ))
        )));

        assert!(!is_permanent_error(&TelemetryError::HttpError(
            "Server error: 500".to_string()
        )));
        assert!(!is_permanent_error(&TelemetryError::EmissionFailed(
            "Connection timeout".to_string()
        )));
    }

    #[test]
    fn test_decision_event_serialization() {
        let event = DecisionEvent::new(
            DecisionType::ConfigValidationResult,
            "test_hash".to_string(),
            ValidationOutputs::success(vec!["rule1".to_string()], 0.95),
            0.9,
            "exec-ref".to_string(),
        );

        // Verify event can be serialized for HTTP transport
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("config_validation_result"));
        assert!(json.contains("config-validation-agent"));
        assert!(json.contains("test_hash"));
    }

    #[test]
    fn test_persist_response_serialization() {
        let response = PersistResponse {
            success: true,
            event_id: Some(Uuid::new_v4().to_string()),
            error: None,
            persisted_at: Some("2024-01-01T00:00:00Z".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));

        let deserialized: PersistResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: Some("1.0.0".to_string()),
            service: Some("ruvector-service".to_string()),
            uptime_seconds: Some(3600),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));

        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "healthy");
    }
}
