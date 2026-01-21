//! Schema inference and inspection for configuration files
//!
//! Provides functionality to analyze configuration structures,
//! infer types, and detect patterns.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::error::{Result, ValidationError};

/// Inferred schema from a configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredSchema {
    /// Root type information
    pub root: TypeInfo,
    /// Detected patterns in the configuration
    pub patterns: Vec<String>,
    /// Inferred constraints
    pub constraints: Vec<String>,
    /// Total number of fields
    pub field_count: usize,
    /// Maximum nesting depth
    pub max_depth: usize,
    /// Number of arrays
    pub array_count: usize,
    /// Number of objects
    pub object_count: usize,
}

/// Type information for a configuration node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Name of this field/node
    pub name: String,
    /// Type name
    pub type_name: TypeName,
    /// Whether this field is required (always present in samples)
    pub required: bool,
    /// Whether this field can be null
    pub nullable: bool,
    /// Child nodes (for objects and arrays)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TypeInfo>,
    /// Example value (for scalar types)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Detected format (for strings: email, url, datetime, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Type names for configuration values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TypeName {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
    Null,
    /// Multiple types detected
    Mixed(Vec<String>),
}

impl std::fmt::Display for TypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeName::String => write!(f, "string"),
            TypeName::Integer => write!(f, "integer"),
            TypeName::Float => write!(f, "float"),
            TypeName::Boolean => write!(f, "boolean"),
            TypeName::Array => write!(f, "array"),
            TypeName::Object => write!(f, "object"),
            TypeName::Null => write!(f, "null"),
            TypeName::Mixed(types) => write!(f, "mixed({})", types.join("|")),
        }
    }
}

/// Schema inference engine
pub struct SchemaInference {
    /// String format detectors
    format_detectors: Vec<Box<dyn FormatDetector>>,
}

impl Default for SchemaInference {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaInference {
    /// Create a new schema inference engine
    pub fn new() -> Self {
        let mut inference = Self {
            format_detectors: Vec::new(),
        };
        inference.add_builtin_detectors();
        inference
    }

    /// Add built-in format detectors
    fn add_builtin_detectors(&mut self) {
        self.format_detectors.push(Box::new(EmailFormatDetector));
        self.format_detectors.push(Box::new(UrlFormatDetector));
        self.format_detectors.push(Box::new(DateTimeFormatDetector));
        self.format_detectors.push(Box::new(UuidFormatDetector));
        self.format_detectors.push(Box::new(IpAddressFormatDetector));
    }

    /// Infer schema from a configuration value
    pub fn infer(&self, value: &serde_json::Value) -> Result<InferredSchema> {
        let mut stats = InferenceStats::default();
        let root = self.infer_type(value, "root", 0, &mut stats);

        let patterns = self.detect_patterns(value);
        let constraints = self.infer_constraints(value);

        Ok(InferredSchema {
            root,
            patterns,
            constraints,
            field_count: stats.field_count,
            max_depth: stats.max_depth,
            array_count: stats.array_count,
            object_count: stats.object_count,
        })
    }

    /// Infer type information for a value
    fn infer_type(
        &self,
        value: &serde_json::Value,
        name: &str,
        depth: usize,
        stats: &mut InferenceStats,
    ) -> TypeInfo {
        stats.max_depth = stats.max_depth.max(depth);

        match value {
            serde_json::Value::Null => TypeInfo {
                name: name.to_string(),
                type_name: TypeName::Null,
                required: true,
                nullable: true,
                children: Vec::new(),
                example: Some("null".to_string()),
                format: None,
            },
            serde_json::Value::Bool(b) => {
                stats.field_count += 1;
                TypeInfo {
                    name: name.to_string(),
                    type_name: TypeName::Boolean,
                    required: true,
                    nullable: false,
                    children: Vec::new(),
                    example: Some(b.to_string()),
                    format: None,
                }
            }
            serde_json::Value::Number(n) => {
                stats.field_count += 1;
                let (type_name, example) = if n.is_i64() {
                    (TypeName::Integer, n.as_i64().unwrap().to_string())
                } else {
                    (TypeName::Float, n.as_f64().unwrap().to_string())
                };
                TypeInfo {
                    name: name.to_string(),
                    type_name,
                    required: true,
                    nullable: false,
                    children: Vec::new(),
                    example: Some(example),
                    format: None,
                }
            }
            serde_json::Value::String(s) => {
                stats.field_count += 1;
                let format = self.detect_string_format(s);
                TypeInfo {
                    name: name.to_string(),
                    type_name: TypeName::String,
                    required: true,
                    nullable: false,
                    children: Vec::new(),
                    example: Some(truncate_example(s, 50)),
                    format,
                }
            }
            serde_json::Value::Array(arr) => {
                stats.array_count += 1;
                let children = if let Some(first) = arr.first() {
                    vec![self.infer_type(first, "items", depth + 1, stats)]
                } else {
                    Vec::new()
                };
                TypeInfo {
                    name: name.to_string(),
                    type_name: TypeName::Array,
                    required: true,
                    nullable: false,
                    children,
                    example: Some(format!("[{} items]", arr.len())),
                    format: None,
                }
            }
            serde_json::Value::Object(obj) => {
                stats.object_count += 1;
                let children: Vec<TypeInfo> = obj
                    .iter()
                    .map(|(key, val)| self.infer_type(val, key, depth + 1, stats))
                    .collect();
                TypeInfo {
                    name: name.to_string(),
                    type_name: TypeName::Object,
                    required: true,
                    nullable: false,
                    children,
                    example: Some(format!("{{{} fields}}", obj.len())),
                    format: None,
                }
            }
        }
    }

