//! Deprecated field detection rules
//!
//! This module provides rules for detecting deprecated configuration
//! fields and suggesting modern alternatives.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding};
use crate::ConfigValue;

/// Information about a deprecated field
#[derive(Debug, Clone)]
pub struct DeprecatedFieldInfo {
    /// The deprecated field path
    pub field_path: String,
    /// When the field was deprecated (optional)
    pub deprecated_since: Option<String>,
    /// When the field will be removed (optional)
    pub removal_version: Option<String>,
    /// The replacement field path (if any)
    pub replacement: Option<String>,
    /// Additional migration instructions
    pub migration_notes: Option<String>,
    /// Severity level for this deprecation
    pub severity: Severity,
}

impl DeprecatedFieldInfo {
    /// Create new deprecated field info
    pub fn new(field_path: impl Into<String>) -> Self {
        Self {
            field_path: field_path.into(),
            deprecated_since: None,
            removal_version: None,
            replacement: None,
            migration_notes: None,
            severity: Severity::Warning,
        }
    }

    /// Set the version when field was deprecated
    pub fn deprecated_since(mut self, version: impl Into<String>) -> Self {
        self.deprecated_since = Some(version.into());
        self
    }

    /// Set the version when field will be removed
    pub fn removal_version(mut self, version: impl Into<String>) -> Self {
        self.removal_version = Some(version.into());
        self
    }

    /// Set the replacement field
    pub fn replacement(mut self, field_path: impl Into<String>) -> Self {
        self.replacement = Some(field_path.into());
        self
    }

    /// Set migration notes
    pub fn migration_notes(mut self, notes: impl Into<String>) -> Self {
        self.migration_notes = Some(notes.into());
        self
    }

    /// Set severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

/// Rule for detecting deprecated fields
pub struct DeprecatedFieldRule {
    id: String,
    name: String,
    /// Map of deprecated field paths to their info
    deprecated_fields: HashMap<String, DeprecatedFieldInfo>,
}

impl DeprecatedFieldRule {
    /// Create a new deprecated field rule
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            deprecated_fields: HashMap::new(),
        }
    }

    /// Add a deprecated field
    pub fn add_deprecated(mut self, info: DeprecatedFieldInfo) -> Self {
        self.deprecated_fields.insert(info.field_path.clone(), info);
        self
    }

    /// Add multiple deprecated fields
    pub fn add_deprecated_fields<I>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = DeprecatedFieldInfo>,
    {
        for info in fields {
            self.deprecated_fields.insert(info.field_path.clone(), info);
        }
        self
    }

    fn field_exists(&self, value: &ConfigValue, path: &str) -> bool {
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
            _ => parts.is_empty(),
        }
    }

    fn build_message(&self, info: &DeprecatedFieldInfo) -> String {
        let mut message = format!("Field '{}' is deprecated", info.field_path);

        if let Some(since) = &info.deprecated_since {
            message.push_str(&format!(" (since {})", since));
        }

        if let Some(removal) = &info.removal_version {
            message.push_str(&format!(", will be removed in {}", removal));
        }

        message
    }

    fn build_suggestion(&self, info: &DeprecatedFieldInfo) -> Option<String> {
        let mut suggestions = Vec::new();

        if let Some(replacement) = &info.replacement {
            suggestions.push(format!("Use '{}' instead", replacement));
        }

        if let Some(notes) = &info.migration_notes {
            suggestions.push(notes.clone());
        }

        if suggestions.is_empty() {
            None
        } else {
            Some(suggestions.join(". "))
        }
    }
}

#[async_trait]
impl Rule for DeprecatedFieldRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Detects deprecated configuration fields"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Deprecated
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

        for (field_path, info) in &self.deprecated_fields {
            if self.field_exists(value, field_path) {
                let full_path = if path.is_empty() {
                    field_path.clone()
                } else {
                    format!("{}.{}", path, field_path)
                };

                let mut finding = ValidationFinding::new(
                    &self.id,
                    RuleCategory::Deprecated,
                    info.severity,
                    self.build_message(info),
                    &full_path,
                );

                if let Some(replacement) = &info.replacement {
                    finding = finding.with_expected(format!("Use '{}'", replacement));
                }

                if let Some(suggestion) = self.build_suggestion(info) {
                    finding = finding.with_suggestion(suggestion);
                }

                // Add context with deprecation details
                let context = serde_json::json!({
                    "deprecated_since": info.deprecated_since,
                    "removal_version": info.removal_version,
                    "replacement": info.replacement,
                });
                finding = finding.with_context(context);

                findings.push(finding);
            }
        }

        findings
    }
}

