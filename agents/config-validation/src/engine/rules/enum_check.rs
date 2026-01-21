//! Enum value validation rules
//!
//! This module provides rules for validating that configuration values
//! are from a set of allowed values.

use async_trait::async_trait;
use std::collections::HashSet;

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use crate::ConfigValue;

/// Rule for validating enum values (allowed value sets)
pub struct EnumRule {
    id: String,
    name: String,
    field_path: String,
    /// Set of allowed string values
    allowed_strings: HashSet<String>,
    /// Whether comparison is case-insensitive
    case_insensitive: bool,
    /// Severity for violations
    severity: Severity,
}

impl EnumRule {
    /// Create a new enum rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            allowed_strings: HashSet::new(),
            case_insensitive: false,
            severity: Severity::Error,
        }
    }

    /// Add an allowed value
    pub fn allow(mut self, value: impl Into<String>) -> Self {
        self.allowed_strings.insert(value.into());
        self
    }

    /// Add multiple allowed values
    pub fn allow_all<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for value in values {
            self.allowed_strings.insert(value.into());
        }
        self
    }

    /// Set case-insensitive comparison
    pub fn case_insensitive(mut self, case_insensitive: bool) -> Self {
        self.case_insensitive = case_insensitive;
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

    fn is_allowed(&self, value: &str) -> bool {
        if self.case_insensitive {
            let lower = value.to_lowercase();
            self.allowed_strings
                .iter()
                .any(|allowed| allowed.to_lowercase() == lower)
        } else {
            self.allowed_strings.contains(value)
        }
    }

    fn get_allowed_list(&self) -> String {
        let mut values: Vec<&str> = self.allowed_strings.iter().map(|s| s.as_str()).collect();
        values.sort();
        values.join(", ")
    }

    fn get_suggestions(&self, value: &str) -> Vec<String> {
        // Simple fuzzy matching for suggestions
        let lower = value.to_lowercase();
        let mut suggestions: Vec<_> = self
            .allowed_strings
            .iter()
            .filter(|allowed| {
                let allowed_lower = allowed.to_lowercase();
                allowed_lower.contains(&lower)
                    || lower.contains(&allowed_lower)
                    || levenshtein_distance(&lower, &allowed_lower) <= 2
            })
            .cloned()
            .collect();
        suggestions.sort();
        suggestions
    }
}

/// Simple Levenshtein distance for suggestion matching
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[async_trait]
impl Rule for EnumRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that values are from a set of allowed values"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Enum
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
                if !self.is_allowed(s) {
                    let mut finding = ValidationFinding::new(
                        &self.id,
                        RuleCategory::Enum,
                        self.severity,
                        format!("Invalid value '{}': not in allowed set", s),
                        &full_path,
                    )
                    .with_expected(format!("one of: {}", self.get_allowed_list()))
                    .with_actual(s.clone());

                    let suggestions = self.get_suggestions(s);
                    if !suggestions.is_empty() {
                        finding = finding.with_suggestion(format!(
                            "Did you mean: {}?",
                            suggestions.join(" or ")
                        ));
                    } else {
                        finding = finding.with_suggestion(format!(
                            "Use one of: {}",
                            self.get_allowed_list()
                        ));
                    }

                    findings.push(finding);
                }
            }
        }

        findings
    }
}

/// Rule for validating integer enum values
pub struct IntegerEnumRule {
    id: String,
    name: String,
    field_path: String,
    /// Set of allowed integer values
    allowed_values: HashSet<i64>,
    /// Value descriptions for better error messages
    value_descriptions: std::collections::HashMap<i64, String>,
    severity: Severity,
}

