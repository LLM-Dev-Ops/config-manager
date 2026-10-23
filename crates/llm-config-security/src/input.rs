//! Input validation and sanitization

use crate::errors::{SecurityError, SecurityResult};
use regex::Regex;
use std::sync::OnceLock;

/// SQL injection patterns
static SQL_INJECTION_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// XSS patterns
static XSS_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// Path traversal patterns
static PATH_TRAVERSAL_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// Command injection patterns
static COMMAND_INJECTION_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// LDAP injection patterns
static LDAP_INJECTION_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

/// Initialize security patterns
fn init_patterns() {
    SQL_INJECTION_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?i)(\bunion\b.*\bselect\b)").unwrap(),
            Regex::new(r"(?i)(\bdrop\b.*\btable\b)").unwrap(),
            Regex::new(r"(?i)(\binsert\b.*\binto\b)").unwrap(),
            Regex::new(r"(?i)(\bdelete\b.*\bfrom\b)").unwrap(),
            Regex::new(r"(?i)(\bupdate\b.*\bset\b)").unwrap(),
            Regex::new(r"(?i)(;.*(--)|(#))").unwrap(),
            Regex::new(r#"(?i)\bor\b\s*['"]?[[:alnum:]_]+['"]?\s*=\s*['"]?[[:alnum:]_]+['"]?"#)
                .unwrap(),
            Regex::new(r"(?i)\bexec(\s|\+)+(s|x)p\w+").unwrap(),
        ]
    });

    XSS_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?i)<script[^>]*>.*?</script>").unwrap(),
            Regex::new(r"(?i)javascript:").unwrap(),
            Regex::new(r"(?i)on\w+\s*=").unwrap(),
            Regex::new(r"(?i)<iframe").unwrap(),
            Regex::new(r"(?i)<embed").unwrap(),
            Regex::new(r"(?i)<object").unwrap(),
            Regex::new(r"(?i)eval\(").unwrap(),
            Regex::new(r"(?i)expression\(").unwrap(),
        ]
    });

    PATH_TRAVERSAL_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"\.\./").unwrap(),
            Regex::new(r"\.\./").unwrap(),
            Regex::new(r"%2e%2e/").unwrap(),
            Regex::new(r"%2e%2e\\").unwrap(),
            Regex::new(r"\.\.\\").unwrap(),
        ]
    });

    COMMAND_INJECTION_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"[;&|`$\n]").unwrap(),
            Regex::new(r"\$\(.*\)").unwrap(),
            Regex::new(r"`.*`").unwrap(),
        ]
    });

    LDAP_INJECTION_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"[*()\\]").unwrap(),
            Regex::new(r"\x00").unwrap(),
        ]
    });
}

/// Configuration for input sanitization
#[derive(Debug, Clone)]
pub struct SanitizationConfig {
    /// Maximum input length
    pub max_length: usize,
    /// Allow special characters
    pub allow_special_chars: bool,
    /// Allow HTML
    pub allow_html: bool,
    /// Trim whitespace
    pub trim_whitespace: bool,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        Self {
            max_length: 1000,
            allow_special_chars: false,
            allow_html: false,
            trim_whitespace: true,
        }
    }
}

/// Input validator for security
pub struct InputValidator {
    config: SanitizationConfig,
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new(SanitizationConfig::default())
    }
}

impl InputValidator {
    /// Create a new input validator
    pub fn new(config: SanitizationConfig) -> Self {
        init_patterns();
        Self { config }
    }

    /// Validate and sanitize input
    pub fn validate(&self, input: &str) -> SecurityResult<String> {
        // Check length
        if input.len() > self.config.max_length {
            return Err(SecurityError::ValidationError(format!(
                "Input exceeds maximum length of {} characters",
                self.config.max_length
            )));
        }

        // Detect SQL injection
        if self.detect_sql_injection(input) {
            return Err(SecurityError::SqlInjectionAttempt);
        }

        // Detect XSS
        if self.detect_xss(input) {
            return Err(SecurityError::XssAttempt);
        }

        // Detect path traversal
        if self.detect_path_traversal(input) {
            return Err(SecurityError::PathTraversalAttempt);
        }

        // Detect command injection
        if self.detect_command_injection(input) {
            return Err(SecurityError::CommandInjectionAttempt);
        }

        // Detect LDAP injection
        if self.detect_ldap_injection(input) {
            return Err(SecurityError::LdapInjectionAttempt);
        }

        // Sanitize
        let sanitized = self.sanitize(input);

        Ok(sanitized)
    }

