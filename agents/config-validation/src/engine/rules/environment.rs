//! Environment-specific validation rules
//!
//! This module provides rules that validate configuration based on
//! the target deployment environment (dev, staging, production, etc.).

use super::{Rule, RuleCategory, RuleContext, Severity, ValidationFinding, FindingBuilder};
use crate::{ConfigValue, Environment};
use async_trait::async_trait;

/// Rule for environment-specific validation
pub struct EnvironmentRule {
    id: String,
    name: String,
    description: String,
}

impl EnvironmentRule {
    pub fn new() -> Self {
        Self {
            id: "environment_check".to_string(),
            name: "Environment-Specific Validation".to_string(),
            description: "Validates configuration against environment-specific constraints".to_string(),
        }
    }
}

impl Default for EnvironmentRule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Rule for EnvironmentRule {
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
        RuleCategory::Environment
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    async fn evaluate(
        &self,
        value: &ConfigValue,
        path: &str,
        context: &RuleContext,
    ) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();

        if let ConfigValue::Object(obj) = value {
            self.check_environment_rules(obj, path, context, &mut findings);
        }

        findings
    }
}

impl EnvironmentRule {
    fn check_environment_rules(
        &self,
        obj: &std::collections::HashMap<String, ConfigValue>,
        base_path: &str,
        context: &RuleContext,
        findings: &mut Vec<ValidationFinding>,
    ) {
        for (key, value) in obj {
            let path = if base_path.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", base_path, key)
            };

            // Production-specific checks
            if matches!(context.environment, Environment::Production) {
                self.check_production_rules(key, value, &path, findings);
            }

            // Development-specific checks
            if matches!(context.environment, Environment::Development) {
                self.check_development_rules(key, value, &path, findings);
            }

            // Staging-specific checks
            if matches!(context.environment, Environment::Staging) {
                self.check_staging_rules(key, value, &path, findings);
            }

            // Recurse into nested objects
            if let ConfigValue::Object(nested) = value {
                self.check_environment_rules(nested, &path, context, findings);
            }
        }
    }

    fn check_production_rules(
        &self,
        key: &str,
        value: &ConfigValue,
        path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        // Check for debug/development settings in production
        let debug_keywords = ["debug", "verbose", "trace", "dev", "test", "mock"];
        let key_lower = key.to_lowercase();

        for keyword in &debug_keywords {
            if key_lower.contains(keyword) {
                if let ConfigValue::Boolean(true) = value {
                    findings.push(
                        FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                            .severity(Severity::Warning)
                            .build(format!(
                                "Debug/development setting '{}' is enabled in production",
                                key
                            ))
                            .with_suggestion("Disable debug settings in production environment")
                    );
                }
            }
        }

        // Check for localhost/127.0.0.1 in production
        if let ConfigValue::String(s) = value {
            if s.contains("localhost") || s.contains("127.0.0.1") || s.contains("0.0.0.0") {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Error)
                        .build(format!(
                            "Local address '{}' found in production configuration",
                            s
                        ))
                        .with_suggestion("Use production-appropriate hostname or IP address")
                );
            }
        }

        // Check for insecure protocols in production
        if let ConfigValue::String(s) = value {
            if s.starts_with("http://") && !s.contains("localhost") {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Error)
                        .build("Insecure HTTP protocol in production configuration")
                        .with_suggestion("Use HTTPS for production URLs")
                );
            }
        }

        // Check for weak timeouts in production
        if key_lower.contains("timeout") {
            if let ConfigValue::Integer(timeout) = value {
                if *timeout > 60000 {
                    findings.push(
                        FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                            .severity(Severity::Warning)
                            .build(format!(
                                "Long timeout value ({} ms) may cause issues in production",
                                timeout
                            ))
                            .with_suggestion("Consider shorter timeouts for production resilience")
                    );
                }
            }
        }

        // Check for TLS/SSL disabled
        if (key_lower.contains("ssl") || key_lower.contains("tls"))
            && (key_lower.contains("enable") || key_lower.contains("verify")) {
            if let ConfigValue::Boolean(false) = value {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Critical)
                        .build("SSL/TLS is disabled in production configuration")
                        .with_suggestion("Enable SSL/TLS for production security")
                );
            }
        }
    }

    fn check_development_rules(
        &self,
        key: &str,
        value: &ConfigValue,
        path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        let key_lower = key.to_lowercase();

        // Warn about production URLs in development
        if let ConfigValue::String(s) = value {
            if s.contains("prod") || s.contains("production") {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Warning)
                        .build("Production reference found in development configuration")
                        .with_suggestion("Ensure this is intentional for development environment")
                );
            }
        }

        // Info about missing debug settings
        if key_lower.contains("debug") || key_lower.contains("log_level") {
            if let ConfigValue::Boolean(false) = value {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Info)
                        .build("Debug settings are disabled in development")
                        .with_suggestion("Consider enabling debug settings for development")
                );
            }
        }
    }

    fn check_staging_rules(
        &self,
        key: &str,
        value: &ConfigValue,
        path: &str,
        findings: &mut Vec<ValidationFinding>,
    ) {
        // Staging should be similar to production but with some flexibility
        let key_lower = key.to_lowercase();

        // Check for production data references
        if let ConfigValue::String(s) = value {
            let s_lower = s.to_lowercase();
            if s_lower.contains("prod-db") || s_lower.contains("production-database") {
                findings.push(
                    FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                        .severity(Severity::Critical)
                        .build("Production database reference in staging configuration")
                        .with_suggestion("Use staging-specific database for isolation")
                );
            }
        }

        // Warn about test data that should not be in staging
        if key_lower.contains("seed") || key_lower.contains("fixture") {
            findings.push(
                FindingBuilder::new(&self.id, RuleCategory::Environment, path)
                    .severity(Severity::Info)
                    .build("Test data configuration found in staging environment")
                    .with_suggestion("Verify test data is appropriate for staging")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_production_debug_warning() {
        let rule = EnvironmentRule::new();
        let config = ConfigValue::Object(
            [("debug_enabled".to_string(), ConfigValue::Boolean(true))]
                .into_iter()
                .collect()
        );
        let context = RuleContext::new(Environment::Production, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.message.contains("debug")));
    }

    #[tokio::test]
    async fn test_production_localhost_error() {
        let rule = EnvironmentRule::new();
        let config = ConfigValue::Object(
            [("database_host".to_string(), ConfigValue::String("localhost:5432".to_string()))]
                .into_iter()
                .collect()
        );
        let context = RuleContext::new(Environment::Production, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.severity == Severity::Error));
    }

    #[tokio::test]
    async fn test_development_no_strict_errors() {
        let rule = EnvironmentRule::new();
        let config = ConfigValue::Object(
            [("debug_enabled".to_string(), ConfigValue::Boolean(true))]
                .into_iter()
                .collect()
        );
        let context = RuleContext::new(Environment::Development, "test");

        let findings = rule.evaluate(&config, "", &context).await;
        // Debug enabled in dev should not produce errors
        assert!(findings.iter().all(|f| f.severity != Severity::Error));
    }
}