/// Rule for detecting deprecated values
pub struct DeprecatedValueRule {
    id: String,
    name: String,
    field_path: String,
    /// Map of deprecated values to their info
    deprecated_values: HashMap<String, DeprecatedValueInfo>,
}

/// Information about a deprecated value
#[derive(Debug, Clone)]
pub struct DeprecatedValueInfo {
    /// The deprecated value
    pub value: String,
    /// The replacement value (if any)
    pub replacement: Option<String>,
    /// Additional notes
    pub notes: Option<String>,
    /// Severity level
    pub severity: Severity,
}

impl DeprecatedValueInfo {
    /// Create new deprecated value info
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            replacement: None,
            notes: None,
            severity: Severity::Warning,
        }
    }

    /// Set the replacement value
    pub fn replacement(mut self, value: impl Into<String>) -> Self {
        self.replacement = Some(value.into());
        self
    }

    /// Set additional notes
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Set severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

impl DeprecatedValueRule {
    /// Create a new deprecated value rule
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        field_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            field_path: field_path.into(),
            deprecated_values: HashMap::new(),
        }
    }

    /// Add a deprecated value
    pub fn add_deprecated_value(mut self, info: DeprecatedValueInfo) -> Self {
        self.deprecated_values.insert(info.value.clone(), info);
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
impl Rule for DeprecatedValueRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Detects deprecated configuration values"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Deprecated
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

        let full_path = if path.is_empty() {
            self.field_path.clone()
        } else {
            format!("{}.{}", path, self.field_path)
        };

        if let Some(field_value) = self.get_value_at_path(value, &self.field_path) {
            if let ConfigValue::String(s) = field_value {
                if let Some(info) = self.deprecated_values.get(s) {
                    let mut finding = ValidationFinding::new(
                        &self.id,
                        RuleCategory::Deprecated,
                        info.severity,
                        format!("Value '{}' is deprecated", s),
                        &full_path,
                    )
                    .with_actual(s.clone());

                    if let Some(replacement) = &info.replacement {
                        finding = finding
                            .with_expected(replacement.clone())
                            .with_suggestion(format!("Use '{}' instead", replacement));
                    }

                    if let Some(notes) = &info.notes {
                        finding = finding.with_context(serde_json::json!({
                            "notes": notes,
                        }));
                    }

                    findings.push(finding);
                }
            }
        }

        findings
    }
}

/// Rule for detecting sunset dates (time-based deprecation)
pub struct SunsetRule {
    id: String,
    name: String,
    /// Map of field paths to their sunset dates
    sunset_dates: HashMap<String, DateTime<Utc>>,
    /// Current time for testing (defaults to now)
    current_time: Option<DateTime<Utc>>,
}

impl SunsetRule {
    /// Create a new sunset rule
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            sunset_dates: HashMap::new(),
            current_time: None,
        }
    }

    /// Add a field with sunset date
    pub fn add_sunset(mut self, field_path: impl Into<String>, sunset_date: DateTime<Utc>) -> Self {
        self.sunset_dates.insert(field_path.into(), sunset_date);
        self
    }

    /// Set current time (for testing)
    pub fn with_current_time(mut self, time: DateTime<Utc>) -> Self {
        self.current_time = Some(time);
        self
    }

    fn field_exists(&self, value: &ConfigValue, path: &str) -> bool {
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
            _ => parts.is_empty(),
        }
    }

    fn current_time(&self) -> DateTime<Utc> {
        self.current_time.unwrap_or_else(Utc::now)
    }
}

