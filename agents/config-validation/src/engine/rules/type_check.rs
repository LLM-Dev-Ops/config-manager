//! Type correctness validation rules
//!
//! This module provides rules for validating that configuration values
//! match their expected types.

use async_trait::async_trait;
use std::collections::HashMap;

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use crate::ConfigValue;

/// Expected type specification for a field
#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedType {
    String,
    Integer,
    Float,
    Boolean,
    Array(Option<Box<ExpectedType>>), // Optional inner type
    Object(Option<HashMap<String, ExpectedType>>), // Optional schema
    OneOf(Vec<ExpectedType>), // Union type
    Any,
}

impl ExpectedType {
    /// Get a human-readable name for this type
    pub fn type_name(&self) -> String {
        match self {
            ExpectedType::String => "string".to_string(),
            ExpectedType::Integer => "integer".to_string(),
            ExpectedType::Float => "float".to_string(),
            ExpectedType::Boolean => "boolean".to_string(),
            ExpectedType::Array(inner) => {
                if let Some(t) = inner {
                    format!("array<{}>", t.type_name())
                } else {
                    "array".to_string()
                }
            }
            ExpectedType::Object(_) => "object".to_string(),
            ExpectedType::OneOf(types) => {
                let names: Vec<String> = types.iter().map(|t| t.type_name()).collect();
                names.join(" | ")
            }
            ExpectedType::Any => "any".to_string(),
        }
    }

    /// Check if a ConfigValue matches this expected type
    pub fn matches(&self, value: &ConfigValue) -> bool {
        match (self, value) {
            (ExpectedType::Any, _) => true,
            (ExpectedType::String, ConfigValue::String(_)) => true,
            (ExpectedType::Integer, ConfigValue::Integer(_)) => true,
            (ExpectedType::Float, ConfigValue::Float(_)) => true,
            (ExpectedType::Float, ConfigValue::Integer(_)) => true, // Allow int where float expected
            (ExpectedType::Boolean, ConfigValue::Boolean(_)) => true,
            (ExpectedType::Array(inner), ConfigValue::Array(arr)) => {
                if let Some(inner_type) = inner {
                    arr.iter().all(|v| inner_type.matches(v))
                } else {
                    true
                }
            }
            (ExpectedType::Object(schema), ConfigValue::Object(map)) => {
                if let Some(schema) = schema {
                    schema.iter().all(|(key, expected)| {
                        map.get(key).map(|v| expected.matches(v)).unwrap_or(true)
                    })
                } else {
                    true
                }
            }
            (ExpectedType::OneOf(types), value) => types.iter().any(|t| t.matches(value)),
            _ => false,
        }
    }
}

/// Get the actual type name from a ConfigValue
fn actual_type_name(value: &ConfigValue) -> &'static str {
    match value {
        ConfigValue::String(_) => "string",
        ConfigValue::Integer(_) => "integer",
        ConfigValue::Float(_) => "float",
        ConfigValue::Boolean(_) => "boolean",
        ConfigValue::Array(_) => "array",
        ConfigValue::Object(_) => "object",
        ConfigValue::Secret(_) => "secret",
    }
}

/// Rule for validating field types
pub struct TypeCheckRule {
    id: String,
    name: String,
    /// Map of field paths to expected types
    type_specs: HashMap<String, ExpectedType>,
    /// Whether to allow additional fields not in the spec
    allow_unknown_fields: bool,
}

impl TypeCheckRule {
    /// Create a new type check rule
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_specs: HashMap::new(),
            allow_unknown_fields: true,
        }
    }

    /// Add a type specification for a field
    pub fn expect_type(mut self, path: impl Into<String>, expected: ExpectedType) -> Self {
        self.type_specs.insert(path.into(), expected);
        self
    }

    /// Set whether to allow unknown fields
    pub fn allow_unknown(mut self, allow: bool) -> Self {
        self.allow_unknown_fields = allow;
        self
    }

    fn get_value_at_path<'a>(&self, value: &'a ConfigValue, path: &str) -> Option<&'a ConfigValue> {
        if path.is_empty() {
            return Some(value);
        }

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
}

