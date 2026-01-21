//! Validation Engine for the Config Validation Agent
//!
//! This module provides the core validation engine that orchestrates
//! rule evaluation against configuration values.

pub mod rules;

use crate::{ConfigValue, Environment};
use rules::{BoxedRule, Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Result of a validation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the configuration is valid (no blocking findings)
    pub is_valid: bool,
    /// All findings from validation
    pub findings: Vec<ValidationFinding>,
    /// Number of rules evaluated
    pub rules_evaluated: usize,
    /// Number of rules that passed (no findings)
    pub rules_passed: usize,
    /// Number of rules that failed (produced findings)
    pub rules_failed: usize,
    /// Validation coverage (rules_evaluated / total_applicable_rules)
    pub coverage: f64,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Schema version used for validation
    pub schema_version: Option<String>,
    /// Environment validated against
    pub environment: Environment,
    /// Validation duration in milliseconds
    pub duration_ms: u64,
    /// Breakdown by category
    pub category_summary: HashMap<RuleCategory, CategorySummary>,
}

/// Summary for a single rule category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub rules_evaluated: usize,
    pub rules_passed: usize,
    pub findings_count: usize,
    pub blocking_count: usize,
}

/// The core validation engine
pub struct ValidationEngine {
    /// Registered validation rules
    rules: Vec<Arc<dyn Rule>>,
    /// Default schema version
    default_schema_version: Option<String>,
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationEngine {
    /// Create a new validation engine with default rules
    pub fn new() -> Self {
        let mut engine = Self {
            rules: Vec::new(),
            default_schema_version: None,
        };
        engine.register_default_rules();
        engine
    }

    /// Create an empty validation engine (no default rules)
    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            default_schema_version: None,
        }
    }

    /// Register default validation rules
    fn register_default_rules(&mut self) {
        // Required field rules
        self.register(Arc::new(rules::required::RequiredFieldsRule::new()));

        // Type checking rules
        self.register(Arc::new(rules::type_check::TypeCheckRule::new()));

        // Bounds validation rules
        self.register(Arc::new(rules::bounds::BoundsRule::new()));

        // Enum validation rules
        self.register(Arc::new(rules::enum_check::EnumRule::new()));

        // Deprecation detection rules
        self.register(Arc::new(rules::deprecated::DeprecatedFieldsRule::new()));

        // Environment-specific rules
        self.register(Arc::new(rules::environment::EnvironmentRule::new()));

        // Compatibility rules
        self.register(Arc::new(rules::compatibility::CompatibilityRule::new()));
    }

    /// Register a validation rule
    pub fn register(&mut self, rule: Arc<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Register a boxed rule
    pub fn register_boxed(&mut self, rule: BoxedRule) {
        self.rules.push(Arc::from(rule));
    }

    /// Set the default schema version
    pub fn with_schema_version(mut self, version: impl Into<String>) -> Self {
        self.default_schema_version = Some(version.into());
        self
    }

    /// Get all registered rules
    pub fn rules(&self) -> &[Arc<dyn Rule>] {
        &self.rules
    }

    /// Get rules by category
    pub fn rules_by_category(&self, category: RuleCategory) -> Vec<Arc<dyn Rule>> {
        self.rules
            .iter()
            .filter(|r| r.category() == category)
            .cloned()
            .collect()
    }

    /// Validate a configuration value
    ///
    /// This method is deterministic - the same input will always produce
    /// the same output. It does NOT modify the configuration in any way.
    pub async fn validate(
        &self,
        value: &ConfigValue,
        environment: Environment,
        namespace: &str,
    ) -> ValidationResult {
        let start = std::time::Instant::now();
        let context = RuleContext::new(environment, namespace);

        self.validate_with_context(value, &context).await.finalize(start.elapsed())
    }

    /// Validate with a custom context
    pub async fn validate_with_context(
        &self,
        value: &ConfigValue,
        context: &RuleContext,
    ) -> ValidationResultBuilder {
        let mut builder = ValidationResultBuilder::new(context.environment);
        builder.schema_version = self.default_schema_version.clone();

        // Filter applicable rules
        let applicable_rules: Vec<_> = self.rules
            .iter()
            .filter(|r| r.is_applicable(context))
            .collect();

        // Evaluate all rules
        for rule in &applicable_rules {
            let findings = rule.evaluate(value, "", context).await;
            let category = rule.category();

            builder.add_rule_result(rule.id(), category, findings);
        }

        builder
    }

    /// Validate multiple configurations for compatibility
    pub async fn validate_compatibility(
        &self,
        configs: &[(&str, &ConfigValue)],
        environment: Environment,
    ) -> ValidationResult {
        let start = std::time::Instant::now();
        let mut builder = ValidationResultBuilder::new(environment);
        builder.schema_version = self.default_schema_version.clone();

        // Get compatibility rules
        let compat_rules = self.rules_by_category(RuleCategory::Compatibility);

        for rule in &compat_rules {
            for (namespace, value) in configs {
                let context = RuleContext::new(environment, *namespace);
                if rule.is_applicable(&context) {
                    let findings = rule.evaluate(value, "", &context).await;
                    builder.add_rule_result(rule.id(), rule.category(), findings);
                }
            }
        }

        builder.finalize(start.elapsed())
    }
}

