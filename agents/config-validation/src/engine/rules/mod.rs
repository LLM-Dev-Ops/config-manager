//! Rule framework for configuration validation
//!
//! This module provides the core abstractions for defining and executing
//! validation rules against configuration values.

pub mod bounds;
pub mod compatibility;
pub mod deprecated;
pub mod enum_check;
pub mod environment;
pub mod required;
pub mod type_check;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Categories of validation rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleCategory {
    /// Required field validation - checks if mandatory fields are present
    Required,
    /// Type correctness validation - ensures values match expected types
    Type,
    /// Value bounds validation - checks numeric/string limits
    Bounds,
    /// Enum value validation - ensures values are from allowed sets
    Enum,
    /// Deprecated field detection - flags outdated configuration
    Deprecated,
    /// Environment-specific rules - dev/staging/prod constraints
    Environment,
    /// Cross-agent/service compatibility - ensures interoperability
    Compatibility,
}

impl fmt::Display for RuleCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleCategory::Required => write!(f, "required"),
            RuleCategory::Type => write!(f, "type"),
            RuleCategory::Bounds => write!(f, "bounds"),
            RuleCategory::Enum => write!(f, "enum"),
            RuleCategory::Deprecated => write!(f, "deprecated"),
            RuleCategory::Environment => write!(f, "environment"),
            RuleCategory::Compatibility => write!(f, "compatibility"),
        }
    }
}

/// Severity level for validation findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational - no action required
    Info,
    /// Warning - should be addressed but not blocking
    Warning,
    /// Error - must be fixed before deployment
    Error,
    /// Critical - security or stability risk
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

impl Default for Severity {
    fn default() -> Self {
        Severity::Error
    }
}

/// A single validation finding representing an issue detected during validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Unique identifier for the rule that generated this finding
    pub rule_id: String,
    /// Category of the validation rule
    pub category: RuleCategory,
    /// Severity level of the finding
    pub severity: Severity,
    /// Human-readable message describing the issue
    pub message: String,
    /// JSON path to the affected field (e.g., "database.connection.timeout")
    pub field_path: String,
    /// Expected value or type (if applicable)
    pub expected: Option<String>,
    /// Actual value found (if applicable)
    pub actual: Option<String>,
    /// Suggested fix or remediation
    pub suggestion: Option<String>,
    /// Additional context or metadata
    pub context: Option<serde_json::Value>,
}

impl ValidationFinding {
    /// Create a new validation finding
    pub fn new(
        rule_id: impl Into<String>,
        category: RuleCategory,
        severity: Severity,
        message: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            category,
            severity,
            message: message.into(),
            field_path: field_path.into(),
            expected: None,
            actual: None,
            suggestion: None,
            context: None,
        }
    }

    /// Set the expected value
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Set the actual value found
    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    /// Set a suggested fix
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Set additional context
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Check if this is a blocking finding (error or critical)
    pub fn is_blocking(&self) -> bool {
        matches!(self.severity, Severity::Error | Severity::Critical)
    }
}

impl fmt::Display for ValidationFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} at '{}': {}",
            self.severity, self.rule_id, self.field_path, self.message
        )
    }
}

/// Context provided to rules during evaluation
#[derive(Debug, Clone)]
pub struct RuleContext {
    /// The target environment being validated
    pub environment: crate::Environment,
    /// Namespace of the configuration
    pub namespace: String,
    /// Additional metadata for rule evaluation
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl RuleContext {
    /// Create a new rule context
    pub fn new(environment: crate::Environment, namespace: impl Into<String>) -> Self {
        Self {
            environment,
            namespace: namespace.into(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Trait for implementing validation rules
///
/// Rules are deterministic, pure validation logic that produce findings
/// without modifying the configuration. Each rule should be focused on
/// a single aspect of validation.
#[async_trait]
pub trait Rule: Send + Sync {
    /// Unique identifier for this rule
    fn id(&self) -> &str;

    /// Human-readable name for this rule
    fn name(&self) -> &str;

    /// Description of what this rule validates
    fn description(&self) -> &str;

    /// Category this rule belongs to
    fn category(&self) -> RuleCategory;

    /// Default severity for findings from this rule
    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    /// Check if this rule is applicable to the given context
    fn is_applicable(&self, _context: &RuleContext) -> bool {
        true
    }

    /// Evaluate the rule against a configuration value
    ///
    /// Returns a list of findings (may be empty if validation passes).
    /// This method is async to support complex validation that may require
    /// external lookups or cross-reference checks.
    async fn evaluate(
        &self,
        value: &crate::ConfigValue,
        path: &str,
        context: &RuleContext,
    ) -> Vec<ValidationFinding>;
}

/// A boxed rule for dynamic dispatch
pub type BoxedRule = Box<dyn Rule>;

/// Builder for creating ValidationFinding instances
pub struct FindingBuilder {
    rule_id: String,
    category: RuleCategory,
    severity: Severity,
    field_path: String,
}

impl FindingBuilder {
    /// Create a new finding builder
    pub fn new(rule_id: impl Into<String>, category: RuleCategory, field_path: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            category,
            severity: Severity::Error,
            field_path: field_path.into(),
        }
    }

    /// Set the severity level
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Build the finding with a message
    pub fn build(self, message: impl Into<String>) -> ValidationFinding {
        ValidationFinding::new(
            self.rule_id,
            self.category,
            self.severity,
            message,
            self.field_path,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn test_finding_display() {
        let finding = ValidationFinding::new(
            "required_field",
            RuleCategory::Required,
            Severity::Error,
            "Field is required but missing",
            "database.host",
        );
        let display = format!("{}", finding);
        assert!(display.contains("error"));
        assert!(display.contains("required_field"));
        assert!(display.contains("database.host"));
    }

    #[test]
    fn test_finding_is_blocking() {
        let error = ValidationFinding::new(
            "test",
            RuleCategory::Required,
            Severity::Error,
            "test",
            "test",
        );
        assert!(error.is_blocking());

        let warning = ValidationFinding::new(
            "test",
            RuleCategory::Required,
            Severity::Warning,
            "test",
            "test",
        );
        assert!(!warning.is_blocking());
    }

    #[test]
    fn test_finding_builder() {
        let finding = FindingBuilder::new("test_rule", RuleCategory::Type, "config.value")
            .severity(Severity::Warning)
            .build("Value has incorrect type");

        assert_eq!(finding.rule_id, "test_rule");
        assert_eq!(finding.category, RuleCategory::Type);
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.field_path, "config.value");
    }
}
