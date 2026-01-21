//! Required field validation rules
//!
//! This module provides rules for validating that required configuration
//! fields are present and non-null.

use async_trait::async_trait;
use std::collections::HashSet;

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use crate::ConfigValue;

/// Rule for validating required fields are present
pub struct RequiredFieldRule {
    /// Unique identifier for this rule instance
    id: String,
    /// Human-readable name
    name: String,
    /// Set of required field paths
    required_paths: HashSet<String>,
    /// Whether to check nested objects recursively
    recursive: bool,
}

impl RequiredFieldRule {
    /// Create a new required field rule
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            required_paths: HashSet::new(),
            recursive: true,
        }
    }

    /// Add a required field path
    pub fn with_required_path(mut self, path: impl Into<String>) -> Self {
        self.required_paths.insert(path.into());
        self
    }

    /// Add multiple required field paths
    pub fn with_required_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for path in paths {
            self.required_paths.insert(path.into());
        }
        self
    }

    /// Set whether to check recursively
    pub fn recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Check if a path exists in the configuration value
    fn path_exists(&self, value: &ConfigValue, path: &str) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        self.check_path_parts(value, &parts)
    }

    fn check_path_parts(&self, value: &ConfigValue, parts: &[&str]) -> bool {
        if parts.is_empty() {
            return true;
        }

        match value {
            ConfigValue::Object(map) => {
                if let Some(next_value) = map.get(parts[0]) {
                    self.check_path_parts(next_value, &parts[1..])
                } else {
                    false
                }
            }
            ConfigValue::Array(arr) => {
                // For arrays, check if the index exists
                if let Ok(index) = parts[0].parse::<usize>() {
                    if let Some(next_value) = arr.get(index) {
                        self.check_path_parts(next_value, &parts[1..])
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => parts.is_empty(),
        }
    }

    /// Get the value at a path, if it exists
    fn get_value_at_path<'a>(&self, value: &'a ConfigValue, path: &str) -> Option<&'a ConfigValue> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_value_parts(value, &parts)
    }

    fn get_value_parts<'a>(&self, value: &'a ConfigValue, parts: &[&str]) -> Option<&'a ConfigValue> {
        if parts.is_empty() {
            return Some(value);
        }

        match value {
            ConfigValue::Object(map) => {
                map.get(parts[0]).and_then(|v| self.get_value_parts(v, &parts[1..]))
            }
            ConfigValue::Array(arr) => {
                parts[0]
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| arr.get(i))
                    .and_then(|v| self.get_value_parts(v, &parts[1..]))
            }
            _ => None,
        }
    }

    /// Check if a value is considered empty
    fn is_empty_value(value: &ConfigValue) -> bool {
        match value {
            ConfigValue::String(s) => s.trim().is_empty(),
            ConfigValue::Array(arr) => arr.is_empty(),
            ConfigValue::Object(map) => map.is_empty(),
            _ => false,
        }
    }
}

#[async_trait]
impl Rule for RequiredFieldRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that required fields are present and non-empty"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Required
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        for required_path in &self.required_paths {
            // Compute the full path
            let full_path = if path.is_empty() {
                required_path.clone()
            } else {
                format!("{}.{}", path, required_path)
            };

            if !self.path_exists(value, required_path) {
                findings.push(
                    ValidationFinding::new(
                        &self.id,
                        RuleCategory::Required,
                        Severity::Error,
                        format!("Required field '{}' is missing", required_path),
                        &full_path,
                    )
                    .with_suggestion(format!("Add the required field '{}'", required_path)),
                );
            } else if let Some(field_value) = self.get_value_at_path(value, required_path) {
                // Check if the value is empty
                if Self::is_empty_value(field_value) {
                    findings.push(
                        ValidationFinding::new(
                            &self.id,
                            RuleCategory::Required,
                            Severity::Error,
                            format!("Required field '{}' is empty", required_path),
                            &full_path,
                        )
                        .with_suggestion(format!(
                            "Provide a non-empty value for '{}'",
                            required_path
                        )),
                    );
                }
            }
        }

        findings
    }
}

/// Rule for validating that configuration objects have no null values
pub struct NoNullValuesRule {
    id: String,
    name: String,
    /// Paths to exclude from null checking
    excluded_paths: HashSet<String>,
}

impl NoNullValuesRule {
    /// Create a new no-null-values rule
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            excluded_paths: HashSet::new(),
        }
    }

    /// Exclude a path from null checking
    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.excluded_paths.insert(path.into());
        self
    }

    /// Check for null/empty values recursively
    fn check_for_nulls(
        &self,
        value: &ConfigValue,
        current_path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        if self.excluded_paths.contains(current_path) {
            return;
        }

        match value {
            ConfigValue::Object(map) => {
                for (key, val) in map {
                    let nested_path = if current_path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", current_path, key)
                    };
                    self.check_for_nulls(val, &nested_path, findings);
                }
            }
            ConfigValue::Array(arr) => {
                for (idx, val) in arr.iter().enumerate() {
                    let nested_path = format!("{}[{}]", current_path, idx);
                    self.check_for_nulls(val, &nested_path, findings);
                }
            }
            ConfigValue::String(s) if s.trim().is_empty() => {
                findings.push(
                    ValidationFinding::new(
                        &self.id,
                        RuleCategory::Required,
                        Severity::Warning,
                        "Empty string value found",
                        current_path,
                    )
                    .with_actual("\"\"")
                    .with_suggestion("Provide a meaningful value or remove the field"),
                );
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Rule for NoNullValuesRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that configuration contains no null or empty values"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Required
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();
        self.check_for_nulls(value, path, &mut findings);
        findings
    }
}

