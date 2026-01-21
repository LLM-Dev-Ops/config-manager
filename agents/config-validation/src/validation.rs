//! Validation logic for configuration files
//!
//! Provides schema-based validation, structural validation, and
//! environment-specific validation rules.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{Result, ValidationError};

/// Severity levels for validation findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    /// Critical error that must be fixed
    Error,
    /// Warning that should be addressed
    Warning,
    /// Informational finding
    Info,
}

impl std::fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationSeverity::Error => write!(f, "error"),
            ValidationSeverity::Warning => write!(f, "warning"),
            ValidationSeverity::Info => write!(f, "info"),
        }
    }
}

/// A single validation finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Severity of the finding
    pub severity: ValidationSeverity,
    /// Unique code for this finding type
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// JSON path to the problematic value
    pub path: String,
    /// Suggested fix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Link to documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_link: Option<String>,
}

impl ValidationFinding {
    /// Create a new error finding
    pub fn error(code: impl Into<String>, message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            code: code.into(),
            message: message.into(),
            path: path.into(),
            suggestion: None,
            doc_link: None,
        }
    }

    /// Create a new warning finding
    pub fn warning(code: impl Into<String>, message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            code: code.into(),
            message: message.into(),
            path: path.into(),
            suggestion: None,
            doc_link: None,
        }
    }

    /// Create a new info finding
    pub fn info(code: impl Into<String>, message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Info,
            code: code.into(),
            message: message.into(),
            path: path.into(),
            suggestion: None,
            doc_link: None,
        }
    }

    /// Add a suggestion to the finding
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add a documentation link to the finding
    pub fn with_doc_link(mut self, link: impl Into<String>) -> Self {
        self.doc_link = Some(link.into());
        self
    }
}

/// Result of a validation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the configuration is valid (no errors)
    pub valid: bool,
    /// List of findings
    pub findings: Vec<ValidationFinding>,
    /// Validation duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl ValidationResult {
    /// Create a new valid result
    pub fn valid() -> Self {
        Self {
            valid: true,
            findings: Vec::new(),
            duration_ms: None,
        }
    }

    /// Create a result with findings
    pub fn with_findings(findings: Vec<ValidationFinding>) -> Self {
        let valid = !findings.iter().any(|f| f.severity == ValidationSeverity::Error);
        Self {
            valid,
            findings,
            duration_ms: None,
        }
    }

    /// Add a finding
    pub fn add_finding(&mut self, finding: ValidationFinding) {
        if finding.severity == ValidationSeverity::Error {
            self.valid = false;
        }
        self.findings.push(finding);
    }

    /// Set the duration
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Get all errors
    pub fn errors(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == ValidationSeverity::Error)
            .collect()
    }

    /// Get all warnings
    pub fn warnings(&self) -> Vec<&ValidationFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == ValidationSeverity::Warning)
            .collect()
    }
}

/// Context for validation operations
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Target environment
    pub environment: String,
    /// Whether to use strict mode
    pub strict_mode: bool,
    /// Custom rules to apply
    pub custom_rules: Vec<String>,
    /// Variables for rule evaluation
    pub variables: HashMap<String, String>,
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self {
            environment: "production".to_string(),
            strict_mode: false,
            custom_rules: Vec::new(),
            variables: HashMap::new(),
        }
    }
}

impl ValidationContext {
    /// Create a new validation context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the environment
    pub fn with_environment(mut self, env: &str) -> Self {
        self.environment = env.to_string();
        self
    }

    /// Enable strict mode
    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    /// Add a custom rule
    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.custom_rules.push(rule.into());
        self
    }

    /// Add a variable
    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }
}

