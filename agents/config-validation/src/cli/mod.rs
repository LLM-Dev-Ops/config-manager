//! CLI module for the Config Validation Agent
//!
//! This module provides command-line interface functionality for validating
//! configurations against schemas, inspecting configuration structures,
//! and checking cross-agent compatibility.

pub mod commands;
pub mod output;

pub use commands::{ValidateCli, ValidateCommands};
pub use output::{OutputFormat, ValidationOutput};

use crate::error::ValidationError;

/// Exit codes for CLI operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Successful execution, all validations passed
    Success = 0,
    /// Validation failed with errors
    ValidationError = 1,
    /// Validation passed with warnings
    ValidationWarning = 2,
    /// Invalid input or arguments
    InvalidInput = 3,
    /// File not found or inaccessible
    FileError = 4,
    /// Schema-related errors
    SchemaError = 5,
    /// Internal error
    InternalError = 10,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code as i32
    }
}

impl ExitCode {
    /// Determine exit code from validation result
    pub fn from_validation_result(has_errors: bool, has_warnings: bool) -> Self {
        if has_errors {
            ExitCode::ValidationError
        } else if has_warnings {
            ExitCode::ValidationWarning
        } else {
            ExitCode::Success
        }
    }
}

/// Run the CLI with the given arguments and return the exit code
pub fn run(cli: ValidateCli) -> Result<ExitCode, ValidationError> {
    match cli.command {
        ValidateCommands::Validate {
            config,
            schema,
            environment,
            format,
            strict,
        } => {
            commands::execute_validate(config, schema, environment, format, strict)
        }
        ValidateCommands::Inspect { config, format } => {
            commands::execute_inspect(config, format)
        }
        ValidateCommands::Compatibility { configs, format } => {
            commands::execute_compatibility(configs, format)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_conversion() {
        assert_eq!(i32::from(ExitCode::Success), 0);
        assert_eq!(i32::from(ExitCode::ValidationError), 1);
        assert_eq!(i32::from(ExitCode::ValidationWarning), 2);
    }

    #[test]
    fn test_exit_code_from_validation_result() {
        assert_eq!(
            ExitCode::from_validation_result(false, false),
            ExitCode::Success
        );
        assert_eq!(
            ExitCode::from_validation_result(true, false),
            ExitCode::ValidationError
        );
        assert_eq!(
            ExitCode::from_validation_result(false, true),
            ExitCode::ValidationWarning
        );
        assert_eq!(
            ExitCode::from_validation_result(true, true),
            ExitCode::ValidationError
        );
    }
}
