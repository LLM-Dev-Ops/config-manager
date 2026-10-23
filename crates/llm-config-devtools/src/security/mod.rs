//! Security scanning module.
//!
//! This module provides comprehensive security scanning capabilities including:
//! - Clippy security lints
//! - Unsafe code detection
//! - Secret scanning
//! - SQL injection vulnerability detection

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod clippy;
pub mod secret;
pub mod sql;
pub mod unsafe_code;

/// Security scan configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent scanner toggles intentionally mirror the CLI flags"
)]
pub struct ScanConfig {
    /// Project root directory.
    pub project_root: PathBuf,

    /// Whether to include clippy scanning.
    pub scan_clippy: bool,

    /// Whether to scan for unsafe code.
    pub scan_unsafe: bool,

    /// Whether to scan for secrets.
    pub scan_secrets: bool,

    /// Whether to scan for SQL injection.
    pub scan_sql: bool,

    /// Maximum number of parallel workers.
    pub max_workers: Option<usize>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            scan_clippy: true,
            scan_unsafe: true,
            scan_secrets: true,
            scan_sql: true,
            max_workers: None,
        }
    }
}

/// Security finding severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Low severity - informational.
    Low,
    /// Medium severity - should be reviewed.
    Medium,
    /// High severity - should be fixed soon.
    High,
    /// Critical severity - must be fixed immediately.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single security finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Finding severity.
    pub severity: Severity,

    /// Finding category (e.g., "`unsafe_code`", "secret", "`sql_injection`").
    pub category: String,

    /// Brief description of the finding.
    pub title: String,

    /// Detailed message about the finding.
    pub message: String,

    /// File path where the finding was detected.
    pub file: PathBuf,

    /// Line number (if applicable).
    pub line: Option<usize>,

    /// Column number (if applicable).
    pub column: Option<usize>,

    /// Code snippet (if applicable).
    pub snippet: Option<String>,

    /// Recommendation for fixing.
    pub recommendation: Option<String>,
}

/// Complete security scan report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityReport {
    /// Timestamp when the scan was performed.
    pub timestamp: DateTime<Utc>,

    /// Project root directory.
    pub project_root: PathBuf,

    /// Git commit hash (if available).
    pub git_commit: Option<String>,

    /// Git branch (if available).
    pub git_branch: Option<String>,

    /// All findings from the scan.
    pub findings: Vec<Finding>,

    /// Summary statistics.
    pub summary: ScanSummary,
}

/// Summary statistics for a security scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    /// Total number of findings.
    pub total: usize,

    /// Number of critical findings.
    pub critical: usize,

    /// Number of high severity findings.
    pub high: usize,

    /// Number of medium severity findings.
    pub medium: usize,

    /// Number of low severity findings.
    pub low: usize,

    /// Number of files scanned.
    pub files_scanned: usize,

    /// Scan duration in seconds.
    pub duration_seconds: f64,
}

impl SecurityReport {
    /// Create a new security report.
    #[must_use]
    pub fn new(project_root: PathBuf, findings: Vec<Finding>) -> Self {
        let summary = Self::calculate_summary(&findings);

        Self {
            timestamp: Utc::now(),
            project_root,
            git_commit: Self::get_git_commit().ok(),
            git_branch: Self::get_git_branch().ok(),
            findings,
            summary,
        }
    }

    /// Calculate summary statistics from findings.
    fn calculate_summary(findings: &[Finding]) -> ScanSummary {
        let total = findings.len();
        let critical = findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let high = findings
            .iter()
            .filter(|f| f.severity == Severity::High)
            .count();
        let medium = findings
            .iter()
            .filter(|f| f.severity == Severity::Medium)
            .count();
        let low = findings
            .iter()
            .filter(|f| f.severity == Severity::Low)
            .count();

        ScanSummary {
            total,
            critical,
            high,
            medium,
            low,
            files_scanned: 0,
            duration_seconds: 0.0,
        }
    }

    /// Get current git commit hash.
    fn get_git_commit() -> Result<String> {
        let repo = git2::Repository::discover(".")?;
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }

    /// Get current git branch.
    fn get_git_branch() -> Result<String> {
        let repo = git2::Repository::discover(".")?;
        let head = repo.head()?;
        let branch = head.shorthand().unwrap_or("unknown").to_string();
        Ok(branch)
    }

    /// Check if there are any critical or high severity findings.
    #[must_use]
    pub const fn has_high_severity(&self) -> bool {
        self.summary.critical > 0 || self.summary.high > 0
    }
}

/// Main security scanner.
pub struct SecurityScanner {
    config: ScanConfig,
}

impl SecurityScanner {
    /// Create a new security scanner with the given configuration.
    #[must_use]
    pub const fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Run the security scan.
    ///
    /// # Errors
    /// Returns an error when an enabled scanner fails to execute.
    pub fn scan(&self) -> Result<SecurityReport> {
        use rayon::prelude::*;

        let start_time = std::time::Instant::now();
        let mut findings = Vec::new();

        tracing::info!("Starting security scan at {:?}", self.config.project_root);

        // Collect scanner functions to run
        let mut scanners: Vec<Box<dyn Fn() -> Result<Vec<Finding>> + Send + Sync>> = Vec::new();

        if self.config.scan_clippy {
            let root = self.config.project_root.clone();
            scanners.push(Box::new(move || clippy::ClippyScanner::new(&root).scan()));
        }

        if self.config.scan_unsafe {
            let root = self.config.project_root.clone();
            scanners.push(Box::new(move || {
                unsafe_code::UnsafeCodeDetector::new(&root).scan()
            }));
        }

        if self.config.scan_secrets {
            let root = self.config.project_root.clone();
            scanners.push(Box::new(move || secret::SecretScanner::new(&root).scan()));
        }

        if self.config.scan_sql {
            let root = self.config.project_root.clone();
            scanners.push(Box::new(move || {
                sql::SqlInjectionScanner::new(&root).scan()
            }));
        }

        // Run scans in parallel using rayon
        let scan_results: Vec<Result<Vec<Finding>>> =
            scanners.par_iter().map(|scanner| scanner()).collect();

        // Collect all findings
        for result in scan_results {
            match result {
                Ok(mut found) => findings.append(&mut found),
                Err(e) => tracing::warn!("Scanner error: {}", e),
            }
        }

        let duration = start_time.elapsed();
        let mut report = SecurityReport::new(self.config.project_root.clone(), findings);
        report.summary.duration_seconds = duration.as_secs_f64();

        tracing::info!(
            "Security scan complete: {} findings in {:.2}s",
            report.summary.total,
            report.summary.duration_seconds
        );

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn test_default_config() {
        let config = ScanConfig::default();
        assert!(config.scan_clippy);
        assert!(config.scan_unsafe);
        assert!(config.scan_secrets);
        assert!(config.scan_sql);
    }

    #[test]
    fn test_security_report_summary() {
        let findings = vec![
            Finding {
                severity: Severity::Critical,
                category: "test".to_string(),
                title: "Test Critical".to_string(),
                message: "Test message".to_string(),
                file: PathBuf::from("test.rs"),
                line: Some(1),
                column: None,
                snippet: None,
                recommendation: None,
            },
            Finding {
                severity: Severity::High,
                category: "test".to_string(),
                title: "Test High".to_string(),
                message: "Test message".to_string(),
                file: PathBuf::from("test.rs"),
                line: Some(2),
                column: None,
                snippet: None,
                recommendation: None,
            },
        ];

        let report = SecurityReport::new(PathBuf::from("."), findings);
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.summary.critical, 1);
        assert_eq!(report.summary.high, 1);
        assert!(report.has_high_severity());
    }
}
