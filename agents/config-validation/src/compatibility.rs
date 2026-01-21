//! Cross-agent configuration compatibility checking
//!
//! Provides functionality to validate that multiple configuration files
//! are compatible with each other.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::error::{Result, ValidationError};

/// Result of a compatibility check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityResult {
    /// Whether the configurations are compatible
    pub is_compatible: bool,
    /// List of conflicts found
    pub conflicts: Vec<Conflict>,
    /// Warnings about potential issues
    pub warnings: Vec<String>,
    /// Suggestions for resolving issues
    pub suggestions: Vec<String>,
    /// Shared keys across all configurations
    pub shared_keys: Vec<String>,
    /// Keys unique to specific configurations
    pub unique_keys: HashMap<String, Vec<String>>,
}

impl CompatibilityResult {
    /// Create a new compatible result
    pub fn compatible() -> Self {
        Self {
            is_compatible: true,
            conflicts: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
            shared_keys: Vec::new(),
            unique_keys: HashMap::new(),
        }
    }

    /// Add a conflict
    pub fn add_conflict(&mut self, conflict: Conflict) {
        self.is_compatible = false;
        self.conflicts.push(conflict);
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Add a suggestion
    pub fn add_suggestion(&mut self, suggestion: impl Into<String>) {
        self.suggestions.push(suggestion.into());
    }
}

/// A conflict between configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// Description of the conflict
    pub description: String,
    /// Path in the configuration where the conflict occurs
    pub path: String,
    /// Value in the first configuration
    pub value1: serde_json::Value,
    /// Value in the second configuration
    pub value2: serde_json::Value,
    /// Names/paths of the conflicting files
    pub files: (String, String),
    /// Severity of the conflict
    pub severity: ConflictSeverity,
}

impl Conflict {
    /// Create a new conflict
    pub fn new(
        description: impl Into<String>,
        path: impl Into<String>,
        value1: serde_json::Value,
        value2: serde_json::Value,
        file1: impl Into<String>,
        file2: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            path: path.into(),
            value1,
            value2,
            files: (file1.into(), file2.into()),
            severity: ConflictSeverity::Error,
        }
    }

    /// Set the severity
    pub fn with_severity(mut self, severity: ConflictSeverity) -> Self {
        self.severity = severity;
        self
    }
}

/// Severity of a compatibility conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictSeverity {
    /// Critical conflict that must be resolved
    Error,
    /// Warning that should be reviewed
    Warning,
    /// Informational difference
    Info,
}

/// Compatibility checker for configurations
pub struct CompatibilityChecker {
    /// Rules for checking compatibility
    rules: Vec<Box<dyn CompatibilityRule>>,
}

impl Default for CompatibilityChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl CompatibilityChecker {
    /// Create a new compatibility checker
    pub fn new() -> Self {
        let mut checker = Self { rules: Vec::new() };
        checker.add_builtin_rules();
        checker
    }

    /// Add built-in compatibility rules
    fn add_builtin_rules(&mut self) {
        self.rules.push(Box::new(TypeCompatibilityRule));
        self.rules.push(Box::new(ValueConflictRule));
        self.rules.push(Box::new(SchemaCompatibilityRule));
        self.rules.push(Box::new(VersionCompatibilityRule));
    }

    /// Check compatibility between configurations
    pub fn check(
        &self,
        configs: &[(PathBuf, serde_json::Value)],
    ) -> Result<CompatibilityResult> {
        let mut result = CompatibilityResult::compatible();

        if configs.len() < 2 {
            return Err(ValidationError::InvalidInput(
                "At least 2 configurations required for compatibility check".to_string(),
            ));
        }

        // Calculate shared and unique keys
        let (shared, unique) = self.analyze_keys(configs);
        result.shared_keys = shared;
        result.unique_keys = unique;

        // Apply all compatibility rules
        for i in 0..configs.len() {
            for j in (i + 1)..configs.len() {
                let (path1, config1) = &configs[i];
                let (path2, config2) = &configs[j];

                for rule in &self.rules {
                    rule.check(
                        config1,
                        config2,
                        &path1.display().to_string(),
                        &path2.display().to_string(),
                        &mut result,
                    )?;
                }
            }
        }

        // Generate suggestions based on findings
        self.generate_suggestions(&mut result);

        Ok(result)
    }

    /// Analyze shared and unique keys across configurations
    fn analyze_keys(
        &self,
        configs: &[(PathBuf, serde_json::Value)],
    ) -> (Vec<String>, HashMap<String, Vec<String>>) {
        let mut all_keys: Vec<HashSet<String>> = Vec::new();

        for (_, config) in configs {
            let mut keys = HashSet::new();
            self.collect_keys(config, "$", &mut keys);
            all_keys.push(keys);
        }

        // Find shared keys (present in all configs)
        let shared: HashSet<String> = if let Some(first) = all_keys.first() {
            all_keys
                .iter()
                .skip(1)
                .fold(first.clone(), |acc, set| acc.intersection(set).cloned().collect())
        } else {
            HashSet::new()
        };

        // Find unique keys per configuration
        let mut unique: HashMap<String, Vec<String>> = HashMap::new();
        for (i, (path, _)) in configs.iter().enumerate() {
            let file_name = path.display().to_string();
            let unique_to_this: Vec<String> = all_keys[i]
                .iter()
                .filter(|k| !shared.contains(*k))
                .cloned()
                .collect();
            if !unique_to_this.is_empty() {
                unique.insert(file_name, unique_to_this);
            }
        }

        let mut shared_vec: Vec<String> = shared.into_iter().collect();
        shared_vec.sort();

        (shared_vec, unique)
    }

