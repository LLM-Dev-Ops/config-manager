//! DecisionEvent structure for ruvector-service integration
//!
//! This module defines the DecisionEvent structure that records all
//! validation decisions for analytics and traceability via ruvector-service.
//!
//! # Design Principles
//!
//! - **Immutable**: Decision events are append-only records
//! - **Traceable**: Full audit trail from input to output
//! - **Analytics-Ready**: Structured for ML/analytics pipelines
//! - **Decoupled**: No direct SQL - uses ruvector-service client

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{IssueSeverity, ValidationIssue, ValidationOutput};

/// Decision event for ruvector-service integration
///
/// Records a complete validation decision including inputs, outputs,
/// confidence levels, and execution metadata for analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEvent {
    /// Unique identifier for this decision event
    pub event_id: Uuid,

    /// Agent identifier (e.g., "config-validation-agent")
    pub agent_id: String,

    /// Agent version (semantic versioning)
    pub agent_version: String,

    /// Type of decision made
    pub decision_type: DecisionType,

    /// Hash of the inputs for deduplication and tracing
    pub inputs_hash: String,

    /// Structured validation outputs
    pub outputs: ValidationOutputs,

    /// Confidence score for this decision (0.0 - 1.0)
    ///
    /// Calculated based on:
    /// - Validation coverage (how much of the schema was validated)
    /// - Rule completeness (how many rules were applied)
    /// - Input quality (presence of schema, context)
    pub confidence: f64,

    /// List of constraints that were applied during validation
    pub constraints_applied: Vec<String>,

    /// Reference to the execution context (request ID, trace ID)
    pub execution_ref: String,

    /// Timestamp when the decision was made
    pub timestamp: DateTime<Utc>,

    /// Additional metadata for analytics
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Performance metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceMetrics>,

    /// Correlation IDs for distributed tracing
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub correlation_ids: HashMap<String, String>,
}

impl DecisionEvent {
    /// Current agent version
    pub const AGENT_VERSION: &'static str = "0.1.0";

    /// Agent identifier
    pub const AGENT_ID: &'static str = "config-validation-agent";

    /// Create a new decision event from validation output
    pub fn from_validation(
        inputs_hash: String,
        output: &ValidationOutput,
        execution_ref: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: Self::AGENT_ID.to_string(),
            agent_version: Self::AGENT_VERSION.to_string(),
            decision_type: DecisionType::ConfigValidationResult,
            inputs_hash,
            outputs: ValidationOutputs::from_output(output),
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

    /// Create a new decision event with custom values
    pub fn new(
        decision_type: DecisionType,
        inputs_hash: String,
        outputs: ValidationOutputs,
        confidence: f64,
        execution_ref: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            agent_id: Self::AGENT_ID.to_string(),
            agent_version: Self::AGENT_VERSION.to_string(),
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

    /// Add performance metrics
    pub fn with_performance(mut self, metrics: PerformanceMetrics) -> Self {
        self.performance = Some(metrics);
        self
    }

    /// Set constraints applied
    pub fn with_constraints(mut self, constraints: Vec<String>) -> Self {
        self.constraints_applied = constraints;
        self
    }

    /// Get a summary for logging
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} - valid={}, confidence={:.2}, errors={}, warnings={}",
            self.agent_id,
            self.decision_type.as_str(),
            self.outputs.is_valid,
            self.confidence,
            self.outputs.error_count,
            self.outputs.warning_count,
        )
    }

    /// Check if this is a high-confidence decision
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Check if this decision indicates a validation failure
    pub fn is_failure(&self) -> bool {
        !self.outputs.is_valid
    }
}

/// Types of decisions the validation agent can make
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// Result of validating a configuration value
    ConfigValidationResult,

    /// Result of validating a schema definition
    SchemaValidationResult,

    /// Result of checking cross-service compatibility
    CompatibilityCheckResult,

    /// Result of environment-specific validation
    EnvironmentValidationResult,

    /// Result of batch validation
    BatchValidationResult,
}

impl DecisionType {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ConfigValidationResult => "config_validation_result",
            Self::SchemaValidationResult => "schema_validation_result",
            Self::CompatibilityCheckResult => "compatibility_check_result",
            Self::EnvironmentValidationResult => "environment_validation_result",
            Self::BatchValidationResult => "batch_validation_result",
        }
    }
}

