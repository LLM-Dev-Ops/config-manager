//! Security error types

use thiserror::Error;

/// Security operation result type
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Security errors
#[derive(Error, Debug)]
pub enum SecurityError {
    /// Input validation failed
    #[error("Input validation failed: {0}")]
    ValidationError(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Cryptographic operation failed
    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),

    /// Policy violation
    #[error("Security policy violation: {0}")]
    PolicyViolation(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    /// Authorization failed
    #[error("Authorization failed: {0}")]
    AuthorizationError(String),

    /// Invalid session
    #[error("Invalid session: {0}")]
    InvalidSession(String),

    /// Suspicious activity detected
    #[error("Suspicious activity detected: {0}")]
    SuspiciousActivity(String),

    /// Configuration error
    #[error("Security configuration error: {0}")]
    ConfigError(String),

    /// Audit log validation failed
    #[error("Audit log validation failed: {0}")]
    AuditError(String),

    /// SQL injection attempt detected
    #[error("SQL injection attempt detected")]
    SqlInjectionAttempt,

    /// XSS attempt detected
    #[error("XSS attempt detected")]
    XssAttempt,

    /// Path traversal attempt detected
    #[error("Path traversal attempt detected")]
    PathTraversalAttempt,

    /// Command injection attempt detected
    #[error("Command injection attempt detected")]
    CommandInjectionAttempt,

    /// LDAP injection attempt detected
    #[error("LDAP injection attempt detected")]
    LdapInjectionAttempt,

    /// Regular expression DoS attempt
    #[error("Regular expression DoS attempt detected")]
    RegexDosAttempt,

    /// Request too large
    #[error("Request size exceeds maximum allowed: {0} bytes")]
    RequestTooLarge(usize),

    /// Invalid content type
    #[error("Invalid content type: {0}")]
    InvalidContentType(String),

    /// Weak password
    #[error("Password does not meet security requirements: {0}")]
    WeakPassword(String),

    /// Invalid token
    #[error("Invalid or expired token")]
    InvalidToken,

    /// CSRF token mismatch
    #[error("CSRF token mismatch")]
    CsrfTokenMismatch,

    /// Insecure protocol
    #[error("Insecure protocol: {0}")]
    InsecureProtocol(String),

    /// Certificate validation failed
    #[error("Certificate validation failed: {0}")]
    CertificateError(String),

    /// General error
    #[error("Security error: {0}")]
    General(String),
}

impl SecurityError {
    /// Get the severity level of the error
    pub fn severity(&self) -> Severity {
        match self {
            Self::SqlInjectionAttempt
            | Self::XssAttempt
            | Self::CommandInjectionAttempt
            | Self::PathTraversalAttempt
            | Self::SuspiciousActivity(_)
            | Self::AuthenticationError(_)
            | Self::AuthorizationError(_) => Severity::Critical,

            Self::RateLimitExceeded(_)
            | Self::PolicyViolation(_)
            | Self::WeakPassword(_)
            | Self::InvalidToken
            | Self::CsrfTokenMismatch => Severity::High,

            Self::ValidationError(_)
            | Self::RequestTooLarge(_)
            | Self::InvalidContentType(_)
            | Self::InvalidSession(_) => Severity::Medium,

            Self::ConfigError(_) | Self::AuditError(_) | Self::General(_) => Severity::Low,

            Self::CryptoError(_) | Self::CertificateError(_) | Self::InsecureProtocol(_) => {
                Severity::High
            }

            Self::LdapInjectionAttempt | Self::RegexDosAttempt => Severity::Critical,
        }
    }

    /// Check if the error should trigger an alert
    pub fn should_alert(&self) -> bool {
        matches!(self.severity(), Severity::Critical | Severity::High)
    }

    /// Get a sanitized error message for external display
    pub fn public_message(&self) -> String {
        match self {
            // Don't expose internal details for security errors
            Self::SqlInjectionAttempt
            | Self::XssAttempt
            | Self::CommandInjectionAttempt
            | Self::PathTraversalAttempt
            | Self::LdapInjectionAttempt
            | Self::RegexDosAttempt
            | Self::SuspiciousActivity(_) => "Request rejected due to security policy".to_string(),

            Self::RateLimitExceeded(_) => "Rate limit exceeded. Please try again later".to_string(),

            Self::AuthenticationError(_) => "Authentication failed".to_string(),

            Self::AuthorizationError(_) => "Access denied".to_string(),

            Self::InvalidSession(_) => "Session expired. Please log in again".to_string(),

            Self::WeakPassword(_) => "Password does not meet security requirements".to_string(),

            Self::RequestTooLarge(_) => "Request size too large".to_string(),

            // For less sensitive errors, use the actual message
            _ => self.to_string(),
        }
    }
}

/// Severity levels for security errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_severity() {
        assert_eq!(
            SecurityError::SqlInjectionAttempt.severity(),
            Severity::Critical
        );
        assert_eq!(
            SecurityError::RateLimitExceeded("test".to_string()).severity(),
            Severity::High
        );
        assert_eq!(
            SecurityError::ValidationError("test".to_string()).severity(),
            Severity::Medium
        );
    }

    #[test]
    fn test_should_alert() {
        assert!(SecurityError::SqlInjectionAttempt.should_alert());
        assert!(SecurityError::WeakPassword("test".to_string()).should_alert());
        assert!(!SecurityError::ValidationError("test".to_string()).should_alert());
    }

    #[test]
    fn test_public_message() {
        let err = SecurityError::SqlInjectionAttempt;
        assert_eq!(
            err.public_message(),
            "Request rejected due to security policy"
        );

        let err = SecurityError::ValidationError("internal details".to_string());
        assert!(err.public_message().contains("validation"));
    }
}
