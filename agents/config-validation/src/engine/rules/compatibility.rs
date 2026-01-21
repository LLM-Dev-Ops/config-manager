//! Cross-agent and cross-service compatibility rules
//!
//! This module provides rules that validate configuration compatibility
//! across different services and agents in the platform.

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding, FindingBuilder};
use crate::ConfigValue;
use async_trait::async_trait;
use std::collections::HashMap;

/// Rule for cross-service compatibility validation
pub struct CompatibilityRule {
    id: String,
    name: String,
    description: String,
    /// Known service dependencies and their required configurations
    service_requirements: HashMap<String, Vec<ServiceRequirement>>,
}

/// A requirement that a service has for compatible configuration
#[derive(Debug, Clone)]
pub struct ServiceRequirement {
    /// Name of the required field
    pub field: String,
    /// Expected format or pattern
    pub format: Option<String>,
    /// Minimum version requirement
    pub min_version: Option<String>,
    /// Description of the requirement
    pub description: String,
}

impl CompatibilityRule {
    pub fn new() -> Self {
        let mut rule = Self {
            id: "compatibility_check".to_string(),
            name: "Cross-Service Compatibility".to_string(),
            description: "Validates configuration compatibility across services".to_string(),
            service_requirements: HashMap::new(),
        };
        rule.register_default_requirements();
        rule
    }

    fn register_default_requirements(&mut self) {
        // Database service requirements
        self.service_requirements.insert(
            "database".to_string(),
            vec![
                ServiceRequirement {
                    field: "host".to_string(),
                    format: Some("hostname".to_string()),
                    min_version: None,
                    description: "Database host must be a valid hostname".to_string(),
                },
                ServiceRequirement {
                    field: "port".to_string(),
                    format: Some("port".to_string()),
                    min_version: None,
                    description: "Database port must be a valid port number".to_string(),
                },
            ],
        );

        // Cache service requirements
        self.service_requirements.insert(
            "cache".to_string(),
            vec![
                ServiceRequirement {
                    field: "ttl".to_string(),
                    format: Some("duration".to_string()),
                    min_version: None,
                    description: "Cache TTL must be a valid duration".to_string(),
                },
            ],
        );

        // API service requirements
        self.service_requirements.insert(
            "api".to_string(),
            vec![
                ServiceRequirement {
                    field: "base_url".to_string(),
                    format: Some("url".to_string()),
                    min_version: None,
                    description: "API base URL must be a valid URL".to_string(),
                },
                ServiceRequirement {
                    field: "timeout".to_string(),
                    format: Some("duration_ms".to_string()),
                    min_version: None,
                    description: "API timeout must be specified in milliseconds".to_string(),
                },
            ],
        );
    }
}

impl Default for CompatibilityRule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Rule for CompatibilityRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> RuleCategory {
        RuleCategory::Compatibility
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        if let ConfigValue::Object(obj) = value {
            self.check_compatibility(obj, path, context, &mut findings);
        }

        findings
    }
}

impl CompatibilityRule {
    fn check_compatibility(
        &self,
        obj: &HashMap<String, ConfigValue>,
        base_path: &str,
        context: &RuleContext,
        findings: &mut Vec<ValidationFinding>,
    ) {
        // Check for known service configurations
        for (key, value) in obj {
            let path = if base_path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", base_path, key)
            };

            // Check if this is a known service configuration
            if let Some(requirements) = self.service_requirements.get(key) {
                if let ConfigValue::Object(service_config) = value {
                    self.validate_service_requirements(
                        key,
                        service_config,
                        &path,
                        requirements,
                        findings,
                    );
                }
            }

            // Check for protocol version mismatches
            self.check_protocol_versions(key, value, &path, findings);

            // Check for connection string format compatibility
            self.check_connection_formats(key, value, &path, findings);

            // Recurse into nested objects
            if let ConfigValue::Object(nested) = value {
                self.check_compatibility(nested, &path, context, findings);
            }
        }