    /// Detect SQL injection attempts
    fn detect_sql_injection(&self, input: &str) -> bool {
        SQL_INJECTION_PATTERNS
            .get()
            .unwrap()
            .iter()
            .any(|pattern| pattern.is_match(input))
    }

    /// Detect XSS attempts
    fn detect_xss(&self, input: &str) -> bool {
        if !self.config.allow_html {
            XSS_PATTERNS
                .get()
                .unwrap()
                .iter()
                .any(|pattern| pattern.is_match(input))
        } else {
            false
        }
    }

    /// Detect path traversal attempts
    fn detect_path_traversal(&self, input: &str) -> bool {
        PATH_TRAVERSAL_PATTERNS
            .get()
            .unwrap()
            .iter()
            .any(|pattern| pattern.is_match(input))
    }

    /// Detect command injection attempts
    fn detect_command_injection(&self, input: &str) -> bool {
        COMMAND_INJECTION_PATTERNS
            .get()
            .unwrap()
            .iter()
            .any(|pattern| pattern.is_match(input))
    }

    /// Detect LDAP injection attempts
    fn detect_ldap_injection(&self, input: &str) -> bool {
        LDAP_INJECTION_PATTERNS
            .get()
            .unwrap()
            .iter()
            .any(|pattern| pattern.is_match(input))
    }

    /// Sanitize input
    fn sanitize(&self, input: &str) -> String {
        let mut sanitized = input.to_string();

        // Trim whitespace
        if self.config.trim_whitespace {
            sanitized = sanitized.trim().to_string();
        }

        // Remove null bytes
        sanitized = sanitized.replace('\0', "");

        // HTML encode if not allowing HTML
        if !self.config.allow_html {
            sanitized = html_escape(&sanitized);
        }

        // Remove control characters
        sanitized = sanitized
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .collect();

        sanitized
    }

    /// Validate an email address
    pub fn validate_email(&self, email: &str) -> SecurityResult<String> {
        let email_regex = Regex::new(
            r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
        ).unwrap();

        if !email_regex.is_match(email) {
            return Err(SecurityError::ValidationError(
                "Invalid email format".to_string(),
            ));
        }

        Ok(email.to_lowercase())
    }

    /// Validate a username
    pub fn validate_username(&self, username: &str) -> SecurityResult<String> {
        // Username: alphanumeric, underscore, hyphen, 3-30 characters
        let username_regex = Regex::new(r"^[a-zA-Z0-9_-]{3,30}$").unwrap();

        if !username_regex.is_match(username) {
            return Err(SecurityError::ValidationError(
                "Username must be 3-30 alphanumeric characters, underscore, or hyphen".to_string(),
            ));
        }

        Ok(username.to_string())
    }

    /// Validate a configuration key
    pub fn validate_config_key(&self, key: &str) -> SecurityResult<String> {
        // Config key: alphanumeric, underscore, hyphen, dot, slash
        let key_regex = Regex::new(r"^[a-zA-Z0-9_\-./]{1,200}$").unwrap();

        if !key_regex.is_match(key) {
            return Err(SecurityError::ValidationError(
                "Invalid configuration key format".to_string(),
            ));
        }

        // Additional checks
        if self.detect_path_traversal(key) {
            return Err(SecurityError::PathTraversalAttempt);
        }

        Ok(key.to_string())
    }

    /// Validate a URL
    pub fn validate_url(&self, url: &str) -> SecurityResult<String> {
        // Basic URL validation
        let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").unwrap();

        if !url_regex.is_match(url) {
            return Err(SecurityError::ValidationError(
                "Invalid URL format".to_string(),
            ));
        }

        // Check for suspicious patterns
        if self.detect_xss(url) {
            return Err(SecurityError::XssAttempt);
        }

        Ok(url.to_string())
    }

