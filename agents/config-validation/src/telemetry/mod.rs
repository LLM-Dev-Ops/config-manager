//! Telemetry module for Config Validation Agent
//!
//! This module provides telemetry capabilities including:
//! - DecisionEvent emission to ruvector-service
//! - Prometheus metrics collection
//! - Non-blocking async telemetry operations
//!
//! # Architecture
//!
//! The telemetry module integrates with the existing contracts module
//! for DecisionEvent definitions and provides:
//!
//! - `emitter` - Async, non-blocking DecisionEvent emission to ruvector-service
//! - `metrics` - Prometheus metrics for validation operations

pub mod emitter;
pub mod metrics;

pub use emitter::{DecisionEventEmitter, EmitterConfig};
pub use metrics::{ValidationMetrics, ValidationMetricsRegistry};

use thiserror::Error;

/// Telemetry errors
#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("Failed to emit decision event: {0}")]
    EmissionFailed(String),

    #[error("Failed to serialize event: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpError(String),

    #[error("Metrics error: {0}")]
    MetricsError(#[from] prometheus::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, TelemetryError>;

/// Telemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable decision event emission
    pub emit_decisions: bool,

    /// Enable Prometheus metrics
    pub enable_metrics: bool,

    /// ruvector-service endpoint
    pub ruvector_endpoint: String,

    /// Maximum queue size for async emission
    pub max_queue_size: usize,

    /// Emission timeout in milliseconds
    pub timeout_ms: u64,

    /// Enable batching of events
    pub enable_batching: bool,

    /// Batch size for event emission
    pub batch_size: usize,

    /// Batch flush interval in milliseconds
    pub batch_flush_interval_ms: u64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            emit_decisions: true,
            enable_metrics: true,
            ruvector_endpoint: "http://localhost:8080".to_string(),
            max_queue_size: 1000,
            timeout_ms: 5000,
            enable_batching: false,
            batch_size: 100,
            batch_flush_interval_ms: 1000,
        }
    }
}

impl TelemetryConfig {
    /// Create a new config builder
    pub fn builder() -> TelemetryConfigBuilder {
        TelemetryConfigBuilder::new()
    }

    /// Create config from environment variables
    pub fn from_env() -> Self {
        Self {
            emit_decisions: std::env::var("TELEMETRY_EMIT_DECISIONS")
                .map(|v| v.parse().unwrap_or(true))
                .unwrap_or(true),
            enable_metrics: std::env::var("TELEMETRY_ENABLE_METRICS")
                .map(|v| v.parse().unwrap_or(true))
                .unwrap_or(true),
            ruvector_endpoint: std::env::var("RUVECTOR_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            max_queue_size: std::env::var("TELEMETRY_MAX_QUEUE_SIZE")
                .map(|v| v.parse().unwrap_or(1000))
                .unwrap_or(1000),
            timeout_ms: std::env::var("TELEMETRY_TIMEOUT_MS")
                .map(|v| v.parse().unwrap_or(5000))
                .unwrap_or(5000),
            enable_batching: std::env::var("TELEMETRY_ENABLE_BATCHING")
                .map(|v| v.parse().unwrap_or(false))
                .unwrap_or(false),
            batch_size: std::env::var("TELEMETRY_BATCH_SIZE")
                .map(|v| v.parse().unwrap_or(100))
                .unwrap_or(100),
            batch_flush_interval_ms: std::env::var("TELEMETRY_BATCH_FLUSH_INTERVAL_MS")
                .map(|v| v.parse().unwrap_or(1000))
                .unwrap_or(1000),
        }
    }
}

/// Builder for TelemetryConfig
pub struct TelemetryConfigBuilder {
    config: TelemetryConfig,
}

impl TelemetryConfigBuilder {
    /// Create a new builder with defaults
    pub fn new() -> Self {
        Self {
            config: TelemetryConfig::default(),
        }
    }

    /// Set the ruvector endpoint
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.ruvector_endpoint = endpoint.into();
        self
    }

    /// Enable or disable decision emission
    pub fn emit_decisions(mut self, enabled: bool) -> Self {
        self.config.emit_decisions = enabled;
        self
    }

    /// Enable or disable metrics
    pub fn enable_metrics(mut self, enabled: bool) -> Self {
        self.config.enable_metrics = enabled;
        self
    }

    /// Set the maximum queue size
    pub fn max_queue_size(mut self, size: usize) -> Self {
        self.config.max_queue_size = size;
        self
    }

    /// Set the timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.config.timeout_ms = timeout;
        self
    }

    /// Enable batching with specified size
    pub fn with_batching(mut self, batch_size: usize, flush_interval_ms: u64) -> Self {
        self.config.enable_batching = true;
        self.config.batch_size = batch_size;
        self.config.batch_flush_interval_ms = flush_interval_ms;
        self
    }

    /// Build the configuration
    pub fn build(self) -> TelemetryConfig {
        self.config
    }
}

impl Default for TelemetryConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();
        assert!(config.emit_decisions);
        assert!(config.enable_metrics);
        assert_eq!(config.max_queue_size, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert!(!config.enable_batching);
    }

    #[test]
    fn test_config_builder() {
        let config = TelemetryConfig::builder()
            .endpoint("http://ruvector:9090")
            .emit_decisions(true)
            .enable_metrics(false)
            .max_queue_size(500)
            .timeout_ms(10000)
            .with_batching(50, 2000)
            .build();

        assert_eq!(config.ruvector_endpoint, "http://ruvector:9090");
        assert!(config.emit_decisions);
        assert!(!config.enable_metrics);
        assert_eq!(config.max_queue_size, 500);
        assert_eq!(config.timeout_ms, 10000);
        assert!(config.enable_batching);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.batch_flush_interval_ms, 2000);
    }
}
