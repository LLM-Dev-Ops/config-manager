//! Output formatting for the Config Validation Agent CLI
//!
//! Provides structured output formatting in JSON, YAML, and human-readable table formats
//! with severity-based coloring for validation findings.

use clap::ValueEnum;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

use crate::error::ValidationError;
use crate::validation::{ValidationFinding, ValidationResult, ValidationSeverity};

/// Output format options for CLI results
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Default)]
pub enum OutputFormat {
    /// Human-readable table format with colors
    #[default]
    Table,
    /// JSON format for machine processing
    Json,
    /// YAML format for configuration output
    Yaml,
}

/// Validation output structure for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationOutput {
    /// Overall validation status
    pub valid: bool,
    /// Number of errors found
    pub error_count: usize,
    /// Number of warnings found
    pub warning_count: usize,
    /// Number of info findings
    pub info_count: usize,
    /// List of validation findings
    pub findings: Vec<FindingOutput>,
    /// Summary message
    pub summary: String,
    /// Validation duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Individual finding output structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingOutput {
    /// Severity level
    pub severity: String,
    /// Finding code/identifier
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Path in configuration where the finding occurred
    pub path: String,
    /// Suggested fix (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Related documentation link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_link: Option<String>,
}

impl ValidationOutput {
    /// Create output from a validation result
    pub fn from_result(result: &ValidationResult) -> Self {
        let error_count = result
            .findings
            .iter()
            .filter(|f| f.severity == ValidationSeverity::Error)
            .count();
        let warning_count = result
            .findings
            .iter()
            .filter(|f| f.severity == ValidationSeverity::Warning)
            .count();
        let info_count = result
            .findings
            .iter()
            .filter(|f| f.severity == ValidationSeverity::Info)
            .count();

        let valid = error_count == 0;

        let summary = if valid && warning_count == 0 {
            "Configuration is valid".to_string()
        } else if valid {
            format!(
                "Configuration is valid with {} warning(s)",
                warning_count
            )
        } else {
            format!(
                "Configuration has {} error(s) and {} warning(s)",
                error_count, warning_count
            )
        };

        let findings = result
            .findings
            .iter()
            .map(FindingOutput::from_finding)
            .collect();

        Self {
            valid,
            error_count,
            warning_count,
            info_count,
            findings,
            summary,
            duration_ms: result.duration_ms,
        }
    }

    /// Render output in the specified format
    pub fn render(&self, format: OutputFormat) -> Result<(), ValidationError> {
        match format {
            OutputFormat::Json => self.render_json(),
            OutputFormat::Yaml => self.render_yaml(),
            OutputFormat::Table => self.render_table(),
        }
    }

    /// Render as JSON
    fn render_json(&self) -> Result<(), ValidationError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
        println!("{}", json);
        Ok(())
    }

    /// Render as YAML
    fn render_yaml(&self) -> Result<(), ValidationError> {
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
        println!("{}", yaml);
        Ok(())
    }

    /// Render as human-readable table
    fn render_table(&self) -> Result<(), ValidationError> {
        let mut stdout = io::stdout();

        // Header
        writeln!(stdout).ok();
        writeln!(stdout, "{}", "Validation Results".cyan().bold()).ok();
        writeln!(stdout, "{}", "=".repeat(60)).ok();
        writeln!(stdout).ok();

        // Summary line
        let status_icon = if self.valid { "+" } else { "x" };
        let status_colored = if self.valid {
            status_icon.green()
        } else {
            status_icon.red()
        };
        writeln!(stdout, "{} {}", status_colored, self.summary).ok();
        writeln!(stdout).ok();

        // Statistics
        if self.error_count > 0 || self.warning_count > 0 || self.info_count > 0 {
            writeln!(stdout, "{}", "Statistics:".cyan().bold()).ok();
            if self.error_count > 0 {
                writeln!(
                    stdout,
                    "  {} Errors:   {}",
                    "x".red(),
                    self.error_count.to_string().red()
                )
                .ok();
            }
            if self.warning_count > 0 {
                writeln!(
                    stdout,
                    "  {} Warnings: {}",
                    "!".yellow(),
                    self.warning_count.to_string().yellow()
                )
                .ok();
            }
            if self.info_count > 0 {
                writeln!(
                    stdout,
                    "  {} Info:     {}",
                    "i".blue(),
                    self.info_count.to_string().blue()
                )
                .ok();
            }
            writeln!(stdout).ok();
        }

        // Findings
        if !self.findings.is_empty() {
            writeln!(stdout, "{}", "Findings:".cyan().bold()).ok();
            writeln!(stdout, "{}", "-".repeat(60)).ok();

            for (index, finding) in self.findings.iter().enumerate() {
                finding.render_table_row(&mut stdout, index + 1)?;
            }
        }

        // Duration
        if let Some(duration) = self.duration_ms {
            writeln!(stdout).ok();
            writeln!(stdout, "Completed in {} ms", duration.to_string().dimmed()).ok();
        }

        stdout.flush().ok();
        Ok(())
    }
}

