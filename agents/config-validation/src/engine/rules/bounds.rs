//! Value bounds validation rules
//!
//! This module provides rules for validating that configuration values
//! fall within acceptable bounds and constraints.

use async_trait::async_trait;

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use crate::ConfigValue;

/// Numeric bounds specification
#[derive(Debug, Clone)]
pub struct NumericBounds {
    /// Minimum value (inclusive)
    pub min: Option<f64>,
    /// Maximum value (inclusive)
    pub max: Option<f64>,
    /// Whether min is exclusive
    pub min_exclusive: bool,
    /// Whether max is exclusive
    pub max_exclusive: bool,
}

impl NumericBounds {
    /// Create unbounded bounds
    pub fn unbounded() -> Self {
        Self {
            min: None,
            max: None,
            min_exclusive: false,
            max_exclusive: false,
        }
    }

    /// Set minimum value (inclusive)
    pub fn min(mut self, value: f64) -> Self {
        self.min = Some(value);
        self.min_exclusive = false;
        self
    }

    /// Set minimum value (exclusive)
    pub fn min_exclusive(mut self, value: f64) -> Self {
        self.min = Some(value);
        self.min_exclusive = true;
        self
    }

    /// Set maximum value (inclusive)
    pub fn max(mut self, value: f64) -> Self {
        self.max = Some(value);
        self.max_exclusive = false;
        self
    }

    /// Set maximum value (exclusive)
    pub fn max_exclusive(mut self, value: f64) -> Self {
        self.max = Some(value);
        self.max_exclusive = true;
        self
    }

    /// Check if a value is within bounds
    pub fn check(&self, value: f64) -> BoundsCheckResult {
        if let Some(min) = self.min {
            let too_low = if self.min_exclusive {
                value <= min
            } else {
                value < min
            };
            if too_low {
                return BoundsCheckResult::BelowMinimum { value, min };
            }
        }

        if let Some(max) = self.max {
            let too_high = if self.max_exclusive {
                value >= max
            } else {
                value > max
            };
            if too_high {
                return BoundsCheckResult::AboveMaximum { value, max };
            }
        }

        BoundsCheckResult::WithinBounds
    }

    /// Get a description of the bounds
    pub fn describe(&self) -> String {
        match (&self.min, &self.max) {
            (Some(min), Some(max)) => {
                let min_bracket = if self.min_exclusive { "(" } else { "[" };
                let max_bracket = if self.max_exclusive { ")" } else { "]" };
                format!("{}{}, {}{}", min_bracket, min, max, max_bracket)
            }
            (Some(min), None) => {
                if self.min_exclusive {
                    format!("> {}", min)
                } else {
                    format!(">= {}", min)
                }
            }
            (None, Some(max)) => {
                if self.max_exclusive {
                    format!("< {}", max)
                } else {
                    format!("<= {}", max)
                }
            }
            (None, None) => "unbounded".to_string(),
        }
    }
}

/// Result of a bounds check
#[derive(Debug, Clone)]
pub enum BoundsCheckResult {
    WithinBounds,
    BelowMinimum { value: f64, min: f64 },
    AboveMaximum { value: f64, max: f64 },
}

/// Rule for validating numeric bounds
pub struct NumericBoundsRule {
    id: String,
    name: String,
    field_path: String,
    bounds: NumericBounds,
    severity: Severity,
}

impl NumericBoundsRule {
    /// Create a new numeric bounds rule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        field_path: impl Into<String>,
        bounds: NumericBounds,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            bounds,
            severity: Severity::Error,
        }
    }

    /// Set the severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
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

    fn extract_number(value: &ConfigValue) -> Option<f64> {
        match value {
            ConfigValue::Integer(i) => Some(*i as f64),
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }
}

#[async_trait]
impl Rule for NumericBoundsRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that numeric values are within specified bounds"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Bounds
    }

    fn default_severity(&self) -> Severity {
        self.severity
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        let full_path = if path.is_empty() {
            self.field_path.clone()
        } else {
            format!("{}.{}", path, self.field_path)
        };

        if let Some(field_value) = self.get_value_at_path(value, &self.field_path) {
            if let Some(num) = Self::extract_number(field_value) {
                match self.bounds.check(num) {
                    BoundsCheckResult::WithinBounds => {}
                    BoundsCheckResult::BelowMinimum { value, min } => {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!(
                                    "Value {} is below minimum {}",
                                    value,
                                    if self.bounds.min_exclusive {
                                        format!("(exclusive: {})", min)
                                    } else {
                                        min.to_string()
                                    }
                                ),
                                &full_path,
                            )
                            .with_expected(self.bounds.describe())
                            .with_actual(value.to_string())
                            .with_suggestion(format!(
                                "Set value to at least {}",
                                if self.bounds.min_exclusive {
                                    min + 1.0
                                } else {
                                    min
                                }
                            )),
                        );
                    }
                    BoundsCheckResult::AboveMaximum { value, max } => {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!(
                                    "Value {} exceeds maximum {}",
                                    value,
                                    if self.bounds.max_exclusive {
                                        format!("(exclusive: {})", max)
                                    } else {
                                        max.to_string()
                                    }
                                ),
                                &full_path,
                            )
                            .with_expected(self.bounds.describe())
                            .with_actual(value.to_string())
                            .with_suggestion(format!(
                                "Set value to at most {}",
                                if self.bounds.max_exclusive {
                                    max - 1.0
                                } else {
                                    max
                                }
                            )),
                        );
                    }
                }
            }
        }

        findings
    }
}