#[async_trait]
impl Rule for SunsetRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Detects configuration fields approaching or past sunset dates"
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Deprecated
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
        let now = self.current_time();

        for (field_path, sunset_date) in &self.sunset_dates {
            if self.field_exists(value, field_path) {
                let full_path = if path.is_empty() {
                    field_path.clone()
                } else {
                    format!("{}.{}", path, field_path)
                };

                let days_until_sunset = (*sunset_date - now).num_days();

                let (severity, message) = if days_until_sunset < 0 {
                    (
                        Severity::Error,
                        format!(
                            "Field '{}' was sunset on {} ({} days ago)",
                            field_path,
                            sunset_date.format("%Y-%m-%d"),
                            -days_until_sunset
                        ),
                    )
                } else if days_until_sunset <= 30 {
                    (
                        Severity::Warning,
                        format!(
                            "Field '{}' will be sunset on {} (in {} days)",
                            field_path,
                            sunset_date.format("%Y-%m-%d"),
                            days_until_sunset
                        ),
                    )
                } else {
                    (
                        Severity::Info,
                        format!(
                            "Field '{}' will be sunset on {} (in {} days)",
                            field_path,
                            sunset_date.format("%Y-%m-%d"),
                            days_until_sunset
                        ),
                    )
                };

                findings.push(
                    ValidationFinding::new(&self.id, RuleCategory::Deprecated, severity, message, &full_path)
                        .with_context(serde_json::json!({
                            "sunset_date": sunset_date.to_rfc3339(),
                            "days_until_sunset": days_until_sunset,
                        }))
                        .with_suggestion("Remove or migrate this field before the sunset date"),
                );
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;
    use chrono::Duration;
    use std::collections::HashMap;

    fn make_context() -> RuleContext {
        RuleContext::new(Environment::Development, "test")
    }

    #[tokio::test]
    async fn test_deprecated_field_rule() {
        let rule = DeprecatedFieldRule::new("dep_001", "Deprecated Fields")
            .add_deprecated(
                DeprecatedFieldInfo::new("old_setting")
                    .deprecated_since("1.0.0")
                    .replacement("new_setting")
                    .migration_notes("Update your configuration to use the new setting"),
            );

        let mut obj = HashMap::new();
        obj.insert("old_setting".to_string(), ConfigValue::String("value".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("deprecated"));
        assert!(findings[0].suggestion.as_ref().unwrap().contains("new_setting"));
    }

    #[tokio::test]
    async fn test_deprecated_field_not_present() {
        let rule = DeprecatedFieldRule::new("dep_002", "Deprecated Fields")
            .add_deprecated(DeprecatedFieldInfo::new("old_setting"));

        let mut obj = HashMap::new();
        obj.insert("new_setting".to_string(), ConfigValue::String("value".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_deprecated_value_rule() {
        let rule = DeprecatedValueRule::new("dep_003", "Deprecated Values", "log_level")
            .add_deprecated_value(
                DeprecatedValueInfo::new("verbose")
                    .replacement("debug")
                    .notes("verbose is now called debug"),
            );

        let mut obj = HashMap::new();
        obj.insert("log_level".to_string(), ConfigValue::String("verbose".to_string()));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].suggestion.as_ref().unwrap().contains("debug"));
    }

    #[tokio::test]
    async fn test_sunset_rule_past() {
        let now = Utc::now();
        let past_date = now - Duration::days(10);

        let rule = SunsetRule::new("sun_001", "Sunset Dates")
            .add_sunset("legacy_feature", past_date)
            .with_current_time(now);

        let mut obj = HashMap::new();
        obj.insert("legacy_feature".to_string(), ConfigValue::Boolean(true));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("sunset"));
    }

    #[tokio::test]
    async fn test_sunset_rule_approaching() {
        let now = Utc::now();
        let future_date = now + Duration::days(15);

        let rule = SunsetRule::new("sun_002", "Sunset Dates")
            .add_sunset("soon_deprecated", future_date)
            .with_current_time(now);

        let mut obj = HashMap::new();
        obj.insert("soon_deprecated".to_string(), ConfigValue::Boolean(true));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[tokio::test]
    async fn test_sunset_rule_far_future() {
        let now = Utc::now();
        let future_date = now + Duration::days(90);

        let rule = SunsetRule::new("sun_003", "Sunset Dates")
            .add_sunset("future_deprecated", future_date)
            .with_current_time(now);

        let mut obj = HashMap::new();
        obj.insert("future_deprecated".to_string(), ConfigValue::Boolean(true));
        let value = ConfigValue::Object(obj);

        let findings = rule.evaluate(&value, "", &make_context()).await;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
    }
}