    /// Detect string format
    fn detect_string_format(&self, value: &str) -> Option<String> {
        for detector in &self.format_detectors {
            if let Some(format) = detector.detect(value) {
                return Some(format);
            }
        }
        None
    }

    /// Detect common patterns in the configuration
    fn detect_patterns(&self, value: &serde_json::Value) -> Vec<String> {
        let mut patterns = Vec::new();

        if let serde_json::Value::Object(obj) = value {
            // Check for environment variables pattern
            if self.has_env_vars(value) {
                patterns.push("Environment variable references (${VAR})".to_string());
            }

            // Check for nested environments pattern
            if obj.contains_key("development") || obj.contains_key("staging") || obj.contains_key("production") {
                patterns.push("Multi-environment configuration".to_string());
            }

            // Check for feature flags pattern
            if obj.contains_key("features") || obj.contains_key("feature_flags") || obj.contains_key("featureFlags") {
                patterns.push("Feature flags".to_string());
            }

            // Check for secrets pattern
            if obj.contains_key("secrets") || obj.contains_key("credentials") {
                patterns.push("Secrets/credentials section".to_string());
            }

            // Check for database configuration pattern
            if obj.contains_key("database") || obj.contains_key("db") {
                patterns.push("Database configuration".to_string());
            }

            // Check for service mesh/microservices pattern
            if obj.contains_key("services") || obj.contains_key("endpoints") {
                patterns.push("Service/endpoint definitions".to_string());
            }
        }

        patterns
    }

    /// Check if configuration contains environment variable references
    fn has_env_vars(&self, value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(s) => s.contains("${") && s.contains("}"),
            serde_json::Value::Object(obj) => obj.values().any(|v| self.has_env_vars(v)),
            serde_json::Value::Array(arr) => arr.iter().any(|v| self.has_env_vars(v)),
            _ => false,
        }
    }

    /// Infer constraints from the configuration
    fn infer_constraints(&self, value: &serde_json::Value) -> Vec<String> {
        let mut constraints = Vec::new();
        let mut string_lengths: Vec<usize> = Vec::new();
        let mut number_values: Vec<f64> = Vec::new();

        self.collect_stats(value, &mut string_lengths, &mut number_values);

        // Infer string length constraints
        if !string_lengths.is_empty() {
            let min = *string_lengths.iter().min().unwrap();
            let max = *string_lengths.iter().max().unwrap();
            if min == max && string_lengths.len() > 1 {
                constraints.push(format!("String lengths appear fixed at {} characters", min));
            } else if max > 100 {
                constraints.push(format!("Some strings are quite long (up to {} chars)", max));
            }
        }

        // Infer number range constraints
        if !number_values.is_empty() {
            let min = number_values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = number_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if min >= 0.0 && number_values.iter().all(|n| *n >= 0.0) {
                constraints.push("All numbers appear to be non-negative".to_string());
            }
            if number_values.iter().all(|n| n.fract() == 0.0) {
                constraints.push("All numbers appear to be integers".to_string());
            }
        }

        constraints
    }

    /// Collect statistics for constraint inference
    fn collect_stats(
        &self,
        value: &serde_json::Value,
        string_lengths: &mut Vec<usize>,
        number_values: &mut Vec<f64>,
    ) {
        match value {
            serde_json::Value::String(s) => {
                string_lengths.push(s.len());
            }
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    number_values.push(f);
                }
            }
            serde_json::Value::Object(obj) => {
                for v in obj.values() {
                    self.collect_stats(v, string_lengths, number_values);
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    self.collect_stats(v, string_lengths, number_values);
                }
            }
            _ => {}
        }
    }
}

/// Statistics collected during inference
#[derive(Default)]
struct InferenceStats {
    field_count: usize,
    max_depth: usize,
    array_count: usize,
    object_count: usize,
}

/// Trait for string format detection
trait FormatDetector: Send + Sync {
    fn detect(&self, value: &str) -> Option<String>;
}

/// Email format detector
struct EmailFormatDetector;

impl FormatDetector for EmailFormatDetector {
    fn detect(&self, value: &str) -> Option<String> {
        // Simple email pattern check
        if value.contains('@') && value.contains('.') {
            let parts: Vec<&str> = value.split('@').collect();
            if parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') {
                return Some("email".to_string());
            }
        }
        None
    }
}

/// URL format detector
struct UrlFormatDetector;

impl FormatDetector for UrlFormatDetector {
    fn detect(&self, value: &str) -> Option<String> {
        if value.starts_with("http://") || value.starts_with("https://") || value.starts_with("ftp://") {
            return Some("uri".to_string());
        }
        None
    }
}