#[async_trait]
impl Rule for TypeCheckRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that configuration values match expected types"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Type
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

        for (field_path, expected_type) in &self.type_specs {
            let full_path = if path.is_empty() {
                field_path.clone()
            } else {
                format!("{}.{}", path, field_path)
            };

            if let Some(field_value) = self.get_value_at_path(value, field_path) {
                if !expected_type.matches(field_value) {
                    findings.push(
                        ValidationFinding::new(
                            &self.id,
                            RuleCategory::Type,
                            Severity::Error,
                            format!(
                                "Type mismatch: expected {}, found {}",
                                expected_type.type_name(),
                                actual_type_name(field_value)
                            ),
                            &full_path,
                        )
                        .with_expected(expected_type.type_name())
                        .with_actual(actual_type_name(field_value))
                        .with_suggestion(format!(
                            "Change the value to type {}",
                            expected_type.type_name()
                        )),
                    );
                }
            }
        }

        findings
    }
}

/// Rule for validating string format patterns
pub struct StringFormatRule {
    id: String,
    name: String,
    /// Field path to validate
    field_path: String,
    /// Expected format
    format: StringFormat,
}

/// Common string formats
#[derive(Debug, Clone)]
pub enum StringFormat {
    /// Email address
    Email,
    /// URL
    Url,
    /// IPv4 address
    Ipv4,
    /// IPv6 address
    Ipv6,
    /// UUID
    Uuid,
    /// ISO 8601 date
    Date,
    /// ISO 8601 datetime
    DateTime,
    /// Semantic version (e.g., 1.2.3)
    SemVer,
    /// Custom regex pattern
    Pattern(regex::Regex),
}

impl StringFormat {
    /// Get the format name
    pub fn name(&self) -> &str {
        match self {
            StringFormat::Email => "email",
            StringFormat::Url => "url",
            StringFormat::Ipv4 => "ipv4",
            StringFormat::Ipv6 => "ipv6",
            StringFormat::Uuid => "uuid",
            StringFormat::Date => "date",
            StringFormat::DateTime => "datetime",
            StringFormat::SemVer => "semver",
            StringFormat::Pattern(_) => "pattern",
        }
    }

    /// Check if a string matches this format
    pub fn matches(&self, value: &str) -> bool {
        match self {
            StringFormat::Email => {
                // Simple email validation
                let parts: Vec<&str> = value.split('@').collect();
                parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
            }
            StringFormat::Url => {
                value.starts_with("http://")
                    || value.starts_with("https://")
                    || value.starts_with("ftp://")
            }
            StringFormat::Ipv4 => {
                let parts: Vec<&str> = value.split('.').collect();
                parts.len() == 4
                    && parts
                        .iter()
                        .all(|p| p.parse::<u8>().is_ok())
            }
            StringFormat::Ipv6 => {
                // Simplified IPv6 validation
                value.contains(':')
                    && value
                        .split(':')
                        .all(|p| p.is_empty() || p.len() <= 4 && p.chars().all(|c| c.is_ascii_hexdigit()))
            }
            StringFormat::Uuid => {
                let clean = value.replace('-', "");
                clean.len() == 32 && clean.chars().all(|c| c.is_ascii_hexdigit())
            }
            StringFormat::Date => {
                // Basic ISO 8601 date: YYYY-MM-DD
                let parts: Vec<&str> = value.split('-').collect();
                parts.len() == 3
                    && parts[0].len() == 4
                    && parts[1].len() == 2
                    && parts[2].len() == 2
                    && parts.iter().all(|p| p.parse::<u32>().is_ok())
            }
            StringFormat::DateTime => {
                // Basic ISO 8601 datetime validation
                value.contains('T') && (value.ends_with('Z') || value.contains('+') || value.contains('-'))
            }
            StringFormat::SemVer => {
                // Semantic version: major.minor.patch
                let parts: Vec<&str> = value.split('.').collect();
                parts.len() >= 3
                    && parts[..3]
                        .iter()
                        .all(|p| p.parse::<u32>().is_ok())
            }
            StringFormat::Pattern(regex) => regex.is_match(value),
        }
    }
}

