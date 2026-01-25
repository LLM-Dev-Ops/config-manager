//! DecisionEvent for schema_violation_signal emission
//!
//! Emits deterministic schema violation signals to ruvector-service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{SchemaValidationOutput, SchemaViolation, ViolationSeverity};

/// Schema violation signal for ruvector-service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaViolationSignal {
    /// Unique event identifier
    pub event_id: Uuid,

    /// Agent identifier
    pub agent_id: String,

    /// Agent version
    pub agent_version: String,

    /// Signal type (always "schema_violation_signal")
    pub signal_type: String,

    /// Decision type
    pub decision_type: SchemaDecisionType,

    /// Hash of inputs for deduplication
    pub inputs_hash: String,

    /// Structured outputs
    pub outputs: SchemaViolationOutputs,

    /// Confidence score (0.0-1.0)
    pub confidence: f64,

    /// Constraints that were applied
    pub constraints_applied: Vec<String>,

    /// Execution reference (request/trace ID)
    pub execution_ref: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Performance metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceMetrics>,

    /// Correlation IDs for distributed tracing
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub correlation_ids: HashMap<String, String>,
}

impl SchemaViolationSignal {
    /// Agent version constant
    pub const AGENT_VERSION: &'static str = "0.1.0";

    /// Agent identifier constant
    pub const AGENT_ID: &'static str = "schema-truth-agent";

    /// Signal type constant
    pub const SIGNAL_TYPE: &'static str = "schema_violation_signal";

    /// Create from validation output
    pub fn from_validation(
        inputs_hash: String,
        output: &SchemaValidationOutput,
        execution_ref: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: Self::AGENT_ID.to_string(),
            agent_version: Self::AGENT_VERSION.to_string(),
            signal_type: Self::SIGNAL_TYPE.to_string(),
            decision_type: SchemaDecisionType::SchemaValidation,
            inputs_hash,
            outputs: SchemaViolationOutputs::from_output(output),
            confidence: output.confidence(),
            constraints_applied: output.constraints_checked.clone(),
            execution_ref,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            performance: Some(PerformanceMetrics {
                duration_ms: output.duration_ms,
                rules_evaluated: output.rules_applied.len() as u32,
                memory_used_bytes: None,
            }),
            correlation_ids: HashMap::new(),
        }
    }

    /// Create with custom values
    pub fn new(
        decision_type: SchemaDecisionType,
        inputs_hash: String,
        outputs: SchemaViolationOutputs,
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

    /// Get summary for logging
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} - valid={}, confidence={:.2}, violations={}, warnings={}",
            self.agent_id,
            self.decision_type.as_str(),
            self.outputs.is_valid,
            self.confidence,
            self.outputs.violation_count,
            self.outputs.warning_count,
        )
    }

    /// Check if high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Check if indicates violation
    pub fn has_violations(&self) -> bool {
        !self.outputs.is_valid
    }
}

/// Schema decision types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaDecisionType {
    /// Schema structure validation
    SchemaValidation,

    /// Schema compatibility check
    SchemaCompatibility,

    /// Schema evolution validation
    SchemaEvolution,

    /// Field type validation
    FieldTypeValidation,

    /// Constraint validation
    ConstraintValidation,
}

impl SchemaDecisionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SchemaValidation => "schema_validation",
            Self::SchemaCompatibility => "schema_compatibility",
            Self::SchemaEvolution => "schema_evolution",
            Self::FieldTypeValidation => "field_type_validation",
            Self::ConstraintValidation => "constraint_validation",
        }
    }
}

/// Structured outputs for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaViolationOutputs {
    /// Overall validity
    pub is_valid: bool,

    /// Violation count
    pub violation_count: u32,

    /// Warning count
    pub warning_count: u32,

    /// Validation coverage
    pub coverage: f64,

    /// Violation codes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violation_codes: Vec<String>,

    /// Warning codes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warning_codes: Vec<String>,

    /// Detailed violations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<ViolationSummary>,

    /// Rules applied
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules_applied: Vec<String>,

    /// Schema fields validated
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields_validated: Vec<String>,
}

impl SchemaViolationOutputs {
    /// Create from validation output
    pub fn from_output(output: &SchemaValidationOutput) -> Self {
        Self {
            is_valid: output.is_valid,
            violation_count: output.violations.len() as u32,
            warning_count: output.warnings.len() as u32,
            coverage: output.coverage,
            violation_codes: output.violations.iter().map(|v| v.code.clone()).collect(),
            warning_codes: output.warnings.iter().map(|w| w.code.clone()).collect(),
            violations: output
                .violations
                .iter()
                .chain(output.warnings.iter())
                .map(ViolationSummary::from_violation)
                .collect(),
            rules_applied: output.rules_applied.clone(),
            fields_validated: Vec::new(),
        }
    }

    /// Create success output
    pub fn success(rules_applied: Vec<String>, coverage: f64) -> Self {
        Self {
            is_valid: true,
            violation_count: 0,
            warning_count: 0,
            coverage,
            violation_codes: Vec::new(),
            warning_codes: Vec::new(),
            violations: Vec::new(),
            rules_applied,
            fields_validated: Vec::new(),
        }
    }

    /// Create failure output
    pub fn failure(violations: Vec<ViolationSummary>) -> Self {
        let violation_codes = violations.iter().map(|v| v.code.clone()).collect();
        Self {
            is_valid: false,
            violation_count: violations.len() as u32,
            warning_count: 0,
            coverage: 1.0,
            violation_codes,
            warning_codes: Vec::new(),
            violations,
            rules_applied: Vec::new(),
            fields_validated: Vec::new(),
        }
    }
}

/// Violation summary for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationSummary {
    /// Violation code
    pub code: String,

    /// Severity
    pub severity: ViolationSeverity,

    /// Path in schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Rule that triggered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
}

impl ViolationSummary {
    /// Create from SchemaViolation
    pub fn from_violation(v: &SchemaViolation) -> Self {
        Self {
            code: v.code.clone(),
            severity: v.severity,
            path: v.path.clone(),
            rule_id: v.rule_id.clone(),
        }
    }

    /// Create new summary
    pub fn new(code: impl Into<String>, severity: ViolationSeverity) -> Self {
        Self {
            code: code.into(),
            severity,
            path: None,
            rule_id: None,
        }
    }
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Rules evaluated count
    pub rules_evaluated: u32,

    /// Memory used (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_used_bytes: Option<u64>,
}

impl PerformanceMetrics {
    /// Create new metrics
    pub fn new(duration_ms: u64, rules_evaluated: u32) -> Self {
        Self {
            duration_ms,
            rules_evaluated,
            memory_used_bytes: None,
        }
    }
}

/// Batch of schema violation signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaViolationSignalBatch {
    pub batch_id: Uuid,
    pub signals: Vec<SchemaViolationSignal>,
    pub created_at: DateTime<Utc>,
    pub source: String,
}

impl SchemaViolationSignalBatch {
    pub fn new(signals: Vec<SchemaViolationSignal>, source: impl Into<String>) -> Self {
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
