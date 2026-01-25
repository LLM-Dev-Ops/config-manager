//! Integration Health Agent Contracts
//!
//! Defines external adapter health monitoring for deterministic integration checks.

mod decision_event;
mod adapters;

pub use decision_event::*;
pub use adapters::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Input for integration health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHealthInput {
    /// Unique request identifier
    pub request_id: Uuid,

    /// Adapters to check
    pub adapters: Vec<AdapterConfig>,

    /// Check options
    #[serde(default)]
    pub options: HealthCheckOptions,

    /// Request context
    #[serde(default)]
    pub context: HashMap<String, String>,

    /// Request timestamp
    pub requested_at: DateTime<Utc>,

    /// Requester identity
    pub requested_by: String,
}

/// Output from integration health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHealthOutput {
    /// Request ID correlation
    pub request_id: Uuid,

    /// Overall health status
    pub is_healthy: bool,

    /// Individual adapter results
    pub adapter_results: Vec<AdapterHealthResult>,

    /// Aggregated health score (0.0-1.0)
    pub health_score: f64,

    /// Total adapters checked
    pub adapters_checked: u32,

    /// Healthy adapter count
    pub healthy_count: u32,

    /// Degraded adapter count
    pub degraded_count: u32,

    /// Unhealthy adapter count
    pub unhealthy_count: u32,

    /// Completion timestamp
    pub completed_at: DateTime<Utc>,

    /// Total duration in milliseconds
    pub duration_ms: u64,
}

impl IntegrationHealthOutput {
    /// Create healthy output
    pub fn healthy(request_id: Uuid, results: Vec<AdapterHealthResult>) -> Self {
        let total = results.len() as u32;
        let healthy = results.iter().filter(|r| r.status == HealthStatus::Healthy).count() as u32;
        let degraded = results.iter().filter(|r| r.status == HealthStatus::Degraded).count() as u32;
        let unhealthy = results.iter().filter(|r| r.status == HealthStatus::Unhealthy).count() as u32;

        let score = if total > 0 {
            (healthy as f64 + degraded as f64 * 0.5) / total as f64
        } else {
            1.0
        };

        Self {
            request_id,
            is_healthy: unhealthy == 0,
            adapter_results: results,
            health_score: score,
            adapters_checked: total,
            healthy_count: healthy,
            degraded_count: degraded,
            unhealthy_count: unhealthy,
            completed_at: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Set duration
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Calculate confidence score
    pub fn confidence(&self) -> f64 {
        let mut conf = self.health_score;
        // Penalty for degraded
        conf -= self.degraded_count as f64 * 0.05;
        // Penalty for unhealthy
        conf -= self.unhealthy_count as f64 * 0.1;
        conf.clamp(0.0, 1.0)
    }
}

/// Health check options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthCheckOptions {
    /// Timeout per adapter in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Whether to run checks in parallel
    #[serde(default = "default_parallel")]
    pub parallel: bool,

    /// Include detailed diagnostics
    #[serde(default)]
    pub include_diagnostics: bool,

    /// Retry failed checks
    #[serde(default)]
    pub retry_failed: bool,
}

fn default_timeout() -> u64 {
    500
}

fn default_parallel() -> bool {
    true
}

/// Individual adapter health result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealthResult {
    /// Adapter identifier
    pub adapter_id: String,

    /// Adapter type
    pub adapter_type: AdapterType,

    /// Health status
    pub status: HealthStatus,

    /// Response latency in milliseconds
    pub latency_ms: u64,

    /// Error message if unhealthy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Diagnostic details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<HashMap<String, serde_json::Value>>,

    /// Check timestamp
    pub checked_at: DateTime<Utc>,
}

impl AdapterHealthResult {
    /// Create healthy result
    pub fn healthy(adapter_id: impl Into<String>, adapter_type: AdapterType, latency_ms: u64) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            adapter_type,
            status: HealthStatus::Healthy,
            latency_ms,
            error: None,
            diagnostics: None,
            checked_at: Utc::now(),
        }
    }

    /// Create degraded result
    pub fn degraded(
        adapter_id: impl Into<String>,
        adapter_type: AdapterType,
        latency_ms: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            adapter_type,
            status: HealthStatus::Degraded,
            latency_ms,
            error: Some(reason.into()),
            diagnostics: None,
            checked_at: Utc::now(),
        }
    }

    /// Create unhealthy result
    pub fn unhealthy(
        adapter_id: impl Into<String>,
        adapter_type: AdapterType,
        error: impl Into<String>,
    ) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            adapter_type,
            status: HealthStatus::Unhealthy,
            latency_ms: 0,
            error: Some(error.into()),
            diagnostics: None,
            checked_at: Utc::now(),
        }
    }

    /// Add diagnostics
    pub fn with_diagnostics(mut self, diagnostics: HashMap<String, serde_json::Value>) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}

/// Health status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Fully operational
    Healthy,
    /// Operational with issues
    Degraded,
    /// Not operational
    Unhealthy,
    /// Status unknown
    Unknown,
}