/// Structured validation outputs for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutputs {
    /// Overall validation result
    pub is_valid: bool,

    /// Count of errors
    pub error_count: u32,

    /// Count of warnings
    pub warning_count: u32,

    /// Count of info messages
    pub info_count: u32,

    /// Validation coverage percentage
    pub coverage: f64,

    /// Error summaries for quick analysis
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_codes: Vec<String>,

    /// Warning summaries for quick analysis
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warning_codes: Vec<String>,

    /// Detailed issues (optional, for debugging)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<IssueSummary>,

    /// Rules that were applied
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules_applied: Vec<String>,

    /// Fields that were validated
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields_validated: Vec<String>,

    /// Fields that were skipped (no rules applied)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields_skipped: Vec<String>,
}

impl ValidationOutputs {
    /// Create from a ValidationOutput
    pub fn from_output(output: &ValidationOutput) -> Self {
        Self {
            is_valid: output.is_valid,
            error_count: output.errors.len() as u32,
            warning_count: output.warnings.len() as u32,
            info_count: output.info.len() as u32,
            coverage: output.coverage,
            error_codes: output.errors.iter().map(|e| e.code.clone()).collect(),
            warning_codes: output.warnings.iter().map(|w| w.code.clone()).collect(),
            issues: output
                .errors
                .iter()
                .chain(output.warnings.iter())
                .map(IssueSummary::from_issue)
                .collect(),
            rules_applied: output.rules_applied.clone(),
            fields_validated: Vec::new(), // Would be populated during validation
            fields_skipped: Vec::new(),
        }
    }

    /// Create a successful validation output
    pub fn success(rules_applied: Vec<String>, coverage: f64) -> Self {
        Self {
            is_valid: true,
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            coverage,
            error_codes: Vec::new(),
            warning_codes: Vec::new(),
            issues: Vec::new(),
            rules_applied,
            fields_validated: Vec::new(),
            fields_skipped: Vec::new(),
        }
    }

    /// Create a failed validation output
    pub fn failure(errors: Vec<IssueSummary>) -> Self {
        let error_codes = errors.iter().map(|e| e.code.clone()).collect();
        Self {
            is_valid: false,
            error_count: errors.len() as u32,
            warning_count: 0,
            info_count: 0,
            coverage: 1.0,
            error_codes,
            warning_codes: Vec::new(),
            issues: errors,
            rules_applied: Vec::new(),
            fields_validated: Vec::new(),
            fields_skipped: Vec::new(),
        }
    }
}

/// Summarized issue for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueSummary {
    /// Issue code
    pub code: String,

    /// Severity level
    pub severity: IssueSeverity,

    /// Path within configuration (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Rule that triggered this issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
}

impl IssueSummary {
    /// Create from a ValidationIssue
    pub fn from_issue(issue: &ValidationIssue) -> Self {
        Self {
            code: issue.code.clone(),
            severity: issue.severity,
            path: issue.path.clone(),
            rule_id: issue.rule_id.clone(),
        }
    }

    /// Create a new issue summary
    pub fn new(code: impl Into<String>, severity: IssueSeverity) -> Self {
        Self {
            code: code.into(),
            severity,
            path: None,
            rule_id: None,
        }
    }
}

/// Performance metrics for decision events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Duration of validation in milliseconds
    pub duration_ms: u64,

    /// Number of rules evaluated
    pub rules_evaluated: u32,

    /// Memory used during validation (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_used_bytes: Option<u64>,
}

impl PerformanceMetrics {
    /// Create new performance metrics
    pub fn new(duration_ms: u64, rules_evaluated: u32) -> Self {
        Self {
            duration_ms,
            rules_evaluated,
            memory_used_bytes: None,
        }
    }

    /// Set memory usage
    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory_used_bytes = Some(bytes);
        self
    }
}

/// Batch of decision events for bulk submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEventBatch {
    /// Batch identifier
    pub batch_id: Uuid,

    /// Events in this batch
    pub events: Vec<DecisionEvent>,

    /// Batch creation timestamp
    pub created_at: DateTime<Utc>,

    /// Source of the batch
    pub source: String,
}

impl DecisionEventBatch {
    /// Create a new batch
    pub fn new(events: Vec<DecisionEvent>, source: impl Into<String>) -> Self {
        Self {
            batch_id: Uuid::new_v4(),
            events,
            created_at: Utc::now(),
            source: source.into(),
        }
    }

    /// Get the number of events in this batch
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get count of failed validations
    pub fn failure_count(&self) -> usize {
        self.events.iter().filter(|e| e.is_failure()).count()
    }

    /// Get count of high confidence decisions
    pub fn high_confidence_count(&self) -> usize {
        self.events.iter().filter(|e| e.is_high_confidence()).count()
    }
}