impl FindingOutput {
    /// Create from a validation finding
    pub fn from_finding(finding: &ValidationFinding) -> Self {
        Self {
            severity: finding.severity.to_string(),
            code: finding.code.clone(),
            message: finding.message.clone(),
            path: finding.path.clone(),
            suggestion: finding.suggestion.clone(),
            doc_link: finding.doc_link.clone(),
        }
    }

    /// Render a single finding as a table row
    fn render_table_row(&self, stdout: &mut io::Stdout, index: usize) -> Result<(), ValidationError> {
        let severity_icon = match self.severity.to_lowercase().as_str() {
            "error" => "x".red(),
            "warning" => "!".yellow(),
            "info" => "i".blue(),
            _ => "-".white(),
        };

        let severity_label = match self.severity.to_lowercase().as_str() {
            "error" => "ERROR".red().bold(),
            "warning" => "WARNING".yellow().bold(),
            "info" => "INFO".blue().bold(),
            _ => self.severity.clone().white(),
        };

        writeln!(stdout).ok();
        writeln!(
            stdout,
            "{} [{}] {} {}",
            severity_icon,
            self.code.dimmed(),
            severity_label,
            self.message
        )
        .ok();
        writeln!(stdout, "  {} {}", "Path:".dimmed(), self.path.cyan()).ok();

        if let Some(suggestion) = &self.suggestion {
            writeln!(
                stdout,
                "  {} {}",
                "Fix:".dimmed(),
                suggestion.green()
            )
            .ok();
        }

        if let Some(doc_link) = &self.doc_link {
            writeln!(
                stdout,
                "  {} {}",
                "Docs:".dimmed(),
                doc_link.blue().underline()
            )
            .ok();
        }

        Ok(())
    }
}

/// Severity coloring utilities
pub struct SeverityColorizer;

impl SeverityColorizer {
    /// Get colored string for a severity level
    pub fn colorize(severity: &ValidationSeverity, text: &str) -> String {
        match severity {
            ValidationSeverity::Error => text.red().bold().to_string(),
            ValidationSeverity::Warning => text.yellow().bold().to_string(),
            ValidationSeverity::Info => text.blue().to_string(),
        }
    }

    /// Get the icon for a severity level
    pub fn icon(severity: &ValidationSeverity) -> String {
        match severity {
            ValidationSeverity::Error => "x".red().to_string(),
            ValidationSeverity::Warning => "!".yellow().to_string(),
            ValidationSeverity::Info => "i".blue().to_string(),
        }
    }

    /// Get the colored severity label
    pub fn label(severity: &ValidationSeverity) -> String {
        match severity {
            ValidationSeverity::Error => "ERROR".red().bold().to_string(),
            ValidationSeverity::Warning => "WARNING".yellow().bold().to_string(),
            ValidationSeverity::Info => "INFO".blue().to_string(),
        }
    }
}

/// Progress indicator for long-running operations
pub struct ProgressIndicator {
    message: String,
    started: bool,
}

impl ProgressIndicator {
    /// Create a new progress indicator
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            started: false,
        }
    }

    /// Start the progress indicator
    pub fn start(&mut self) {
        if !self.started {
            print!("{} {}... ", "->".blue(), self.message);
            io::stdout().flush().ok();
            self.started = true;
        }
    }

    /// Complete the progress with success
    pub fn success(&self) {
        if self.started {
            println!("{}", "done".green());
        }
    }

    /// Complete the progress with failure
    pub fn failure(&self, error: &str) {
        if self.started {
            println!("{} ({})", "failed".red(), error);
        }
    }
}

/// Format a file size in human-readable format
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Format a duration in human-readable format
pub fn format_duration(ms: u64) -> String {
    if ms >= 60000 {
        let minutes = ms / 60000;
        let seconds = (ms % 60000) / 1000;
        format!("{}m {}s", minutes, seconds)
    } else if ms >= 1000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_default() {
        assert_eq!(OutputFormat::default(), OutputFormat::Table);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.50s");
        assert_eq!(format_duration(65000), "1m 5s");
    }

    #[test]
    fn test_validation_output_valid() {
        let result = ValidationResult {
            valid: true,
            findings: vec![],
            duration_ms: Some(100),
        };
        let output = ValidationOutput::from_result(&result);
        assert!(output.valid);
        assert_eq!(output.error_count, 0);
        assert_eq!(output.warning_count, 0);
        assert_eq!(output.summary, "Configuration is valid");
    }

    #[test]
    fn test_finding_output_from_finding() {
        let finding = ValidationFinding {
            severity: ValidationSeverity::Error,
            code: "E001".to_string(),
            message: "Test error".to_string(),
            path: "$.config.key".to_string(),
            suggestion: Some("Fix this".to_string()),
            doc_link: None,
        };
        let output = FindingOutput::from_finding(&finding);
        assert_eq!(output.severity, "error");
        assert_eq!(output.code, "E001");
        assert_eq!(output.message, "Test error");
        assert_eq!(output.suggestion, Some("Fix this".to_string()));
    }
}