impl StringFormatRule {
    /// Create a new string format rule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        field_path: impl Into<String>,
        format: StringFormat,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            format,
        }
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
impl Rule for StringFormatRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates that string values match expected formats"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Type
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

        let full_path = if path.is_empty() {
            self.field_path.clone()
        } else {
            format!("{}.{}", path, self.field_path)
        };

        if let Some(field_value) = self.get_value_at_path(value, &self.field_path) {
            match field_value {
                ConfigValue::String(s) => {
                    if !self.format.matches(s) {
                        findings.push(
                            ValidationFinding::new(
                                &self.id,
                                RuleCategory::Type,
                                Severity::Error,
                                format!(
                                    "Invalid {} format: '{}'",
                                    self.format.name(),
                                    s
                                ),
                                &full_path,
                            )
                            .with_expected(format!("valid {}", self.format.name()))
                            .with_actual(s.clone())
                            .with_suggestion(format!(
                                "Provide a valid {} value",
                                self.format.name()
                            )),
                        );
                    }
                }
                _ => {
                    findings.push(
                        ValidationFinding::new(
                            &self.id,
                            RuleCategory::Type,
                            Severity::Error,
                            format!(
                                "Expected string for {} format, found {}",
                                self.format.name(),
                                actual_type_name(field_value)
                            ),
                            &full_path,
                        )
                        .with_expected("string")
                        .with_actual(actual_type_name(field_value)),
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

    fn make_context() -> RuleContext {
        RuleContext::new(Environment::Development, "test")
    }

    #[test]
    fn test_expected_type_matches() {
        assert!(ExpectedType::String.matches(&ConfigValue::String("test".to_string())));
        assert!(!ExpectedType::String.matches(&ConfigValue::Integer(42)));

        assert!(ExpectedType::Integer.matches(&ConfigValue::Integer(42)));
        assert!(ExpectedType::Float.matches(&ConfigValue::Float(3.14)));
        assert!(ExpectedType::Float.matches(&ConfigValue::Integer(42))); // int -> float allowed

        assert!(ExpectedType::Boolean.matches(&ConfigValue::Boolean(true)));
    }

    #[test]
    fn test_array_type_matching() {
        let arr = ConfigValue::Array(vec![
            ConfigValue::Integer(1),
            ConfigValue::Integer(2),
        ]);

        assert!(ExpectedType::Array(None).matches(&arr));
        assert!(ExpectedType::Array(Some(Box::new(ExpectedType::Integer))).matches(&arr));
        assert!(!ExpectedType::Array(Some(Box::new(ExpectedType::String))).matches(&arr));
    }

    #[test]
    fn test_union_type() {
        let union = ExpectedType::OneOf(vec![ExpectedType::String, ExpectedType::Integer]);
        assert!(union.matches(&ConfigValue::String("test".to_string())));
        assert!(union.matches(&ConfigValue::Integer(42)));
        assert!(!union.matches(&ConfigValue::Boolean(true)));
    }

    #[tokio::test]
    async fn test_type_check_rule() {
        let rule = TypeCheckRule::new("type_001", "Type Check")
            .expect_type("port", ExpectedType::Integer)
            .expect_type("host", ExpectedType::String);

        let mut obj = HashMap::new();
        obj.insert("port".to_string(), ConfigValue::Integer(8080));
        obj.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_type_check_failure() {
        let rule = TypeCheckRule::new("type_002", "Type Check")
            .expect_type("port", ExpectedType::Integer);

        let mut obj = HashMap::new();
        obj.insert("port".to_string(), ConfigValue::String("8080".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("Type mismatch"));
    }

    #[test]
    fn test_string_formats() {
        assert!(StringFormat::Email.matches("test@example.com"));
        assert!(!StringFormat::Email.matches("invalid"));

        assert!(StringFormat::Url.matches("https://example.com"));
        assert!(!StringFormat::Url.matches("not-a-url"));

        assert!(StringFormat::Ipv4.matches("192.168.1.1"));
        assert!(!StringFormat::Ipv4.matches("999.999.999.999"));

        assert!(StringFormat::Uuid.matches("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!StringFormat::Uuid.matches("not-a-uuid"));

        assert!(StringFormat::SemVer.matches("1.2.3"));
        assert!(StringFormat::SemVer.matches("10.20.30"));
        assert!(!StringFormat::SemVer.matches("1.2"));
    }

    #[tokio::test]
    async fn test_string_format_rule() {
        let rule = StringFormatRule::new(
            "fmt_001",
            "Email Format",
            "contact.email",
            StringFormat::Email,
        );

        let mut contact = HashMap::new();
        contact.insert("email".to_string(), ConfigValue::String("test@example.com".to_string()));
        let mut obj = HashMap::new();
        obj.insert("contact".to_string(), ConfigValue::Object(contact));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }
}
