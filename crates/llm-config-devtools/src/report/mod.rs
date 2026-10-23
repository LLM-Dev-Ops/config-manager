//! Report generation module.
//!
//! Provides multiple output formats for security scan reports including:
//! - JSON (machine-readable)
//! - YAML (human-readable structured)
//! - Markdown (documentation)
//! - SARIF (GitHub Security integration)

use crate::error::{DevtoolsError, Result};
use crate::security::SecurityReport;
use std::fs;
use std::path::Path;

pub mod json;
pub mod markdown;
pub mod sarif;
pub mod yaml;

/// Output format for reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// JSON format.
    Json,
    /// YAML format.
    Yaml,
    /// Markdown format.
    Markdown,
    /// SARIF format (for GitHub Security).
    Sarif,
}

impl std::str::FromStr for OutputFormat {
    type Err = DevtoolsError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "markdown" | "md" => Ok(Self::Markdown),
            "sarif" => Ok(Self::Sarif),
            _ => Err(DevtoolsError::Config(format!("Unknown output format: {s}"))),
        }
    }
}

/// Generate a report in the specified format.
///
/// # Errors
/// Returns an error when the selected formatter cannot serialize the report.
pub fn generate_report(report: &SecurityReport, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => json::generate(report),
        OutputFormat::Yaml => yaml::generate(report),
        OutputFormat::Markdown => markdown::generate(report),
        OutputFormat::Sarif => sarif::generate(report),
    }
}

/// Write a report to a file.
///
/// # Errors
/// Returns an error when formatting fails or the output file cannot be written.
pub fn write_report(report: &SecurityReport, format: OutputFormat, path: &Path) -> Result<()> {
    let content = generate_report(report, format)?;
    fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parsing() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("yaml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!("yml".parse::<OutputFormat>().unwrap(), OutputFormat::Yaml);
        assert_eq!(
            "markdown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            "md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            "sarif".parse::<OutputFormat>().unwrap(),
            OutputFormat::Sarif
        );
    }

    #[test]
    fn test_invalid_format() {
        assert!("invalid".parse::<OutputFormat>().is_err());
    }
}
