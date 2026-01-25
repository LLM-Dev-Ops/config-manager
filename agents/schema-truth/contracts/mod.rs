//! Schema Truth Agent Contracts
//!
//! Defines configuration truth and schema truth for deterministic validation.

mod decision_event;
mod schemas;

pub use decision_event::*;
pub use schemas::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Input for schema truth validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationInput {
    /// Unique request identifier
    pub request_id: Uuid,

    /// Schema definition to validate
    pub schema: SchemaDefinition,

    /// Optional parent schema for inheritance checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_schema: Option<SchemaDefinition>,

    /// Validation context
    #[serde(default)]
    pub context: HashMap<String, String>,

    /// Request timestamp
    pub requested_at: DateTime<Utc>,

    /// Requester identity
    pub requested_by: String,
}

/// Output from schema truth validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationOutput {
    /// Request ID correlation
    pub request_id: Uuid,

    /// Whether schema is valid
    pub is_valid: bool,

    /// Schema violations found
    pub violations: Vec<SchemaViolation>,

    /// Warnings (non-blocking)
    pub warnings: Vec<SchemaViolation>,

    /// Rules that were applied
    pub rules_applied: Vec<String>,

    /// Constraints checked
    pub constraints_checked: Vec<String>,

    /// Validation coverage (0.0-1.0)
    pub coverage: f64,

    /// Completion timestamp
    pub completed_at: DateTime<Utc>,

    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl SchemaValidationOutput {
    /// Create successful output
    pub fn success(request_id: Uuid, rules_applied: Vec<String>) -> Self {
        Self {
            request_id,
            is_valid: true,
            violations: Vec::new(),
            warnings: Vec::new(),
            rules_applied,
            constraints_checked: Vec::new(),
            coverage: 1.0,
            completed_at: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create failure output
    pub fn failure(request_id: Uuid, violations: Vec<SchemaViolation>) -> Self {
        Self {
            request_id,
            is_valid: false,
            violations,
            warnings: Vec::new(),
            rules_applied: Vec::new(),
            constraints_checked: Vec::new(),
            coverage: 1.0,
            completed_at: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Set duration
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Set coverage
    pub fn with_coverage(mut self, coverage: f64) -> Self {
        self.coverage = coverage.clamp(0.0, 1.0);
        self
    }

    /// Calculate confidence score
    pub fn confidence(&self) -> f64 {
        let mut conf = self.coverage;
        // Penalty for warnings
        conf -= self.warnings.len() as f64 * 0.05;
        // Penalty for few rules
        if self.rules_applied.len() < 3 {
            conf -= 0.1;
        }
        conf.clamp(0.0, 1.0)
    }
}

/// Schema violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaViolation {
    /// Violation code
    pub code: String,

    /// Severity level
    pub severity: ViolationSeverity,

    /// Human-readable message
    pub message: String,

    /// Path within schema (JSON path)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Expected value/type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,

    /// Actual value/type found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,

    /// Suggested fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,

    /// Rule that triggered this
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
}

impl SchemaViolation {
    /// Create error violation
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: ViolationSeverity::Error,
            message: message.into(),
            path: None,
            expected: None,
            actual: None,
            suggestion: None,
            rule_id: None,
        }
    }

    /// Create warning violation
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: ViolationSeverity::Warning,
            message: message.into(),
            path: None,
            expected: None,
            actual: None,
            suggestion: None,
            rule_id: None,
        }
    }

    /// Set path
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set expected/actual
    pub fn with_expected_actual(
        mut self,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }
}

/// Severity levels for violations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}
