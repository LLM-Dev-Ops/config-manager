//! Error types for the devtools crate.
//!
//! This module provides comprehensive error handling for all scanning operations.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for devtools operations.
pub type Result<T> = std::result::Result<T, DevtoolsError>;

/// Main error type for devtools operations.
#[derive(Error, Debug)]
pub enum DevtoolsError {
    /// IO error occurred during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Cargo command execution failed.
    #[error("Cargo command failed: {0}")]
    CargoExecution(String),

    /// Failed to parse cargo metadata.
    #[error("Failed to parse cargo metadata: {0}")]
    CargoMetadata(#[from] cargo_metadata::Error),

    /// Clippy execution failed.
    #[error("Clippy execution failed: {0}")]
    ClippyFailed(String),

    /// Pattern compilation failed.
    #[error("Pattern compilation failed: {0}")]
    PatternError(#[from] regex::Error),

    /// Project root not found or invalid.
    #[error("Invalid project root: {path}")]
    InvalidProjectRoot {
        /// The invalid path
        path: PathBuf,
    },

    /// No Cargo.toml found in project.
    #[error("No Cargo.toml found at: {path}")]
    NoCargoToml {
        /// The path where Cargo.toml was expected
        path: PathBuf,
    },

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Report generation failed.
    #[error("Report generation failed: {0}")]
    ReportGeneration(String),

    /// Git repository error.
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// No findings to report.
    #[error("No security findings to report")]
    NoFindings,

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Ignore/walkdir error.
    #[error("File walking error: {0}")]
    Ignore(String),

    /// Other error.
    #[error("{0}")]
    Other(String),
}

impl From<ignore::Error> for DevtoolsError {
    fn from(err: ignore::Error) -> Self {
        Self::Ignore(err.to_string())
    }
}

impl From<serde_json::Error> for DevtoolsError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for DevtoolsError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = DevtoolsError::CargoExecution("test error".to_string());
        assert_eq!(err.to_string(), "Cargo command failed: test error");
    }

    #[test]
    fn test_invalid_project_root() {
        let err = DevtoolsError::InvalidProjectRoot {
            path: PathBuf::from("/invalid/path"),
        };
        assert!(err.to_string().contains("/invalid/path"));
    }
}