/// String length bounds specification
#[derive(Debug, Clone)]
pub struct StringLengthBounds {
    /// Minimum length
    pub min: Option<usize>,
    /// Maximum length
    pub max: Option<usize>,
}

impl StringLengthBounds {
    /// Create unbounded length bounds
    pub fn unbounded() -> Self {
        Self {
            min: None,
            max: None,
        }
    }

    /// Set minimum length
    pub fn min(mut self, len: usize) -> Self {
        self.min = Some(len);
        self
    }

    /// Set maximum length
    pub fn max(mut self, len: usize) -> Self {
        self.max = Some(len);
        self
    }

    /// Set exact length
    pub fn exact(len: usize) -> Self {
        Self {
            min: Some(len),
            max: Some(len),
        }
    }

    /// Check if a length is within bounds
    pub fn check(&self, len: usize) -> StringLengthCheckResult {
        if let Some(min) = self.min {
            if len < min {
                return StringLengthCheckResult::TooShort { len, min };
            }
        }
        if let Some(max) = self.max {
            if len > max {
                return StringLengthCheckResult::TooLong { len, max };
            }
        }
        StringLengthCheckResult::Valid
    }

    /// Get a description of the bounds
    pub fn describe(&self) -> String {
        match (self.min, self.max) {
            (Some(min), Some(max)) if min == max => format!("exactly {} characters", min),
            (Some(min), Some(max)) => format!("{}-{} characters", min, max),
            (Some(min), None) => format!("at least {} characters", min),
            (None, Some(max)) => format!("at most {} characters", max),
            (None, None) => "any length".to_string(),
        }
    }
}

/// Result of string length check
#[derive(Debug, Clone)]
pub enum StringLengthCheckResult {
    Valid,
    TooShort { len: usize, min: usize },
    TooLong { len: usize, max: usize },
}

/// Rule for validating string length bounds
pub struct StringLengthRule {
    id: String,
    name: String,
    field_path: String,
    bounds: StringLengthBounds,
    severity: Severity,
}

impl StringLengthRule {
    /// Create a new string length rule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        field_path: impl Into<String>,
        bounds: StringLengthBounds,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            bounds,
            severity: Severity::Error,
        }
    }

    /// Set the severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
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
}

#[async_trait]
impl Rule for StringLengthRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that string values are within length bounds"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Bounds
    }

    fn default_severity(&self) -> Severity {
        self.severity
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        let full_path = if path.is_empty() {
            self.field_path.clone()
        } else {
            format!("{}.{}", path, self.field_path)
        };

        if let Some(field_value) = self.get_value_at_path(value, &self.field_path) {
            if let ConfigValue::String(s) = field_value {
                match self.bounds.check(s.len()) {
                    StringLengthCheckResult::Valid => {}
                    StringLengthCheckResult::TooShort { len, min } => {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!(
                                    "String length {} is below minimum {}",
                                    len, min
                                ),
                                &full_path,
                            )
                            .with_expected(self.bounds.describe())
                            .with_actual(format!("{} characters", len))
                            .with_suggestion(format!(
                                "Provide a value with at least {} characters",
                                min
                            )),
                        );
                    }
                    StringLengthCheckResult::TooLong { len, max } => {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!(
                                    "String length {} exceeds maximum {}",
                                    len, max
                                ),
                                &full_path,
                            )
                            .with_expected(self.bounds.describe())
                            .with_actual(format!("{} characters", len))
                            .with_suggestion(format!(
                                "Shorten the value to at most {} characters",
                                max
                            )),
                        );
                    }
                }
            }
        }

        findings
    }
}

/// Rule for validating array size bounds
pub struct ArraySizeRule {
    id: String,
    name: String,
    field_path: String,
    min_size: Option<usize>,
    max_size: Option<usize>,
    severity: Severity,
}