/// Main validator for configurations
pub struct Validator {
    context: ValidationContext,
    schema: Option<serde_json::Value>,
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Validator {
    /// Create a new validator
    pub fn new(context: ValidationContext) -> Self {
        let mut validator = Self {
            context,
            schema: None,
            rules: Vec::new(),
        };
        validator.add_builtin_rules();
        validator
    }

    /// Add built-in validation rules
    fn add_builtin_rules(&mut self) {
        self.rules.push(Box::new(TypeValidationRule));
        self.rules.push(Box::new(RequiredFieldsRule));
        self.rules.push(Box::new(SecurityRule));
        self.rules.push(Box::new(NamingConventionRule));
    }

    /// Load a schema for validation
    pub fn load_schema(&mut self, schema_content: &str) -> Result<()> {
        let schema: serde_json::Value = serde_json::from_str(schema_content)
            .map_err(|e| ValidationError::SchemaError(format!("Invalid schema: {}", e)))?;
        self.schema = Some(schema);
        Ok(())
    }

    /// Validate a configuration value
    pub fn validate(&self, config: &serde_json::Value) -> Result<ValidationResult> {
        use std::time::Instant;
        let start = Instant::now();

        let mut result = ValidationResult::valid();

        // Apply schema validation if schema is loaded
        if let Some(schema) = &self.schema {
            self.validate_against_schema(config, schema, "$", &mut result)?;
        }

        // Apply all validation rules
        for rule in &self.rules {
            rule.validate(config, &self.context, &mut result)?;
        }

        let duration = start.elapsed().as_millis() as u64;
        Ok(result.with_duration(duration))
    }

    /// Validate configuration against schema
    fn validate_against_schema(
        &self,
        config: &serde_json::Value,
        schema: &serde_json::Value,
        path: &str,
        result: &mut ValidationResult,
    ) -> Result<()> {
        // Check type
        if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
            let actual_type = get_json_type(config);
            if actual_type != expected_type && expected_type != "any" {
                result.add_finding(
                    ValidationFinding::error(
                        "E001",
                        format!("Expected type '{}' but found '{}'", expected_type, actual_type),
                        path,
                    )
                    .with_suggestion(format!("Change the value to type '{}'", expected_type)),
                );
            }
        }

        // Check required fields for objects
        if let (Some(serde_json::Value::Object(config_obj)), Some(required)) =
            (Some(config), schema.get("required").and_then(|r| r.as_array()))
        {
            if let serde_json::Value::Object(config_map) = config {
                for req in required {
                    if let Some(field) = req.as_str() {
                        if !config_map.contains_key(field) {
                            result.add_finding(
                                ValidationFinding::error(
                                    "E002",
                                    format!("Missing required field '{}'", field),
                                    path,
                                )
                                .with_suggestion(format!("Add the required field '{}'", field)),
                            );
                        }
                    }
                }
            }
        }

        // Recursively validate properties
        if let (serde_json::Value::Object(config_obj), Some(properties)) =
            (config, schema.get("properties").and_then(|p| p.as_object()))
        {
            for (key, prop_schema) in properties {
                if let Some(prop_value) = config_obj.get(key) {
                    let prop_path = format!("{}.{}", path, key);
                    self.validate_against_schema(prop_value, prop_schema, &prop_path, result)?;
                }
            }
        }

        // Validate array items
        if let (serde_json::Value::Array(items), Some(items_schema)) =
            (config, schema.get("items"))
        {
            for (i, item) in items.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                self.validate_against_schema(item, items_schema, &item_path, result)?;
            }
        }

        // Check constraints
        self.validate_constraints(config, schema, path, result)?;

        Ok(())
    }

    /// Validate value constraints
    fn validate_constraints(
        &self,
        value: &serde_json::Value,
        schema: &serde_json::Value,
        path: &str,
        result: &mut ValidationResult,
    ) -> Result<()> {
        // String constraints
        if let serde_json::Value::String(s) = value {
            if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
                if (s.len() as u64) < min_len {
                    result.add_finding(ValidationFinding::error(
                        "E003",
                        format!("String length {} is less than minimum {}", s.len(), min_len),
                        path,
                    ));
                }
            }
            if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
                if (s.len() as u64) > max_len {
                    result.add_finding(ValidationFinding::error(
                        "E004",
                        format!("String length {} exceeds maximum {}", s.len(), max_len),
                        path,
                    ));
                }
            }
            if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if !re.is_match(s) {
                        result.add_finding(ValidationFinding::error(
                            "E005",
                            format!("String does not match pattern '{}'", pattern),
                            path,
                        ));
                    }
                }
            }
        }

        // Number constraints
        if let Some(num) = value.as_f64() {
            if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
                if num < min {
                    result.add_finding(ValidationFinding::error(
                        "E006",
                        format!("Value {} is less than minimum {}", num, min),
                        path,
                    ));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
                if num > max {
                    result.add_finding(ValidationFinding::error(
                        "E007",
                        format!("Value {} exceeds maximum {}", num, max),
                        path,
                    ));
                }
            }
        }

        // Enum constraints
        if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
            if !enum_values.contains(value) {
                let allowed: Vec<String> = enum_values
                    .iter()
                    .map(|v| v.to_string())
                    .collect();
                result.add_finding(
                    ValidationFinding::error(
                        "E008",
                        format!("Value not in allowed enum values"),
                        path,
                    )
                    .with_suggestion(format!("Allowed values: {}", allowed.join(", "))),
                );
            }
        }

        Ok(())
    }
}

