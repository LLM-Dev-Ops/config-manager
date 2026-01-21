//! Error types for the Config Validation Agent
//!
//! Provides structured error types for validation, parsing, and I/O operations.

use thiserror::Error;

/// Main error type for validation operations
#[derive(Error, Debug)]
pub enum ValidationError {
    /// Invalid input data or arguments
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// File access or I/O error
    #[error("File error: {0}")]
    FileError(String),

    /// Configuration parsing error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Schema-related error
    #[error("Schema error: {0}")]
    SchemaError(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Validation rule violation
    #[error("Validation failed: {0}")]
    RuleViolation(String),

    /// Compatibility check failure
    #[error("Compatibility error: {0}")]
    CompatibilityError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl ValidationError {
    /// Create an invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        ValidationError::InvalidInput(msg.into())
    }

    /// Create a file error
    pub fn file_error(msg: impl Into<String>) -> Self {
        ValidationError::FileError(msg.into())
    }

    /// Create a parse error
    pub fn parse_error(msg: impl Into<String>) -> Self {
        ValidationError::ParseError(msg.into())
    }

    /// Create a schema error
    pub fn schema_error(msg: impl Into<String>) -> Self {
        ValidationError::SchemaError(msg.into())
    }

    /// Check if this is a user-facing error (vs internal)
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            ValidationError::InvalidInput(_)
                | ValidationError::FileError(_)
                | ValidationError::ParseError(_)
                | ValidationError::SchemaError(_)
        )
    }
}

impl From<std::io::Error> for ValidationError {
    fn from(err: std::io::Error) -> Self {
        ValidationError::FileError(err.to_string())
    }
}

impl From<serde_json::Error> for ValidationError {
    fn from(err: serde_json::Error) -> Self {
        ValidationError::ParseError(format!("JSON error: {}", err))
    }
}

impl From<serde_yaml::Error> for ValidationError {
    fn from(err: serde_yaml::Error) -> Self {
        ValidationError::ParseError(format!("YAML error: {}", err))
    }
}

impl From<toml::de::Error> for ValidationError {
    fn from(err: toml::de::Error) -> Self {
        ValidationError::ParseError(format!("TOML error: {}", err))
    }
}

/// Result type alias for validation operations
pub type Result<T> = std::result::Result<T, ValidationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ValidationError::InvalidInput("test error".to_string());
        assert_eq!(err.to_string(), "Invalid input: test error");
    }

    #[test]
    fn test_is_user_error() {
        assert!(ValidationError::InvalidInput("test".to_string()).is_user_error());
        assert!(ValidationError::FileError("test".to_string()).is_user_error());
        assert!(!ValidationError::InternalError("test".to_string()).is_user_error());
    }

    #[test]
    fn test_error_constructors() {
        let err = ValidationError::invalid_input("test");
        assert!(matches!(err, ValidationError::InvalidInput(_)));

        let err = ValidationError::file_error("test");
        assert!(matches!(err, ValidationError::FileError(_)));

        let err = ValidationError::parse_error("test");
        assert!(matches!(err, ValidationError::ParseError(_)));
    }
}
