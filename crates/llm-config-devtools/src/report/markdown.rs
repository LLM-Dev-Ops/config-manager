//! Markdown report generator.

use crate::error::Result;
use crate::security::{SecurityReport, Severity};
use std::fmt::Write as _;

/// Generate a Markdown report.
///
/// # Errors
/// Returns an error if report generation cannot be completed.
pub fn generate(report: &SecurityReport) -> Result<String> {
    let mut md = String::new();

    // Title
    md.push_str("# Security Scan Report\n\n");

    // Metadata
    let _ = writeln!(
        md,
        "**Generated**: {}",
        report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    let _ = writeln!(md, "**Project**: {}", report.project_root.display());

    if let Some(commit) = &report.git_commit {
        let _ = writeln!(md, "**Commit**: `{commit}`");
    }
    if let Some(branch) = &report.git_branch {
        let _ = writeln!(md, "**Branch**: `{branch}`");
    }

    md.push_str("\n---\n\n");

    // Summary
    md.push_str("## Summary\n\n");
    let _ = writeln!(md, "- **Total Findings**: {}", report.summary.total);
    let _ = writeln!(md, "- **Critical**: {} 🔴", report.summary.critical);
    let _ = writeln!(md, "- **High**: {} 🟠", report.summary.high);
    let _ = writeln!(md, "- **Medium**: {} 🟡", report.summary.medium);
    let _ = writeln!(md, "- **Low**: {} 🟢", report.summary.low);
    let _ = writeln!(
        md,
        "- **Scan Duration**: {:.2}s",
        report.summary.duration_seconds
    );

    md.push('\n');

    // Status
    if report.summary.total == 0 {
        md.push_str("✅ **Status**: No security issues found!\n\n");
    } else if report.summary.critical > 0 {
        md.push_str("🔴 **Status**: CRITICAL issues found - immediate action required\n\n");
    } else if report.summary.high > 0 {
        md.push_str("🟠 **Status**: HIGH severity issues found - fix soon\n\n");
    } else {
        md.push_str("🟡 **Status**: Issues found - review recommended\n\n");
    }

    if report.findings.is_empty() {
        return Ok(md);
    }

    md.push_str("---\n\n");

    // Findings by severity
    for severity in [
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
    ] {
        let severity_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .collect();

        if severity_findings.is_empty() {
            continue;
        }

        let emoji = match severity {
            Severity::Critical => "🔴",
            Severity::High => "🟠",
            Severity::Medium => "🟡",
            Severity::Low => "🟢",
        };

        let _ = write!(md, "## {emoji} {severity} Severity Findings\n\n");

        for (i, finding) in severity_findings.iter().enumerate() {
            let _ = write!(md, "### {}.{} {}\n\n", severity, i + 1, finding.title);

            let _ = writeln!(md, "- **Category**: `{}`", finding.category);
            let _ = writeln!(md, "- **Severity**: {severity}");
            let _ = writeln!(md, "- **File**: `{}`", finding.file.display());

            if let Some(line) = finding.line {
                let _ = writeln!(md, "- **Line**: {line}");
            }

            let _ = write!(md, "\n**Message**: {}\n\n", finding.message);

            if let Some(snippet) = &finding.snippet {
                md.push_str("**Code**:\n");
                md.push_str("```rust\n");
                md.push_str(snippet);
                md.push_str("\n```\n\n");
            }

            if let Some(recommendation) = &finding.recommendation {
                let _ = write!(md, "**Recommendation**: {recommendation}\n\n");
            }

            md.push_str("---\n\n");
        }
    }

    // Recommendations
    md.push_str("## General Recommendations\n\n");
    md.push_str("1. Address all Critical and High severity findings immediately\n");
    md.push_str("2. Review and plan fixes for Medium severity findings\n");
    md.push_str("3. Consider fixing Low severity findings during refactoring\n");
    md.push_str("4. Re-run security scan after fixes to verify resolution\n");
    md.push_str("5. Integrate security scanning into CI/CD pipeline\n\n");

    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::Finding;
    use std::path::PathBuf;

    #[test]
    fn test_markdown_generation_empty() {
        let report = SecurityReport::new(PathBuf::from("."), vec![]);
        let md = generate(&report).unwrap();
        assert!(md.contains("# Security Scan Report"));
        assert!(md.contains("No security issues found"));
    }

    #[test]
    fn test_markdown_generation_with_findings() {
        let findings = vec![Finding {
            severity: Severity::Critical,
            category: "test".to_string(),
            title: "Test Finding".to_string(),
            message: "Test message".to_string(),
            file: PathBuf::from("test.rs"),
            line: Some(42),
            column: None,
            snippet: Some("let x = unsafe { ... };".to_string()),
            recommendation: Some("Fix this immediately".to_string()),
        }];

        let report = SecurityReport::new(PathBuf::from("."), findings);
        let md = generate(&report).unwrap();

        assert!(md.contains("Test Finding"));
        assert!(md.contains("test.rs"));
        assert!(md.contains("Line**: 42"));
        assert!(md.contains("Fix this immediately"));
    }
}