    /// Recursively collect all keys in a configuration
    fn collect_keys(&self, value: &serde_json::Value, path: &str, keys: &mut HashSet<String>) {
        keys.insert(path.to_string());

        match value {
            serde_json::Value::Object(obj) => {
                for (key, val) in obj {
                    let new_path = format!("{}.{}", path, key);
                    self.collect_keys(val, &new_path, keys);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, i);
                    self.collect_keys(val, &new_path, keys);
                }
            }
            _ => {}
        }
    }

    /// Generate suggestions based on findings
    fn generate_suggestions(&self, result: &mut CompatibilityResult) {
        if !result.conflicts.is_empty() {
            result.add_suggestion(
                "Review conflicting values and decide on a canonical source of truth".to_string(),
            );
        }

        if !result.unique_keys.is_empty() {
            let total_unique: usize = result.unique_keys.values().map(|v| v.len()).sum();
            if total_unique > 5 {
                result.add_suggestion(
                    "Consider consolidating configuration schemas to reduce divergence".to_string(),
                );
            }
        }

        // Check for potential environment-specific overrides
        // Collect suggestions first to avoid borrow issues
        let url_suggestions: Vec<String> = result.conflicts
            .iter()
            .filter(|conflict| {
                conflict.path.contains("url")
                    || conflict.path.contains("host")
                    || conflict.path.contains("endpoint")
            })
            .map(|conflict| {
                format!(
                    "URL/endpoint differences at '{}' may be intentional for different environments",
                    conflict.path
                )
            })
            .collect();

        for suggestion in url_suggestions {
            result.add_suggestion(suggestion);
        }
    }
}

/// Trait for compatibility rules
trait CompatibilityRule: Send + Sync {
    /// Check compatibility between two configurations
    fn check(
        &self,
        config1: &serde_json::Value,
        config2: &serde_json::Value,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) -> Result<()>;
}

/// Type compatibility rule
struct TypeCompatibilityRule;

impl CompatibilityRule for TypeCompatibilityRule {
    fn check(
        &self,
        config1: &serde_json::Value,
        config2: &serde_json::Value,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) -> Result<()> {
        self.check_types(config1, config2, "$", file1, file2, result);
        Ok(())
    }
}

impl TypeCompatibilityRule {
    fn check_types(
        &self,
        val1: &serde_json::Value,
        val2: &serde_json::Value,
        path: &str,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) {
        // Skip if either is null (nullable compatibility)
        if val1.is_null() || val2.is_null() {
            return;
        }

        let type1 = get_type_name(val1);
        let type2 = get_type_name(val2);

        if type1 != type2 {
            result.add_conflict(
                Conflict::new(
                    format!("Type mismatch: {} vs {}", type1, type2),
                    path,
                    val1.clone(),
                    val2.clone(),
                    file1,
                    file2,
                )
                .with_severity(ConflictSeverity::Error),
            );
            return;
        }

        // Recursively check objects
        if let (serde_json::Value::Object(obj1), serde_json::Value::Object(obj2)) = (val1, val2) {
            // Check keys present in both
            for key in obj1.keys() {
                if let (Some(v1), Some(v2)) = (obj1.get(key), obj2.get(key)) {
                    self.check_types(v1, v2, &format!("{}.{}", path, key), file1, file2, result);
                }
            }
        }

        // Recursively check arrays (using first element as representative)
        if let (serde_json::Value::Array(arr1), serde_json::Value::Array(arr2)) = (val1, val2) {
            if let (Some(first1), Some(first2)) = (arr1.first(), arr2.first()) {
                self.check_types(first1, first2, &format!("{}[0]", path), file1, file2, result);
            }
        }
    }
}

/// Value conflict detection rule
struct ValueConflictRule;

impl CompatibilityRule for ValueConflictRule {
    fn check(
        &self,
        config1: &serde_json::Value,
        config2: &serde_json::Value,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) -> Result<()> {
        self.check_values(config1, config2, "$", file1, file2, result);
        Ok(())
    }
}

