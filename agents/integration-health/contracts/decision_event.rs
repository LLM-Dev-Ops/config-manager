//! DecisionEvent for integration_health_signal emission
//!
//! Emits deterministic integration health signals to ruvector-service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{AdapterType, HealthStatus, IntegrationHealthOutput};

/// Integration health signal for ruvector-service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHealthSignal {
    /// Unique event identifier
    pub event_id: Uuid,

    /// Agent identifier
    pub agent_id: String,

    /// Agent version
    pub agent_version: String,

    /// Signal type (always "integration_health_signal")
    pub signal_type: String,

    /// Decision type
    pub decision_type: IntegrationDecisionType,

    /// Hash of inputs for deduplication
    pub inputs_hash: String,

    /// Structured outputs
    pub outputs: IntegrationHealthOutputs,

    /// Confidence score (0.0-1.0)
    pub confidence: f64,

    /// Constraints applied
    pub constraints_applied: Vec<String>,

    /// Execution reference
    pub execution_ref: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Performance metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceMetrics>,

    /// Correlation IDs
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub correlation_ids: HashMap<String, String>,
}

impl IntegrationHealthSignal {
    pub const AGENT_VERSION: &'static str = "0.1.0";
    pub const AGENT_ID: &'static str = "integration-health-agent";
    pub const SIGNAL_TYPE: &'static str = "integration_health_signal";

    /// Create from health check output
    pub fn from_health_check(
        inputs_hash: String,
        output: &IntegrationHealthOutput,
        execution_ref: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: Self::AGENT_ID.to_string(),
            agent_version: Self::AGENT_VERSION.to_string(),
            signal_type: Self::SIGNAL_TYPE.to_string(),
            decision_type: IntegrationDecisionType::HealthCheck,
            inputs_hash,
            outputs: IntegrationHealthOutputs::from_output(output),
            confidence: output.confidence(),
            constraints_applied: Vec::new(),
            execution_ref,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            performance: Some(PerformanceMetrics {
                duration_ms: output.duration_ms,
                adapters_checked: output.adapters_checked,
                memory_used_bytes: None,
            }),
            correlation_ids: HashMap::new(),
        }
    }

    /// Create with custom values
    pub fn new(
        decision_type: IntegrationDecisionType,
        inputs_hash: String,
        outputs: IntegrationHealthOutputs,
        confidence: f64,
        execution_ref: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: Self::AGENT_ID.to_string(),
            agent_version: Self::AGENT_VERSION.to_string(),
            signal_type: Self::SIGNAL_TYPE.to_string(),
            decision_type,
            inputs_hash,
            outputs,
            confidence: confidence.clamp(0.0, 1.0),
            constraints_applied: Vec::new(),
            execution_ref,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            performance: None,
            correlation_ids: HashMap::new(),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Add correlation ID
    pub fn with_correlation_id(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.correlation_ids.insert(key.into(), value.into());
        self
    }

    /// Get summary
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} - healthy={}, score={:.2}, adapters={}/{}",
            self.agent_id,
            self.decision_type.as_str(),
            self.outputs.is_healthy,
            self.outputs.health_score,
            self.outputs.healthy_count,
            self.outputs.adapters_checked,
        )
    }

    /// Check if high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Check if indicates unhealthy
    pub fn is_unhealthy(&self) -> bool {
        !self.outputs.is_healthy
    }
}

/// Integration decision types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationDecisionType {
    /// General health check
    HealthCheck,
    /// Connectivity test
    ConnectivityTest,
    /// Latency measurement
    LatencyMeasurement,
    /// Availability check
    AvailabilityCheck,
    /// Capacity check
    CapacityCheck,
}

impl IntegrationDecisionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HealthCheck => "health_check",
            Self::ConnectivityTest => "connectivity_test",
            Self::LatencyMeasurement => "latency_measurement",
            Self::AvailabilityCheck => "availability_check",
            Self::CapacityCheck => "capacity_check",
        }
    }
}

/// Structured outputs for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHealthOutputs {
    /// Overall health
    pub is_healthy: bool,

    /// Health score (0.0-1.0)
    pub health_score: f64,

    /// Total adapters checked
    pub adapters_checked: u32,

    /// Healthy count
    pub healthy_count: u32,

    /// Degraded count
    pub degraded_count: u32,

    /// Unhealthy count
    pub unhealthy_count: u32,

    /// Adapter summaries
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_summaries: Vec<AdapterSummary>,

    /// Average latency
    pub avg_latency_ms: f64,

    /// Max latency
    pub max_latency_ms: u64,
}

impl IntegrationHealthOutputs {
    /// Create from health check output
    pub fn from_output(output: &IntegrationHealthOutput) -> Self {
        let latencies: Vec<u64> = output
            .adapter_results
            .iter()
            .map(|r| r.latency_ms)
            .collect();

        let avg_latency = if latencies.is_empty() {
            0.0
        } else {
            latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
        };

        let max_latency = latencies.iter().max().copied().unwrap_or(0);

        Self {
            is_healthy: output.is_healthy,
            health_score: output.health_score,
            adapters_checked: output.adapters_checked,
            healthy_count: output.healthy_count,
            degraded_count: output.degraded_count,
            unhealthy_count: output.unhealthy_count,
            adapter_summaries: output
                .adapter_results
                .iter()
                .map(|r| AdapterSummary {
                    adapter_id: r.adapter_id.clone(),
                    adapter_type: r.adapter_type,
                    status: r.status,
                    latency_ms: r.latency_ms,
                })
                .collect(),
            avg_latency_ms: avg_latency,
            max_latency_ms: max_latency,
        }
    }

    /// Create healthy output
    pub fn healthy(adapters_checked: u32) -> Self {
        Self {
            is_healthy: true,
            health_score: 1.0,
            adapters_checked,
            healthy_count: adapters_checked,
            degraded_count: 0,
            unhealthy_count: 0,
            adapter_summaries: Vec::new(),
            avg_latency_ms: 0.0,
            max_latency_ms: 0,
        }
    }
}

/// Adapter summary for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSummary {
    pub adapter_id: String,
    pub adapter_type: AdapterType,
    pub status: HealthStatus,
    pub latency_ms: u64,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub duration_ms: u64,
    pub adapters_checked: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_used_bytes: Option<u64>,
}

impl PerformanceMetrics {
    pub fn new(duration_ms: u64, adapters_checked: u32) -> Self {
        Self {
            duration_ms,
            adapters_checked,
            memory_used_bytes: None,
        }
    }
}

/// Batch of integration health signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationHealthSignalBatch {
    pub batch_id: Uuid,
    pub signals: Vec<IntegrationHealthSignal>,
    pub created_at: DateTime<Utc>,
    pub source: String,
}

impl IntegrationHealthSignalBatch {
    pub fn new(signals: Vec<IntegrationHealthSignal>, source: impl Into<String>) -> Self {
        Self {
            batch_id: Uuid::new_v4(),
            signals,
            created_at: Utc::now(),
            source: source.into(),
        }
    }

    pub fn len(&self) -> usize {
        self.signals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
}
