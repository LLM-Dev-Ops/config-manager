//! Clippy security lints scanner.
//!
//! Runs cargo clippy with security-focused lints and parses the output.

use crate::error::{DevtoolsError, Result};
use crate::security::{Finding, Severity};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Scanner for running clippy with security lints.
pub struct ClippyScanner {
    project_root: PathBuf,
}

impl ClippyScanner {
    /// Create a new clippy scanner.
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// Run clippy security scan.
    ///
    /// # Errors
    /// Returns an error when the Clippy process cannot be started.
    pub fn scan(&self) -> Result<Vec<Finding>> {
        tracing::info!("Running clippy security lints");

        // Check if clippy is available
        if !Self::is_clippy_available() {
            tracing::warn!("Clippy not available, skipping scan");
            return Ok(Vec::new());
        }

        let output = Command::new("cargo")
            .arg("clippy")
            .arg("--all-targets")
            .arg("--all-features")
            .arg("--message-format=json")
            .arg("--")
            .arg("-W")
            .arg("clippy::all")
            .arg("-W")
            .arg("clippy::pedantic")
            .arg("-W")
            .arg("clippy::nursery")
            .arg("-W")
            .arg("clippy::cargo")
            .arg("-W")
            .arg("unsafe-code")
            .arg("-W")
            .arg("missing-docs")
            .arg("-A")
            .arg("clippy::module-name-repetitions")
            .arg("-A")
            .arg("clippy::missing-errors-doc")
            .arg("-A")
            .arg("clippy::missing-panics-doc")
            .current_dir(&self.project_root)
            .output()
            .map_err(|e| DevtoolsError::CargoExecution(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Clippy exited with non-zero status: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_clippy_output(&stdout))
    }

    fn is_clippy_available() -> bool {
        Command::new("cargo")
            .arg("clippy")
            .arg("--version")
            .output()
            .is_ok()
    }

    fn parse_clippy_output(output: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        for line in output.lines() {
            if let Ok(message) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(msg) = message.get("message") {
                    if let Some(level) = msg.get("level").and_then(|l| l.as_str()) {
                        if level == "warning" || level == "error" {
                            if let Some(finding) = Self::parse_clippy_message(msg) {
                                findings.push(finding);
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("Found {} clippy warnings/errors", findings.len());
        findings
    }

    fn parse_clippy_message(msg: &serde_json::Value) -> Option<Finding> {
        let message = msg.get("message")?.as_str()?.to_string();
        let code = msg
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("unknown");

        let spans = msg.get("spans")?.as_array()?;
        let span = spans.first()?;

        let file = span
            .get("file_name")
            .and_then(|f| f.as_str())
            .map(PathBuf::from)?;
        let line = span
            .get("line_start")
            .and_then(serde_json::Value::as_u64)
            .and_then(|line| usize::try_from(line).ok());
        let column = span
            .get("column_start")
            .and_then(serde_json::Value::as_u64)
            .and_then(|column| usize::try_from(column).ok());

        let snippet = span
            .get("text")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
            .map(std::string::ToString::to_string);

        let severity = if code.contains("unsafe") || code.contains("security") {
            Severity::High
        } else if code.contains("clippy::pedantic") || code.contains("clippy::nursery") {
            Severity::Medium
        } else {
            Severity::Low
        };

        Some(Finding {
            severity,
            category: "clippy".to_string(),
            title: format!("Clippy: {code}"),
            message,
            file,
            line,
            column,
            snippet,
            recommendation: Some(
                "Run 'cargo clippy --fix' to automatically fix some issues".to_string(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clippy_scanner_creation() {
        let scanner = ClippyScanner::new(Path::new("."));
        assert_eq!(scanner.project_root, PathBuf::from("."));
    }

    #[test]
    fn test_clippy_available() {
        // This might be false in CI without Rust toolchain
        let _ = ClippyScanner::is_clippy_available();
    }
}