/// DateTime format detector
struct DateTimeFormatDetector;

impl FormatDetector for DateTimeFormatDetector {
    fn detect(&self, value: &str) -> Option<String> {
        // ISO 8601 date-time pattern
        if value.len() >= 10 {
            let chars: Vec<char> = value.chars().collect();
            if chars.len() >= 10
                && chars[4] == '-'
                && chars[7] == '-'
                && chars[0..4].iter().all(|c| c.is_ascii_digit())
                && chars[5..7].iter().all(|c| c.is_ascii_digit())
                && chars[8..10].iter().all(|c| c.is_ascii_digit())
            {
                if value.len() == 10 {
                    return Some("date".to_string());
                }
                if value.contains('T') || value.contains(' ') {
                    return Some("date-time".to_string());
                }
            }
        }
        None
    }
}

/// UUID format detector
struct UuidFormatDetector;

impl FormatDetector for UuidFormatDetector {
    fn detect(&self, value: &str) -> Option<String> {
        // UUID pattern: 8-4-4-4-12
        if value.len() == 36 {
            let parts: Vec<&str> = value.split('-').collect();
            if parts.len() == 5
                && parts[0].len() == 8
                && parts[1].len() == 4
                && parts[2].len() == 4
                && parts[3].len() == 4
                && parts[4].len() == 12
                && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Some("uuid".to_string());
            }
        }
        None
    }
}

/// IP address format detector
struct IpAddressFormatDetector;

impl FormatDetector for IpAddressFormatDetector {
    fn detect(&self, value: &str) -> Option<String> {
        // IPv4 pattern
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() == 4 {
            if parts.iter().all(|p| p.parse::<u8>().is_ok()) {
                return Some("ipv4".to_string());
            }
        }
        // IPv6 pattern (simplified)
        if value.contains(':') && !value.contains('@') && !value.starts_with("http") {
            let parts: Vec<&str> = value.split(':').collect();
            if parts.len() >= 3 && parts.iter().all(|p| p.is_empty() || p.chars().all(|c| c.is_ascii_hexdigit())) {
                return Some("ipv6".to_string());
            }
        }
        None
    }
}

/// Truncate a string for display as an example
fn truncate_example(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_name_display() {
        assert_eq!(TypeName::String.to_string(), "string");
        assert_eq!(TypeName::Integer.to_string(), "integer");
        assert_eq!(TypeName::Object.to_string(), "object");
        assert_eq!(
            TypeName::Mixed(vec!["string".to_string(), "number".to_string()]).to_string(),
            "mixed(string|number)"
        );
    }

    #[test]
    fn test_schema_inference_basic() {
        let inference = SchemaInference::new();
        let value: serde_json::Value = serde_json::json!({
            "name": "test",
            "count": 42,
            "enabled": true
        });

        let schema = inference.infer(&value).unwrap();
        assert_eq!(schema.root.type_name, TypeName::Object);
        assert_eq!(schema.root.children.len(), 3);
        assert_eq!(schema.field_count, 3);
    }

    #[test]
    fn test_schema_inference_nested() {
        let inference = SchemaInference::new();
        let value: serde_json::Value = serde_json::json!({
            "outer": {
                "inner": {
                    "value": 1
                }
            }
        });

        let schema = inference.infer(&value).unwrap();
        assert_eq!(schema.max_depth, 3);
        assert_eq!(schema.object_count, 3); // root + outer + inner
    }

    #[test]
    fn test_format_detection_email() {
        let detector = EmailFormatDetector;
        assert_eq!(detector.detect("test@example.com"), Some("email".to_string()));
        assert_eq!(detector.detect("not-an-email"), None);
    }

    #[test]
    fn test_format_detection_url() {
        let detector = UrlFormatDetector;
        assert_eq!(detector.detect("https://example.com"), Some("uri".to_string()));
        assert_eq!(detector.detect("not-a-url"), None);
    }

    #[test]
    fn test_format_detection_uuid() {
        let detector = UuidFormatDetector;
        assert_eq!(
            detector.detect("550e8400-e29b-41d4-a716-446655440000"),
            Some("uuid".to_string())
        );
        assert_eq!(detector.detect("not-a-uuid"), None);
    }

    #[test]
    fn test_pattern_detection() {
        let inference = SchemaInference::new();
        let value: serde_json::Value = serde_json::json!({
            "database": {
                "host": "${DB_HOST}",
                "port": 5432
            },
            "features": {
                "new_feature": true
            }
        });

        let schema = inference.infer(&value).unwrap();
        assert!(schema.patterns.contains(&"Environment variable references (${VAR})".to_string()));
        assert!(schema.patterns.contains(&"Feature flags".to_string()));
        assert!(schema.patterns.contains(&"Database configuration".to_string()));
    }

    #[test]
    fn test_truncate_example() {
        assert_eq!(truncate_example("short", 50), "short");
        assert_eq!(
            truncate_example("this is a very long string that needs truncation", 20),
            "this is a very lo..."
        );
    }
}