        // Cross-field compatibility checks
        self.check_cross_field_compatibility(obj, base_path, findings);
    }

    fn validate_service_requirements(
        &self,
        service: &str,
        config: &HashMap<String, ConfigValue>,
        base_path: &str,
        requirements: &[ServiceRequirement],
        findings: &mut Vec<ValidationFinding>,
    ) {
        for req in requirements {
            let field_path = format!("{}.{}", base_path, req.field);

            if let Some(value) = config.get(&req.field) {
                // Validate format if specified
                if let Some(format) = &req.format {
                    if !self.validate_format(value, format) {
                        findings.push(
                            FindingBuilder::new(&self.id, RuleCategory::Compatibility, &field_path)
                                .severity(Severity::Error)
                                .build(format!(
                                    "Field '{}' in service '{}' does not match expected format '{}'",
                                    req.field, service, format
                                ))
                                .with_suggestion(&req.description)
                        );
                    }
                }
            }
        }
    }

    fn validate_format(&self, value: &ConfigValue, format: &str) -> bool {
        match format {
            "hostname" => {
                if let ConfigValue::String(s) = value {
                    // Basic hostname validation
                    !s.is_empty() && !s.contains(' ')
                } else {
                    false
                }
            }
            "port" => {
                if let ConfigValue::Integer(p) = value {
                    *p > 0 && *p <= 65535
                } else {
                    false
                }
            }
            "url" => {
                if let ConfigValue::String(s) = value {
                    s.starts_with("http://") || s.starts_with("https://")
                } else {
                    false
                }
            }
            "duration" | "duration_ms" => {
                matches!(value, ConfigValue::Integer(i) if *i >= 0)
            }
            _ => true, // Unknown format, skip validation
        }
    }

    fn check_protocol_versions(
        &self,
        key: &str,
        value: &ConfigValue,
        path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        let key_lower = key.to_lowercase();

        // Check for version fields
        if key_lower.contains("version") || key_lower.contains("protocol") {
            if let ConfigValue::String(version) = value {
                // Check for deprecated versions
                let deprecated_versions = ["v1", "1.0", "0."];
                for deprecated in &deprecated_versions {
                    if version.starts_with(deprecated) {
                        findings.push(
                            FindingBuilder::new(&self.id, RuleCategory::Compatibility, path)
                                .severity(Severity::Warning)
                                .build(format!(
                                    "Protocol version '{}' may be deprecated",
                                    version
                                ))
                                .with_suggestion("Consider upgrading to a newer protocol version")
                        );
                    }
                }
            }
        }
    }

    fn check_connection_formats(
        &self,
        key: &str,
        value: &ConfigValue,
        path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        let key_lower = key.to_lowercase();

        // Check connection strings
        if key_lower.contains("connection") || key_lower.contains("dsn") || key_lower.contains("uri") {
            if let ConfigValue::String(conn_str) = value {
                // Check for credentials in connection string
                if conn_str.contains("password=") || conn_str.contains(":password@") {
                    findings.push(
                        FindingBuilder::new(&self.id, RuleCategory::Compatibility, path)
                            .severity(Severity::Critical)
                            .build("Connection string contains embedded credentials")
                            .with_suggestion("Use separate credential fields or secret references")
                    );
                }

                // Check for compatible format
                if !conn_str.contains("://") && !conn_str.contains("=") {
                    findings.push(
                        FindingBuilder::new(&self.id, RuleCategory::Compatibility, path)
                            .severity(Severity::Warning)
                            .build("Connection string format may not be compatible")
                            .with_suggestion("Use standard URI or key=value format")
                    );
                }
            }
        }
    }

    fn check_cross_field_compatibility(
        &self,
        obj: &HashMap<String, ConfigValue>,
        base_path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        // Check database + cache TTL alignment
        let has_database = obj.contains_key("database");
        let has_cache = obj.contains_key("cache");

        if has_database && has_cache {
            // Recommend cache invalidation strategy when both are present
            if let Some(ConfigValue::Object(cache_config)) = obj.get("cache") {
                if !cache_config.contains_key("invalidation") {
                    findings.push(
                        FindingBuilder::new(&self.id, RuleCategory::Compatibility, base_path)
                            .severity(Severity::Info)
                            .build("Database and cache are configured but no invalidation strategy specified")
                            .with_suggestion("Consider adding cache.invalidation configuration for data consistency")
                    );
                }
            }
        }

        // Check for retry + timeout compatibility
        if let (Some(timeout), Some(retry)) = (obj.get("timeout"), obj.get("retry")) {
            if let (ConfigValue::Integer(timeout_ms), ConfigValue::Object(retry_config)) = (timeout, retry) {
                if let Some(ConfigValue::Integer(max_retries)) = retry_config.get("max_retries") {
                    // Warn if total retry time could exceed reasonable limits
                    if *max_retries > 5 && *timeout_ms > 30000 {
                        let path = if base_path.is_empty() {
                            "retry".to_string()
                        } else {
                            format!("{}.retry", base_path)
                        };
                        findings.push(
                            FindingBuilder::new(&self.id, RuleCategory::Compatibility, &path)
                                .severity(Severity::Warning)
                                .build("High retry count with long timeout may cause excessive delays")
                                .with_suggestion("Consider reducing retries or timeout for better responsiveness")
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;

    #[tokio::test]
    async fn test_database_port_validation() {
        let rule = CompatibilityRule::new();
        let config = ConfigValue::Object(
            [(
                "database".to_string(),
                ConfigValue::Object(
                    [
                        ("host".to_string(), ConfigValue::String("localhost".to_string())),
                        ("port".to_string(), ConfigValue::Integer(99999)), // Invalid port
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        );
        let context = RuleContext::new(Environment::Production, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.message.contains("port")));
    }

    #[tokio::test]
    async fn test_connection_string_credentials() {
        let rule = CompatibilityRule::new();
        let config = ConfigValue::Object(
            [(
                "connection_string".to_string(),
                ConfigValue::String("postgres://user:password@host/db".to_string()),
            )]
            .into_iter()
            .collect(),
        );
        let context = RuleContext::new(Environment::Production, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.severity == Severity::Critical));
    }

    #[tokio::test]
    async fn test_valid_config_no_errors() {
        let rule = CompatibilityRule::new();
        let config = ConfigValue::Object(
            [(
                "database".to_string(),
                ConfigValue::Object(
                    [
                        ("host".to_string(), ConfigValue::String("db.example.com".to_string())),
                        ("port".to_string(), ConfigValue::Integer(5432)),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        );
        let context = RuleContext::new(Environment::Production, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        // Valid config should not produce errors
        assert!(findings.iter().all(|f| f.severity != Severity::Error));
    }
}