impl ValueConflictRule {
    fn check_values(
        &self,
        val1: &serde_json::Value,
        val2: &serde_json::Value,
        path: &str,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) {
        match (val1, val2) {
            (serde_json::Value::Object(obj1), serde_json::Value::Object(obj2)) => {
                for key in obj1.keys() {
                    if let (Some(v1), Some(v2)) = (obj1.get(key), obj2.get(key)) {
                        self.check_values(v1, v2, &format!("{}.{}", path, key), file1, file2, result);
                    }
                }
            }
            (serde_json::Value::Array(_), serde_json::Value::Array(_)) => {
                // Arrays with different lengths might be intentional
                // Just note it as info
            }
            _ => {
                // Check for value conflicts in scalar values
                if val1 != val2 && !val1.is_null() && !val2.is_null() {
                    // Determine severity based on key name
                    let key_lower = path.to_lowercase();
                    let severity = if key_lower.contains("version") {
                        ConflictSeverity::Warning
                    } else if key_lower.contains("url") || key_lower.contains("host") || key_lower.contains("port") {
                        ConflictSeverity::Warning // Could be environment-specific
                    } else {
                        ConflictSeverity::Info
                    };

                    if severity == ConflictSeverity::Warning || severity == ConflictSeverity::Error {
                        result.add_conflict(
                            Conflict::new(
                                format!("Value mismatch"),
                                path,
                                val1.clone(),
                                val2.clone(),
                                file1,
                                file2,
                            )
                            .with_severity(severity),
                        );
                    }
                }
            }
        }
    }
}

/// Schema compatibility rule (for configurations that include schema info)
struct SchemaCompatibilityRule;

impl CompatibilityRule for SchemaCompatibilityRule {
    fn check(
        &self,
        config1: &serde_json::Value,
        config2: &serde_json::Value,
        _file1: &str,
        _file2: &str,
        result: &mut CompatibilityResult,
    ) -> Result<()> {
        // Check for $schema field mismatch
        if let (Some(schema1), Some(schema2)) = (
            config1.get("$schema").and_then(|v| v.as_str()),
            config2.get("$schema").and_then(|v| v.as_str()),
        ) {
            if schema1 != schema2 {
                result.add_warning(format!(
                    "Different schema references: '{}' vs '{}'",
                    schema1, schema2
                ));
            }
        }

        Ok(())
    }
}

/// Version compatibility rule
struct VersionCompatibilityRule;

impl CompatibilityRule for VersionCompatibilityRule {
    fn check(
        &self,
        config1: &serde_json::Value,
        config2: &serde_json::Value,
        file1: &str,
        file2: &str,
        result: &mut CompatibilityResult,
    ) -> Result<()> {
        // Check for version field mismatch
        let version1 = config1
            .get("version")
            .or_else(|| config1.get("apiVersion"))
            .or_else(|| config1.get("configVersion"));
        let version2 = config2
            .get("version")
            .or_else(|| config2.get("apiVersion"))
            .or_else(|| config2.get("configVersion"));

        if let (Some(v1), Some(v2)) = (version1, version2) {
            if v1 != v2 {
                result.add_warning(format!(
                    "Version mismatch: {} ({}) vs {} ({})",
                    v1, file1, v2, file2
                ));
            }
        }

        Ok(())
    }
}

/// Get the type name for a JSON value
fn get_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatibility_result_default() {
        let result = CompatibilityResult::compatible();
        assert!(result.is_compatible);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_add_conflict() {
        let mut result = CompatibilityResult::compatible();
        result.add_conflict(Conflict::new(
            "Test conflict",
            "$.path",
            serde_json::json!(1),
            serde_json::json!(2),
            "file1.json",
            "file2.json",
        ));
        assert!(!result.is_compatible);
        assert_eq!(result.conflicts.len(), 1);
    }

    #[test]
    fn test_checker_compatible_configs() {
        let checker = CompatibilityChecker::new();
        let configs = vec![
            (
                PathBuf::from("config1.json"),
                serde_json::json!({
                    "name": "test",
                    "version": "1.0.0"
                }),
            ),
            (
                PathBuf::from("config2.json"),
                serde_json::json!({
                    "name": "test",
                    "version": "1.0.0"
                }),
            ),
        ];

        let result = checker.check(&configs).unwrap();
        assert!(result.is_compatible);
    }

    #[test]
    fn test_checker_type_mismatch() {
        let checker = CompatibilityChecker::new();
        let configs = vec![
            (
                PathBuf::from("config1.json"),
                serde_json::json!({
                    "port": 8080
                }),
            ),
            (
                PathBuf::from("config2.json"),
                serde_json::json!({
                    "port": "8080"
                }),
            ),
        ];

        let result = checker.check(&configs).unwrap();
        assert!(!result.is_compatible);
        assert!(result.conflicts.iter().any(|c| c.description.contains("Type mismatch")));
    }

    #[test]
    fn test_checker_requires_two_configs() {
        let checker = CompatibilityChecker::new();
        let configs = vec![(
            PathBuf::from("config1.json"),
            serde_json::json!({"key": "value"}),
        )];

        let result = checker.check(&configs);
        assert!(result.is_err());
    }

    #[test]
    fn test_conflict_severity() {
        let conflict = Conflict::new(
            "Test",
            "$.path",
            serde_json::json!(1),
            serde_json::json!(2),
            "f1",
            "f2",
        )
        .with_severity(ConflictSeverity::Warning);

        assert_eq!(conflict.severity, ConflictSeverity::Warning);
    }
}