impl IntegerEnumRule {
    /// Create a new integer enum rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            allowed_values: HashSet::new(),
            value_descriptions: std::collections::HashMap::new(),
            severity: Severity::Error,
        }
    }

    /// Add an allowed value
    pub fn allow(mut self, value: i64) -> Self {
        self.allowed_values.insert(value);
        self
    }

    /// Add an allowed value with description
    pub fn allow_with_desc(mut self, value: i64, description: impl Into<String>) -> Self {
        self.allowed_values.insert(value);
        self.value_descriptions.insert(value, description.into());
        self
    }

    /// Add multiple allowed values
    pub fn allow_range(mut self, start: i64, end: i64) -> Self {
        for v in start..=end {
            self.allowed_values.insert(v);
        }
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

    fn get_allowed_list(&self) -> String {
        let mut values: Vec<_> = self.allowed_values.iter().collect();
        values.sort();

        if values.len() > 10 {
            // For large sets, show range
            format!(
                "{} to {} ({} values)",
                values.first().unwrap(),
                values.last().unwrap(),
                values.len()
            )
        } else {
            values
                .iter()
                .map(|v| {
                    if let Some(desc) = self.value_descriptions.get(v) {
                        format!("{} ({})", v, desc)
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[async_trait]
impl Rule for IntegerEnumRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that integer values are from a set of allowed values"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Enum
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
            if let ConfigValue::Integer(i) = field_value {
                if !self.allowed_values.contains(i) {
                    findings.push(
                        ValidationFinding::new(
                            &self.id,
                            RuleCategory::Enum,
                            self.severity,
                            format!("Invalid value {}: not in allowed set", i),
                            &full_path,
                        )
                        .with_expected(format!("one of: {}", self.get_allowed_list()))
                        .with_actual(i.to_string())
                        .with_suggestion(format!("Use one of: {}", self.get_allowed_list())),
                    );
                }
            }
        }

        findings
    }
}

/// Rule for validating array elements against an enum
pub struct ArrayEnumRule {
    id: String,
    name: String,
    field_path: String,
    allowed_values: HashSet<String>,
    case_insensitive: bool,
    /// Whether to allow duplicates in the array
    allow_duplicates: bool,
    severity: Severity,
}

impl ArrayEnumRule {
    /// Create a new array enum rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            allowed_values: HashSet::new(),
            case_insensitive: false,
            allow_duplicates: true,
            severity: Severity::Error,
        }
    }

    /// Add an allowed value
    pub fn allow(mut self, value: impl Into<String>) -> Self {
        self.allowed_values.insert(value.into());
        self
    }

    /// Add multiple allowed values
    pub fn allow_all<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for value in values {
            self.allowed_values.insert(value.into());
        }
        self
    }

    /// Set case-insensitive comparison
    pub fn case_insensitive(mut self, case_insensitive: bool) -> Self {
        self.case_insensitive = case_insensitive;
        self
    }

    /// Set whether to allow duplicates
    pub fn allow_duplicates(mut self, allow: bool) -> Self {
        self.allow_duplicates = allow;
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

    fn is_allowed(&self, value: &str) -> bool {
        if self.case_insensitive {
            let lower = value.to_lowercase();
            self.allowed_values
                .iter()
                .any(|allowed| allowed.to_lowercase() == lower)
        } else {
            self.allowed_values.contains(value)
        }
    }

    fn get_allowed_list(&self) -> String {
        let mut values: Vec<&str> = self.allowed_values.iter().map(|s| s.as_str()).collect();
        values.sort();
        values.join(", ")
    }
}

#[async_trait]
impl Rule for ArrayEnumRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that array elements are from a set of allowed values"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Enum
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
                let mut seen: HashSet<String> = HashSet::new();

                for (idx, item) in arr.iter().enumerate() {
                    if let ConfigValue::String(s) = item {
                        // Check if value is allowed
                        if !self.is_allowed(s) {
                            findings.push(
                                ValidationFinding::new(
                                    &self.id,
                                    RuleCategory::Enum,
                                    self.severity,
                                    format!("Invalid array element '{}' at index {}", s, idx),
                                    format!("{}[{}]", full_path, idx),
                                )
                                .with_expected(format!("one of: {}", self.get_allowed_list()))
                                .with_actual(s.clone()),
                            );
                        }

                        // Check for duplicates
                        if !self.allow_duplicates {
                            let key = if self.case_insensitive {
                                s.to_lowercase()
                            } else {
                                s.clone()
                            };
                            if seen.contains(&key) {
                                findings.push(
                                    ValidationFinding::new(
                                        &self.id,
                                        RuleCategory::Enum,
                                        Severity::Warning,
                                        format!("Duplicate array element '{}' at index {}", s, idx),
                                        format!("{}[{}]", full_path, idx),
                                    )
                                    .with_suggestion("Remove duplicate entries"),
                                );
                            }
                            seen.insert(key);
                        }
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
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("hello", "world"), 4);
        assert_eq!(levenshtein_distance("", "hello"), 5);
    }

    #[tokio::test]
    async fn test_enum_rule_valid() {
        let rule = EnumRule::new("enum_001", "Log Level", "level")
            .allow_all(["debug", "info", "warn", "error"]);

        let mut obj = HashMap::new();
        obj.insert("level".to_string(), ConfigValue::String("info".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_enum_rule_invalid() {
        let rule = EnumRule::new("enum_002", "Log Level", "level")
            .allow_all(["debug", "info", "warn", "error"]);

        let mut obj = HashMap::new();
        obj.insert("level".to_string(), ConfigValue::String("verbose".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("verbose"));
    }

    #[tokio::test]
    async fn test_enum_rule_case_insensitive() {
        let rule = EnumRule::new("enum_003", "Log Level", "level")
            .allow_all(["debug", "info", "warn", "error"])
            .case_insensitive(true);

        let mut obj = HashMap::new();
        obj.insert("level".to_string(), ConfigValue::String("INFO".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_enum_rule_suggestions() {
        let rule = EnumRule::new("enum_004", "Log Level", "level")
            .allow_all(["debug", "info", "warn", "error"]);

        let mut obj = HashMap::new();
        obj.insert("level".to_string(), ConfigValue::String("inf".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].suggestion.as_ref().unwrap().contains("info"));
    }

    #[tokio::test]
    async fn test_integer_enum_rule() {
        let rule = IntegerEnumRule::new("enum_005", "HTTP Status", "status")
            .allow_with_desc(200, "OK")
            .allow_with_desc(404, "Not Found")
            .allow_with_desc(500, "Server Error");

        let mut obj = HashMap::new();
        obj.insert("status".to_string(), ConfigValue::Integer(200));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_array_enum_rule() {
        let rule = ArrayEnumRule::new("enum_006", "Permissions", "permissions")
            .allow_all(["read", "write", "delete"]);

        let mut obj = HashMap::new();
        obj.insert(
            "permissions".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("read".to_string()),
                ConfigValue::String("write".to_string()),
            ]),
        );
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_array_enum_rule_duplicates() {
        let rule = ArrayEnumRule::new("enum_007", "Permissions", "permissions")
            .allow_all(["read", "write", "delete"])
            .allow_duplicates(false);

        let mut obj = HashMap::new();
        obj.insert(
            "permissions".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("read".to_string()),
                ConfigValue::String("read".to_string()),
            ]),
        );
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("Duplicate"));
    }
}