/// Rule for validating conditional requirements
/// (e.g., if field A is present, field B is required)
pub struct ConditionalRequiredRule {
    id: String,
    name: String,
    /// Condition field path
    condition_path: String,
    /// Value that triggers the requirement (None means any value)
    condition_value: Option<ConfigValue>,
    /// Fields required when condition is met
    required_when_true: HashSet<String>,
}

impl ConditionalRequiredRule {
    /// Create a new conditional required rule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        condition_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            condition_path: condition_path.into(),
            condition_value: None,
            required_when_true: HashSet::new(),
        }
    }

    /// Set the value that triggers the requirement
    pub fn when_equals(mut self, value: ConfigValue) -> Self {
        self.condition_value = Some(value);
        self
    }

    /// Add a field that is required when condition is met
    pub fn then_require(mut self, path: impl Into<String>) -> Self {
        self.required_when_true.insert(path.into());
        self
    }

    /// Add multiple fields that are required when condition is met
    pub fn then_require_all<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for path in paths {
            self.required_when_true.insert(path.into());
        }
        self
    }

    fn get_value_at_path<'a>(&self, value: &'a ConfigValue, path: &str) -> Option<&'a ConfigValue> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_value_parts(value, &parts)
    }

    fn get_value_parts<'a>(&self, value: &'a ConfigValue, parts: &[&str]) -> Option<&'a ConfigValue> {
        if parts.is_empty() {
            return Some(value);
        }

        match value {
            ConfigValue::Object(map) => {
                map.get(parts[0]).and_then(|v| self.get_value_parts(v, &parts[1..]))
            }
            _ => None,
        }
    }

    fn values_match(a: &ConfigValue, b: &ConfigValue) -> bool {
        match (a, b) {
            (ConfigValue::String(sa), ConfigValue::String(sb)) => sa == sb,
            (ConfigValue::Integer(ia), ConfigValue::Integer(ib)) => ia == ib,
            (ConfigValue::Float(fa), ConfigValue::Float(fb)) => (fa - fb).abs() < f64::EPSILON,
            (ConfigValue::Boolean(ba), ConfigValue::Boolean(bb)) => ba == bb,
            _ => false,
        }
    }
}

#[async_trait]
impl Rule for ConditionalRequiredRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates conditional field requirements"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Required
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        // Check if condition is met
        let condition_met = match self.get_value_at_path(value, &self.condition_path) {
            Some(cond_value) => {
                if let Some(expected) = &self.condition_value {
                    Self::values_match(cond_value, expected)
                } else {
                    true // Any value triggers the condition
                }
            }
            None => false,
        };

        if condition_met {
            // Check required fields
            for required_path in &self.required_when_true {
                let full_path = if path.is_empty() {
                    required_path.clone()
                } else {
                    format!("{}.{}", path, required_path)
                };

                if self.get_value_at_path(value, required_path).is_none() {
                    let condition_desc = if let Some(expected) = &self.condition_value {
                        format!("'{}' equals {:?}", self.condition_path, expected)
                    } else {
                        format!("'{}' is present", self.condition_path)
                    };

                    findings.push(
                        ValidationFinding::new(
                            &self.id,
                            RuleCategory::Required,
                            Severity::Error,
                            format!(
                                "Field '{}' is required when {}",
                                required_path, condition_desc
                            ),
                            &full_path,
                        )
                        .with_suggestion(format!("Add the required field '{}'", required_path)),
                    );
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;
    use std::collections::HashMap;

    fn make_context() -> RuleContext {
        RuleContext::new(Environment::Development, "test")
    }

    #[tokio::test]
    async fn test_required_field_present() {
        let rule = RequiredFieldRule::new("req_001", "Test Required")
            .with_required_path("name");

        let mut obj = HashMap::new();
        obj.insert("name".to_string(), ConfigValue::String("test".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_required_field_missing() {
        let rule = RequiredFieldRule::new("req_001", "Test Required")
            .with_required_path("name");

        let obj = HashMap::new();
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("missing"));
    }

    #[tokio::test]
    async fn test_required_nested_field() {
        let rule = RequiredFieldRule::new("req_002", "Test Nested Required")
            .with_required_path("database.host");

        let mut db = HashMap::new();
        db.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
        let mut obj = HashMap::new();
        obj.insert("database".to_string(), ConfigValue::Object(db));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_required_field_empty() {
        let rule = RequiredFieldRule::new("req_003", "Test Empty")
            .with_required_path("name");

        let mut obj = HashMap::new();
        obj.insert("name".to_string(), ConfigValue::String("".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("empty"));
    }

    #[tokio::test]
    async fn test_conditional_required() {
        let rule = ConditionalRequiredRule::new("cond_001", "Conditional Test", "auth.enabled")
            .when_equals(ConfigValue::Boolean(true))
            .then_require("auth.secret");

        // Auth enabled but secret missing
        let mut auth = HashMap::new();
        auth.insert("enabled".to_string(), ConfigValue::Boolean(true));
        let mut obj = HashMap::new();
        obj.insert("auth".to_string(), ConfigValue::Object(auth));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("auth.secret"));
    }

    #[tokio::test]
    async fn test_conditional_not_triggered() {
        let rule = ConditionalRequiredRule::new("cond_002", "Conditional Test", "auth.enabled")
            .when_equals(ConfigValue::Boolean(true))
            .then_require("auth.secret");

        // Auth disabled, secret not required
        let mut auth = HashMap::new();
        auth.insert("enabled".to_string(), ConfigValue::Boolean(false));
        let mut obj = HashMap::new();
        obj.insert("auth".to_string(), ConfigValue::Object(auth));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }
}