/// Query parameters for retrieving decision events from ruvector-service
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionEventQuery {
    /// Filter by agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Filter by decision type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_type: Option<DecisionType>,

    /// Filter by validity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_valid: Option<bool>,

    /// Filter by minimum confidence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confidence: Option<f64>,

    /// Filter by time range start
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_timestamp: Option<DateTime<Utc>>,

    /// Filter by time range end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_timestamp: Option<DateTime<Utc>>,

    /// Filter by execution reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_ref: Option<String>,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: u32,

    /// Offset for pagination
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    100
}

impl DecisionEventQuery {
    /// Create a new query for this agent's decisions
    pub fn for_agent() -> Self {
        Self {
            agent_id: Some(DecisionEvent::AGENT_ID.to_string()),
            ..Default::default()
        }
    }

    /// Filter by decision type
    pub fn with_type(mut self, decision_type: DecisionType) -> Self {
        self.decision_type = Some(decision_type);
        self
    }

    /// Filter for failed validations only
    pub fn failures_only(mut self) -> Self {
        self.is_valid = Some(false);
        self
    }

    /// Filter by minimum confidence
    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = Some(confidence);
        self
    }

    /// Filter by time range
    pub fn in_time_range(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.from_timestamp = Some(from);
        self.to_timestamp = Some(to);
        self
    }

    /// Set pagination
    pub fn paginate(mut self, limit: u32, offset: u32) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ValidationOutput;

    #[test]
    fn test_decision_event_from_validation() {
        let output = ValidationOutput::success(
            Uuid::new_v4(),
            vec!["type_check".to_string(), "bounds_check".to_string()],
        )
        .with_coverage(0.95)
        .with_duration(42);

        let event = DecisionEvent::from_validation(
            "abc123hash".to_string(),
            &output,
            "req-12345".to_string(),
        );

        assert_eq!(event.agent_id, DecisionEvent::AGENT_ID);
        assert!(event.outputs.is_valid);
        assert_eq!(event.outputs.rules_applied.len(), 2);
        assert!(event.confidence >= 0.8);
    }

    #[test]
    fn test_decision_event_summary() {
        let outputs = ValidationOutputs::failure(vec![
            IssueSummary::new("TYPE_MISMATCH", IssueSeverity::Error),
        ]);

        let event = DecisionEvent::new(
            DecisionType::ConfigValidationResult,
            "hash123".to_string(),
            outputs,
            0.9,
            "exec-ref".to_string(),
        );

        let summary = event.summary();
        assert!(summary.contains("config-validation-agent"));
        assert!(summary.contains("valid=false"));
        assert!(summary.contains("errors=1"));
    }

    #[test]
    fn test_validation_outputs_from_output() {
        let output = ValidationOutput::failure(
            Uuid::new_v4(),
            vec![
                super::super::ValidationIssue::error("E001", "Test error"),
                super::super::ValidationIssue::error("E002", "Another error"),
            ],
        );

        let outputs = ValidationOutputs::from_output(&output);

        assert!(!outputs.is_valid);
        assert_eq!(outputs.error_count, 2);
        assert_eq!(outputs.error_codes.len(), 2);
    }

    #[test]
    fn test_decision_event_batch() {
        let events = vec![
            DecisionEvent::new(
                DecisionType::ConfigValidationResult,
                "h1".to_string(),
                ValidationOutputs::success(vec![], 1.0),
                0.95,
                "r1".to_string(),
            ),
            DecisionEvent::new(
                DecisionType::ConfigValidationResult,
                "h2".to_string(),
                ValidationOutputs::failure(vec![IssueSummary::new("E1", IssueSeverity::Error)]),
                0.8,
                "r2".to_string(),
            ),
        ];

        let batch = DecisionEventBatch::new(events, "test-source");

        assert_eq!(batch.len(), 2);
        assert_eq!(batch.failure_count(), 1);
        assert_eq!(batch.high_confidence_count(), 2);
    }

    #[test]
    fn test_decision_event_query() {
        let query = DecisionEventQuery::for_agent()
            .with_type(DecisionType::ConfigValidationResult)
            .failures_only()
            .with_min_confidence(0.5)
            .paginate(50, 0);

        assert_eq!(query.agent_id, Some(DecisionEvent::AGENT_ID.to_string()));
        assert_eq!(query.decision_type, Some(DecisionType::ConfigValidationResult));
        assert_eq!(query.is_valid, Some(false));
        assert_eq!(query.limit, 50);
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new(150, 10)
            .with_memory(1024 * 1024);

        assert_eq!(metrics.duration_ms, 150);
        assert_eq!(metrics.rules_evaluated, 10);
        assert_eq!(metrics.memory_used_bytes, Some(1024 * 1024));
    }
}
