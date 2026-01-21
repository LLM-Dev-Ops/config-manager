//! Config Validation Agent Contract Definitions
//!
//! This module defines the contracts for the Config Validation Agent,
//! which provides read-only validation of configuration objects without
//! any enforcement or modification capabilities.
//!
//! # Architecture
//!
//! The validation agent follows a pure functional design:
//! - Receives configuration objects via `ValidationInput`
//! - Applies validation rules defined by `ValidationRule` trait
//! - Returns validation results via `ValidationOutput`
//! - Emits `DecisionEvent` records to ruvector-service for analytics
//!
//! # Design Principles
//!
//! - **Read-only**: No enforcement or modification of configurations
//! - **Stateless**: Each validation is independent
//! - **Traceable**: All decisions are recorded via DecisionEvent
//! - **Composable**: Rules can be combined and prioritized

pub mod schemas;
pub mod decision_event;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Re-export core types
pub use schemas::{
    ConfigSchema, FieldRule, FieldType, EnvironmentRule, CompatibilityRule,
    SchemaDefinition, ValidationConstraint, DeprecationInfo,
};
pub use decision_event::{DecisionEvent, ValidationOutputs};

/// Input for configuration validation
///
/// Contains the configuration object to be validated along with
/// context about the environment and validation requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInput {
    /// Unique identifier for this validation request
    pub request_id: Uuid,

    /// The configuration namespace being validated
    pub namespace: String,

    /// The configuration key being validated
    pub key: String,

    /// The configuration value to validate
    pub value: ConfigValueRef,

    /// Target environment for validation
    pub environment: EnvironmentRef,

    /// Schema to validate against (if provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<ConfigSchema>,

    /// Additional validation rules to apply
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_rules: Vec<RuleRef>,

    /// Context metadata for the validation
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, String>,

    /// Timestamp when the validation was requested
    pub requested_at: DateTime<Utc>,

    /// User or service requesting validation
    pub requested_by: String,
}

impl ValidationInput {
    /// Create a new validation input
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: ConfigValueRef,
        environment: EnvironmentRef,
        requested_by: impl Into<String>,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            namespace: namespace.into(),
            key: key.into(),
            value,
            environment,
            schema: None,
            additional_rules: Vec::new(),
            context: HashMap::new(),
            requested_at: Utc::now(),
            requested_by: requested_by.into(),
        }
    }

    /// Add a schema to validate against
    pub fn with_schema(mut self, schema: ConfigSchema) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Add additional validation rules
    pub fn with_rules(mut self, rules: Vec<RuleRef>) -> Self {
        self.additional_rules = rules;
        self
    }

    /// Add context metadata
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Compute a deterministic hash of the inputs for traceability
    pub fn compute_hash(&self) -> String {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        self.namespace.hash(&mut hasher);
        self.key.hash(&mut hasher);
        format!("{:?}", self.value).hash(&mut hasher);
        format!("{:?}", self.environment).hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Reference to a configuration value (mirrors llm-config-storage ConfigValue)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ConfigValueRef {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValueRef>),
    Object(HashMap<String, ConfigValueRef>),
    /// Secret values are represented by their encrypted form indicator
    Secret { encrypted: bool },
    /// Null/missing value
    Null,
}

impl ConfigValueRef {
    /// Get the type name of this value
    pub fn type_name(&self) -> &'static str {
        match self {
            ConfigValueRef::String(_) => "string",
            ConfigValueRef::Integer(_) => "integer",
            ConfigValueRef::Float(_) => "float",
            ConfigValueRef::Boolean(_) => "boolean",
            ConfigValueRef::Array(_) => "array",
            ConfigValueRef::Object(_) => "object",
            ConfigValueRef::Secret { .. } => "secret",
            ConfigValueRef::Null => "null",
        }
    }

    /// Check if this is a secret value
    pub fn is_secret(&self) -> bool {
        matches!(self, ConfigValueRef::Secret { .. })
    }
}

/// Reference to an environment (mirrors llm-config-storage Environment)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentRef {
    Base,
    Development,
    Staging,
    Production,
    Edge,
}

impl std::fmt::Display for EnvironmentRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvironmentRef::Base => write!(f, "base"),
            EnvironmentRef::Development => write!(f, "development"),
            EnvironmentRef::Staging => write!(f, "staging"),
            EnvironmentRef::Production => write!(f, "production"),
            EnvironmentRef::Edge => write!(f, "edge"),
        }
    }
}

/// Reference to a validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRef {
    /// Unique identifier for the rule
    pub rule_id: String,
    /// Rule type/category
    pub rule_type: String,
    /// Rule configuration parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Output from configuration validation