/// Builder for ValidationResult
pub struct ValidationResultBuilder {
    environment: Environment,
    schema_version: Option<String>,
    findings: Vec<ValidationFinding>,
    rules_evaluated: usize,
    rules_passed: usize,
    rules_failed: usize,
    category_summary: HashMap<RuleCategory, CategorySummary>,
}

impl ValidationResultBuilder {
    fn new(environment: Environment) -> Self {
        Self {
            environment,
            schema_version: None,
            findings: Vec::new(),
            rules_evaluated: 0,
            rules_passed: 0,
            rules_failed: 0,
            category_summary: HashMap::new(),
        }
    }

    fn add_rule_result(&mut self, rule_id: &str, category: RuleCategory, findings: Vec<ValidationFinding>) {
        self.rules_evaluated += 1;

        let summary = self.category_summary.entry(category).or_insert(CategorySummary {
            rules_evaluated: 0,
            rules_passed: 0,
            findings_count: 0,
            blocking_count: 0,
        });

        summary.rules_evaluated += 1;

        if findings.is_empty() {
            self.rules_passed += 1;
            summary.rules_passed += 1;
        } else {
            self.rules_failed += 1;
            let blocking = findings.iter().filter(|f| f.is_blocking()).count();
            summary.findings_count += findings.len();
            summary.blocking_count += blocking;
            self.findings.extend(findings);
        }
    }

    fn finalize(self, duration: std::time::Duration) -> ValidationResult {
        let is_valid = !self.findings.iter().any(|f| f.is_blocking());
        let coverage = if self.rules_evaluated > 0 {
            1.0 // All applicable rules were evaluated
        } else {
            0.0
        };

        // Calculate confidence based on:
        // - Coverage (how many rules were applicable and evaluated)
        // - Pass rate (how many rules passed without findings)
        // - Severity distribution (more critical issues = lower confidence)
        let pass_rate = if self.rules_evaluated > 0 {
            self.rules_passed as f64 / self.rules_evaluated as f64
        } else {
            1.0
        };

        let critical_count = self.findings.iter()
            .filter(|f| matches!(f.severity, Severity::Critical))
            .count();
        let error_count = self.findings.iter()
            .filter(|f| matches!(f.severity, Severity::Error))
            .count();

        let severity_penalty = (critical_count as f64 * 0.2) + (error_count as f64 * 0.1);
        let confidence = (coverage * 0.3 + pass_rate * 0.7 - severity_penalty).max(0.0).min(1.0);

        ValidationResult {
            is_valid,
            findings: self.findings,
            rules_evaluated: self.rules_evaluated,
            rules_passed: self.rules_passed,
            rules_failed: self.rules_failed,
            coverage,
            confidence,
            schema_version: self.schema_version,
            environment: self.environment,
            duration_ms: duration.as_millis() as u64,
            category_summary: self.category_summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_empty_engine() {
        let engine = ValidationEngine::empty();
        assert!(engine.rules().is_empty());
    }

    #[tokio::test]
    async fn test_default_engine_has_rules() {
        let engine = ValidationEngine::new();
        assert!(!engine.rules().is_empty());
    }

    #[tokio::test]
    async fn test_validation_is_deterministic() {
        let engine = ValidationEngine::new();
        let config = ConfigValue::Object(
            [("key".to_string(), ConfigValue::String("value".to_string()))]
                .into_iter()
                .collect()
        );

        let result1 = engine.validate(&config, Environment::Production, "test").await;
        let result2 = engine.validate(&config, Environment::Production, "test").await;

        assert_eq!(result1.is_valid, result2.is_valid);
        assert_eq!(result1.findings.len(), result2.findings.len());
        assert_eq!(result1.rules_evaluated, result2.rules_evaluated);
    }

    #[tokio::test]
    async fn test_valid_config_produces_valid_result() {
        let engine = ValidationEngine::empty();
        let config = ConfigValue::Object(
            [("database".to_string(), ConfigValue::Object(
                [("host".to_string(), ConfigValue::String("localhost".to_string()))]
                    .into_iter()
                    .collect()
            ))]
                .into_iter()
                .collect()
        );

        let result = engine.validate(&config, Environment::Development, "myapp").await;
        assert!(result.is_valid);
        assert!(result.findings.is_empty());
    }
}
