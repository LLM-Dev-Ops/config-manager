//! # LLM Config Security
//!
//! Enterprise-grade security hardening and validation for LLM Config Manager.
//!
//! ## Features
//!
//! - Input validation and sanitization
//! - Rate limiting and throttling
//! - Cryptographic operations validation
//! - Security policy enforcement
//! - Audit logging validation
//! - Attack prevention (SQLi, XSS, CSRF, etc.)

pub mod audit;
pub mod crypto;
pub mod errors;
pub mod input;
pub mod policy;
pub mod rate_limit;
pub mod validation;

pub use audit::{AuditConfig, AuditValidator};
pub use crypto::{CryptoValidator, KeyValidator};
pub use errors::{SecurityError, SecurityResult};
pub use input::{InputValidator, SanitizationConfig};
pub use policy::{PolicyEnforcer, SecurityPolicy};
pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use validation::{ValidationRule, Validator};

/// Security context for operations
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// User performing the operation
    pub user_id: String,
    /// IP address of the request
    pub ip_address: String,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Session identifier
    pub session_id: Option<String>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl SecurityContext {
    /// Create a new security context
    pub fn new(user_id: impl Into<String>, ip_address: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            ip_address: ip_address.into(),
            timestamp: chrono::Utc::now(),
            session_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Security hardening configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityConfig {
    /// Enable input validation
    pub enable_input_validation: bool,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Enable crypto validation
    pub enable_crypto_validation: bool,
    /// Enable audit validation
    pub enable_audit_validation: bool,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Session timeout in seconds
    pub session_timeout_seconds: u64,
    /// Password minimum length
    pub password_min_length: usize,
    /// Require special characters in passwords
    pub password_require_special: bool,
    /// Enable CORS
    pub enable_cors: bool,
    /// Allowed CORS origins
    pub cors_allowed_origins: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_input_validation: true,
            enable_rate_limiting: true,
            enable_crypto_validation: true,
            enable_audit_validation: true,
            max_request_size: 10 * 1024 * 1024, // 10MB
            session_timeout_seconds: 3600,      // 1 hour
            password_min_length: 12,
            password_require_special: true,
            enable_cors: false,
            cors_allowed_origins: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_context_creation() {
        let ctx = SecurityContext::new("user123", "192.168.1.1")
            .with_session("sess_abc123")
            .with_metadata("request_id", "req_xyz789");

        assert_eq!(ctx.user_id, "user123");
        assert_eq!(ctx.ip_address, "192.168.1.1");
        assert_eq!(ctx.session_id, Some("sess_abc123".to_string()));
        assert_eq!(
            ctx.metadata.get("request_id"),
            Some(&"req_xyz789".to_string())
        );
    }

    #[test]
    fn test_security_config_defaults() {
        let config = SecurityConfig::default();
        assert!(config.enable_input_validation);
        assert!(config.enable_rate_limiting);
        assert_eq!(config.max_request_size, 10 * 1024 * 1024);
        assert_eq!(config.password_min_length, 12);
    }
}