/// Get the JSON type name
fn get_json_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Trait for validation rules
pub trait ValidationRule: Send + Sync {
    /// Apply this rule to a configuration
    fn validate(
        &self,
        config: &serde_json::Value,
        context: &ValidationContext,
        result: &mut ValidationResult,
    ) -> Result<()>;

    /// Get the rule name
    fn name(&self) -> &'static str;
}

/// Type validation rule
struct TypeValidationRule;

impl ValidationRule for TypeValidationRule {
    fn validate(
        &self,
        config: &serde_json::Value,
        _context: &ValidationContext,
        result: &mut ValidationResult,
    ) -> Result<()> {
        // Check for null values in unexpected places
        self.check_nulls(config, "$", result);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "type_validation"
    }
}

impl TypeValidationRule {
    fn check_nulls(&self, value: &serde_json::Value, path: &str, result: &mut ValidationResult) {
        match value {
            serde_json::Value::Null => {
                result.add_finding(
                    ValidationFinding::warning(
                        "W001",
                        "Null value found - consider using explicit default",
                        path,
                    )
                    .with_suggestion("Replace null with an explicit default value"),
                );
            }
            serde_json::Value::Object(obj) => {
                for (key, val) in obj {
                    self.check_nulls(val, &format!("{}.{}", path, key), result);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    self.check_nulls(val, &format!("{}[{}]", path, i), result);
                }
            }
            _ => {}
        }
    }
}

/// Required fields validation rule
struct RequiredFieldsRule;