    /// Validate JSON input
    pub fn validate_json(&self, json: &str) -> SecurityResult<serde_json::Value> {
        // Check for suspicious patterns before parsing
        if self.detect_xss(json) {
            return Err(SecurityError::XssAttempt);
        }

        // Parse JSON
        serde_json::from_str(json)
            .map_err(|e| SecurityError::ValidationError(format!("Invalid JSON: {}", e)))
    }
}

/// HTML escape special characters
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        let validator = InputValidator::default();

        // Should detect SQL injection
        assert!(validator
            .validate("' OR '1'='1")
            .is_err_and(|e| matches!(e, SecurityError::SqlInjectionAttempt)));

        assert!(validator
            .validate("'; DROP TABLE users; --")
            .is_err_and(|e| matches!(e, SecurityError::SqlInjectionAttempt)));

        // Should allow normal input
        assert!(validator.validate("normal text").is_ok());
    }

    #[test]
    fn test_xss_detection() {
        let validator = InputValidator::default();

        // Should detect XSS
        assert!(validator
            .validate("<script>alert('XSS')</script>")
            .is_err_and(|e| matches!(e, SecurityError::XssAttempt)));

        assert!(validator
            .validate("javascript:alert('XSS')")
            .is_err_and(|e| matches!(e, SecurityError::XssAttempt)));

        // Should allow normal input
        assert!(validator.validate("normal text").is_ok());
    }

    #[test]
    fn test_path_traversal_detection() {
        let validator = InputValidator::default();

        // Should detect path traversal
        assert!(validator
            .validate("../../etc/passwd")
            .is_err_and(|e| matches!(e, SecurityError::PathTraversalAttempt)));

        // Should allow normal paths
        assert!(validator.validate("normal/path").is_ok());
    }

    #[test]
    fn test_command_injection_detection() {
        let validator = InputValidator::default();

        // Should detect command injection
        assert!(validator
            .validate("test; rm -rf /")
            .is_err_and(|e| matches!(e, SecurityError::CommandInjectionAttempt)));

        assert!(validator
            .validate("$(malicious)")
            .is_err_and(|e| matches!(e, SecurityError::CommandInjectionAttempt)));

        // Should allow normal input
        assert!(validator.validate("normal text").is_ok());
    }

    #[test]
    fn test_ldap_injection_detection() {
        let validator = InputValidator::default();

        assert!(validator
            .validate("(uid=admin)")
            .is_err_and(|e| matches!(e, SecurityError::LdapInjectionAttempt)));

        assert!(validator.validate("normal text").is_ok());
    }

    #[test]
    fn test_email_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_email("user@example.com").is_ok());
        assert!(validator.validate_email("invalid.email").is_err());
        assert!(validator.validate_email("@example.com").is_err());
    }

    #[test]
    fn test_username_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_username("user123").is_ok());
        assert!(validator.validate_username("user-name_123").is_ok());
        assert!(validator.validate_username("ab").is_err()); // Too short
        assert!(validator.validate_username("user@name").is_err()); // Invalid char
    }

    #[test]
    fn test_config_key_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_config_key("app/config/key").is_ok());
        assert!(validator.validate_config_key("app.config.key").is_ok());
        assert!(validator.validate_config_key("../etc/passwd").is_err());
    }

    #[test]
    fn test_url_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_url("https://example.com").is_ok());
        assert!(validator.validate_url("http://example.com/path").is_ok());
        assert!(validator.validate_url("invalid-url").is_err());
        assert!(validator.validate_url("javascript:alert('XSS')").is_err());
    }

    #[test]
    fn test_json_validation() {
        let validator = InputValidator::default();

        assert!(validator.validate_json(r#"{"key": "value"}"#).is_ok());
        assert!(validator.validate_json("invalid json").is_err());
        assert!(validator
            .validate_json(r#"{"xss": "<script>alert('XSS')</script>"}"#)
            .is_err());
    }

    #[test]
    fn test_sanitization() {
        let validator = InputValidator::default();

        let result = validator.validate("  test input  ").unwrap();
        assert_eq!(result, "test input");

        let result = validator.validate("test<script>").unwrap();
        assert!(result.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_length_validation() {
        let config = SanitizationConfig {
            max_length: 10,
            ..Default::default()
        };
        let validator = InputValidator::new(config);

        assert!(validator.validate("short").is_ok());
        assert!(validator.validate("this is way too long").is_err());
    }
}