///
/// Contains the complete validation results including all issues found,
/// warnings, and metadata about the validation process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutput {
    /// Reference to the original request
    pub request_id: Uuid,

    /// Overall validation result
    pub is_valid: bool,

    /// Validation issues found (errors that cause validation to fail)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationIssue>,

    /// Warnings (non-blocking issues)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ValidationIssue>,

    /// Informational messages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub info: Vec<ValidationIssue>,

    /// Rules that were applied during validation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules_applied: Vec<String>,

    /// Constraints that were checked
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints_checked: Vec<String>,

    /// Validation coverage as a percentage (0.0-1.0)
    pub coverage: f64,

    /// Timestamp when validation completed
    pub completed_at: DateTime<Utc>,

    /// Duration of validation in milliseconds
    pub duration_ms: u64,

    /// Additional metadata about the validation
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ValidationOutput {
    /// Create a successful validation output
    pub fn success(request_id: Uuid, rules_applied: Vec<String>) -> Self {
        Self {
            request_id,
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
            rules_applied,
            constraints_checked: Vec::new(),
            coverage: 1.0,
            completed_at: Utc::now(),
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed validation output
    pub fn failure(request_id: Uuid, errors: Vec<ValidationIssue>) -> Self {
        Self {
            request_id,
            is_valid: false,
            errors,
            warnings: Vec::new(),
            info: Vec::new(),
            rules_applied: Vec::new(),
            constraints_checked: Vec::new(),
            coverage: 1.0,
            completed_at: Utc::now(),
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Add a warning to the output
    pub fn with_warning(mut self, warning: ValidationIssue) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Set the duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Set coverage
    pub fn with_coverage(mut self, coverage: f64) -> Self {
        self.coverage = coverage.clamp(0.0, 1.0);
        self
    }

    /// Calculate confidence score based on validation coverage and results
    pub fn confidence(&self) -> f64 {
        // Base confidence from coverage
        let mut confidence = self.coverage;

        // Reduce confidence if there are warnings
        confidence -= self.warnings.len() as f64 * 0.05;

        // Reduce confidence if few rules were applied
        if self.rules_applied.len() < 3 {
            confidence -= 0.1;
        }

        confidence.clamp(0.0, 1.0)
    }
}

/// A single validation issue (error, warning, or info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Issue code for categorization
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Severity level
    pub severity: IssueSeverity,

    /// Path within the configuration (for nested values)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// The rule that triggered this issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,

    /// Expected value or format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,

    /// Actual value found
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,

    /// Suggested fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl ValidationIssue {
    /// Create a new error issue
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: IssueSeverity::Error,
            path: None,
            rule_id: None,
            expected: None,
            actual: None,
            suggestion: None,
        }
    }

    /// Create a new warning issue
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: IssueSeverity::Warning,
            path: None,
            rule_id: None,
            expected: None,
            actual: None,
            suggestion: None,
        }
    }

    /// Create a new info issue
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: IssueSeverity::Info,
            path: None,
            rule_id: None,
            expected: None,
            actual: None,
            suggestion: None,
        }
    }

    /// Set the path for this issue
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set the rule that triggered this issue
    pub fn with_rule(mut self, rule_id: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self
    }

    /// Set expected and actual values
    pub fn with_values(
        mut self,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    /// Set a suggestion for fixing the issue
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Severity level for validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    /// Blocking error - validation fails
    Error,
    /// Non-blocking warning
    Warning,
    /// Informational message
    Info,
}

/// Trait for implementing validation rules
///
/// All validation rules must implement this trait to participate in
/// the validation process. Rules are stateless and read-only.
pub trait ValidationRule: Send + Sync {
    /// Unique identifier for this rule
    fn id(&self) -> &str;

    /// Human-readable name for this rule
    fn name(&self) -> &str;

    /// Description of what this rule validates
    fn description(&self) -> &str;

    /// Validate a configuration value
    ///
    /// # Arguments
    ///
    /// * `input` - The validation input containing the value and context
    ///
    /// # Returns
    ///
    /// A vector of validation issues found. Empty vector means the value
    /// passes this rule.
    fn validate(&self, input: &ValidationInput) -> Vec<ValidationIssue>;

    /// Check if this rule applies to the given input
    ///
    /// Default implementation returns true for all inputs.
    fn applies_to(&self, input: &ValidationInput) -> bool {
        let _ = input;
        true
    }

    /// Get the priority of this rule (higher = runs first)
    ///
    /// Default priority is 100.
    fn priority(&self) -> i32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_input_creation() {
        let input = ValidationInput::new(
            "app/database",
            "connection_string",
            ConfigValueRef::String("postgres://localhost/db".to_string()),
            EnvironmentRef::Development,
            "test-user",
        );

        assert_eq!(input.namespace, "app/database");
        assert_eq!(input.key, "connection_string");
        assert!(!input.compute_hash().is_empty());
    }

    #[test]
    fn test_validation_output_success() {
        let request_id = Uuid::new_v4();
        let output = ValidationOutput::success(
            request_id,
            vec!["type_check".to_string(), "bounds_check".to_string()],
        );

        assert!(output.is_valid);
        assert!(output.errors.is_empty());
        assert_eq!(output.rules_applied.len(), 2);
    }

    #[test]
    fn test_validation_issue_creation() {
        let issue = ValidationIssue::error("TYPE_MISMATCH", "Expected string, got integer")
            .with_path("config.database.port")
            .with_values("string", "integer")
            .with_suggestion("Convert the value to a string");

        assert_eq!(issue.code, "TYPE_MISMATCH");
        assert_eq!(issue.severity, IssueSeverity::Error);
        assert!(issue.suggestion.is_some());
    }

    #[test]
    fn test_config_value_ref_type_name() {
        assert_eq!(ConfigValueRef::String("test".to_string()).type_name(), "string");
        assert_eq!(ConfigValueRef::Integer(42).type_name(), "integer");
        assert_eq!(ConfigValueRef::Boolean(true).type_name(), "boolean");
        assert_eq!(ConfigValueRef::Secret { encrypted: true }.type_name(), "secret");
    }

    #[test]
    fn test_environment_ref_display() {
        assert_eq!(format!("{}", EnvironmentRef::Production), "production");
        assert_eq!(format!("{}", EnvironmentRef::Development), "development");
    }

    #[test]
    fn test_validation_output_confidence() {
        let request_id = Uuid::new_v4();

        // High confidence - many rules, full coverage
        let output = ValidationOutput::success(
            request_id,
            vec!["rule1".to_string(), "rule2".to_string(), "rule3".to_string()],
        ).with_coverage(1.0);
        assert!(output.confidence() >= 0.9);

        // Lower confidence - few rules
        let output2 = ValidationOutput::success(
            request_id,
            vec!["rule1".to_string()],
        ).with_coverage(0.5);
        assert!(output2.confidence() < 0.5);
    }
}