impl ValidationRule for RequiredFieldsRule {
    fn validate(
        &self,
        config: &serde_json::Value,
        context: &ValidationContext,
        result: &mut ValidationResult,
    ) -> Result<()> {
        // Check for common required fields based on environment
        if context.environment == "production" {
            if let serde_json::Value::Object(obj) = config {
                // Check for logging configuration in production
                if !obj.contains_key("logging") && !obj.contains_key("log") {
                    result.add_finding(
                        ValidationFinding::warning(
                            "W002",
                            "Missing logging configuration for production",
                            "$",
                        )
                        .with_suggestion("Add a 'logging' section with appropriate settings"),
                    );
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "required_fields"
    }
}

/// Security validation rule
struct SecurityRule;

impl ValidationRule for SecurityRule {
    fn validate(
        &self,
        config: &serde_json::Value,
        _context: &ValidationContext,
        result: &mut ValidationResult,
    ) -> Result<()> {
        self.check_security(config, "$", result);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "security"
    }
}

impl SecurityRule {
    fn check_security(&self, value: &serde_json::Value, path: &str, result: &mut ValidationResult) {
        match value {
            serde_json::Value::String(s) => {
                // Check for potential secrets in plain text
                let lower = s.to_lowercase();
                let key_lower = path.to_lowercase();

                let is_secret_key = key_lower.contains("password")
                    || key_lower.contains("secret")
                    || key_lower.contains("api_key")
                    || key_lower.contains("apikey")
                    || key_lower.contains("token")
                    || key_lower.contains("private");

                if is_secret_key && !s.starts_with("${") && !s.starts_with("enc:") && s.len() > 0 {
                    result.add_finding(
                        ValidationFinding::error(
                            "S001",
                            "Potential secret in plain text",
                            path,
                        )
                        .with_suggestion("Use environment variables (${VAR}) or encrypted values (enc:...)"),
                    );
                }

                // Check for localhost/development URLs in production-looking configs
                if (lower.contains("localhost") || lower.contains("127.0.0.1"))
                    && (key_lower.contains("url") || key_lower.contains("endpoint") || key_lower.contains("host"))
                {
                    result.add_finding(
                        ValidationFinding::warning(
                            "S002",
                            "Localhost/development URL found",
                            path,
                        )
                        .with_suggestion("Ensure this is intentional for your environment"),
                    );
                }
            }
            serde_json::Value::Object(obj) => {
                for (key, val) in obj {
                    self.check_security(val, &format!("{}.{}", path, key), result);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    self.check_security(val, &format!("{}[{}]", path, i), result);
                }
            }
            _ => {}
        }
    }
}

/// Naming convention validation rule
struct NamingConventionRule;

impl ValidationRule for NamingConventionRule {
    fn validate(
        &self,
        config: &serde_json::Value,
        _context: &ValidationContext,
        result: &mut ValidationResult,
    ) -> Result<()> {
        if let serde_json::Value::Object(obj) = config {
            self.check_naming(obj, "$", result);
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "naming_convention"
    }
}

impl NamingConventionRule {
    fn check_naming(
        &self,
        obj: &serde_json::Map<String, serde_json::Value>,
        path: &str,
        result: &mut ValidationResult,
    ) {
        for (key, value) in obj {
            // Check for inconsistent naming (mixing camelCase and snake_case)
            let has_underscore = key.contains('_');
            let has_camel = key.chars().any(|c| c.is_uppercase());

            if has_underscore && has_camel {
                result.add_finding(
                    ValidationFinding::info(
                        "I001",
                        format!("Mixed naming convention in key '{}'", key),
                        path,
                    )
                    .with_suggestion("Use consistent naming: either camelCase or snake_case"),
                );
            }

            // Recursively check nested objects
            if let serde_json::Value::Object(nested) = value {
                self.check_naming(nested, &format!("{}.{}", path, key), result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_severity_display() {
        assert_eq!(ValidationSeverity::Error.to_string(), "error");
        assert_eq!(ValidationSeverity::Warning.to_string(), "warning");
        assert_eq!(ValidationSeverity::Info.to_string(), "info");
    }

    #[test]
    fn test_validation_finding_builders() {
        let finding = ValidationFinding::error("E001", "Test error", "$.path")
            .with_suggestion("Fix it")
            .with_doc_link("http://docs.example.com");

        assert_eq!(finding.severity, ValidationSeverity::Error);
        assert_eq!(finding.code, "E001");
        assert_eq!(finding.suggestion, Some("Fix it".to_string()));
        assert_eq!(finding.doc_link, Some("http://docs.example.com".to_string()));
    }

    #[test]
    fn test_validation_result_valid() {
        let result = ValidationResult::valid();
        assert!(result.valid);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn test_validation_result_with_findings() {
        let findings = vec![
            ValidationFinding::warning("W001", "Warning", "$.path"),
            ValidationFinding::error("E001", "Error", "$.path"),
        ];
        let result = ValidationResult::with_findings(findings);
        assert!(!result.valid); // Has error, so not valid
        assert_eq!(result.findings.len(), 2);
    }

    #[test]
    fn test_validation_context_builder() {
        let context = ValidationContext::new()
            .with_environment("staging")
            .with_strict_mode(true)
            .with_variable("key", "value");

        assert_eq!(context.environment, "staging");
        assert!(context.strict_mode);
        assert_eq!(context.variables.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_validator_basic() {
        let context = ValidationContext::new();
        let validator = Validator::new(context);

        let config: serde_json::Value = serde_json::json!({
            "name": "test",
            "value": 42
        });

        let result = validator.validate(&config).unwrap();
        assert!(result.valid);
    }

    #[test]
    fn test_security_rule_detects_plain_password() {
        let context = ValidationContext::new();
        let validator = Validator::new(context);

        let config: serde_json::Value = serde_json::json!({
            "database": {
                "password": "mysecretpassword"
            }
        });

        let result = validator.validate(&config).unwrap();
        assert!(!result.valid);
        assert!(result.findings.iter().any(|f| f.code == "S001"));
    }
}