impl ArraySizeRule {
    /// Create a new array size rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            min_size: None,
            max_size: None,
            severity: Severity::Error,
        }
    }

    /// Set minimum array size
    pub fn min_size(mut self, size: usize) -> Self {
        self.min_size = Some(size);
        self
    }

    /// Set maximum array size
    pub fn max_size(mut self, size: usize) -> Self {
        self.max_size = Some(size);
        self
    }

    /// Set the severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
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
}

#[async_trait]
impl Rule for ArraySizeRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that arrays have acceptable sizes"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Bounds
    }

    fn default_severity(&self) -> Severity {
        self.severity
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        _context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        let full_path = if path.is_empty() {
            self.field_path.clone()
        } else {
            format!("{}.{}", path, self.field_path)
        };

        if let Some(field_value) = self.get_value_at_path(value, &self.field_path) {
            if let ConfigValue::Array(arr) = field_value {
                let size = arr.len();

                if let Some(min) = self.min_size {
                    if size < min {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!("Array size {} is below minimum {}", size, min),
                                &full_path,
                            )
                            .with_expected(format!("at least {} elements", min))
                            .with_actual(format!("{} elements", size)),
                        );
                    }
                }

                if let Some(max) = self.max_size {
                    if size > max {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Bounds,
                                self.severity,
                                format!("Array size {} exceeds maximum {}", size, max),
                                &full_path,
                            )
                            .with_expected(format!("at most {} elements", max))
                            .with_actual(format!("{} elements", size)),
                        );
                    }
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

    #[test]
    fn test_numeric_bounds() {
        let bounds = NumericBounds::unbounded().min(0.0).max(100.0);

        assert!(matches!(bounds.check(50.0), BoundsCheckResult::WithinBounds));
        assert!(matches!(bounds.check(0.0), BoundsCheckResult::WithinBounds));
        assert!(matches!(bounds.check(100.0), BoundsCheckResult::WithinBounds));
        assert!(matches!(bounds.check(-1.0), BoundsCheckResult::BelowMinimum { .. }));
        assert!(matches!(bounds.check(101.0), BoundsCheckResult::AboveMaximum { .. }));
    }

    #[test]
    fn test_exclusive_bounds() {
        let bounds = NumericBounds::unbounded().min_exclusive(0.0).max_exclusive(100.0);

        assert!(matches!(bounds.check(50.0), BoundsCheckResult::WithinBounds));
        assert!(matches!(bounds.check(0.0), BoundsCheckResult::BelowMinimum { .. }));
        assert!(matches!(bounds.check(100.0), BoundsCheckResult::AboveMaximum { .. }));
    }

    #[tokio::test]
    async fn test_numeric_bounds_rule() {
        let rule = NumericBoundsRule::new(
            "bounds_001",
            "Port Bounds",
            "server.port",
            NumericBounds::unbounded().min(1.0).max(65535.0),
        );

        let mut server = HashMap::new();
        server.insert("port".to_string(), ConfigValue::Integer(8080));
        let mut obj = HashMap::new();
        obj.insert("server".to_string(), ConfigValue::Object(server));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_numeric_bounds_violation() {
        let rule = NumericBoundsRule::new(
            "bounds_002",
            "Port Bounds",
            "port",
            NumericBounds::unbounded().min(1.0).max(65535.0),
        );

        let mut obj = HashMap::new();
        obj.insert("port".to_string(), ConfigValue::Integer(70000));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("exceeds"));
    }

    #[test]
    fn test_string_length_bounds() {
        let bounds = StringLengthBounds::unbounded().min(3).max(10);

        assert!(matches!(bounds.check(5), StringLengthCheckResult::Valid));
        assert!(matches!(bounds.check(3), StringLengthCheckResult::Valid));
        assert!(matches!(bounds.check(10), StringLengthCheckResult::Valid));
        assert!(matches!(bounds.check(2), StringLengthCheckResult::TooShort { .. }));
        assert!(matches!(bounds.check(11), StringLengthCheckResult::TooLong { .. }));
    }

    #[tokio::test]
    async fn test_string_length_rule() {
        let rule = StringLengthRule::new(
            "len_001",
            "Name Length",
            "name",
            StringLengthBounds::unbounded().min(1).max(50),
        );

        let mut obj = HashMap::new();
        obj.insert("name".to_string(), ConfigValue::String("test".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_array_size_rule() {
        let rule = ArraySizeRule::new("arr_001", "Items Size", "items")
            .min_size(1)
            .max_size(10);

        let mut obj = HashMap::new();
        obj.insert(
            "items".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("a".to_string()),
                ConfigValue::String("b".to_string()),
            ]),
        );
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }
}
