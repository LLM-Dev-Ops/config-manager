//! JSON report generator.

use crate::error::Result;
use crate::security::SecurityReport;

/// Generate a JSON report.
///
/// # Errors
/// Returns an error when the report cannot be serialized as JSON.
pub fn generate(report: &SecurityReport) -> Result<String> {
    let json = serde_json::to_string_pretty(report)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{Finding, Severity};
    use std::path::PathBuf;

    #[test]
    fn test_json_generation() {
        let report = SecurityReport::new(PathBuf::from("."), vec![]);
        let json = generate(&report).unwrap();
        assert!(json.contains("timestamp"));
        assert!(json.contains("findings"));
    }

    #[test]
    fn test_json_with_findings() {
        let findings = vec![Finding {
            severity: Severity::High,
            category: "test".to_string(),
            title: "Test Finding".to_string(),
            message: "Test message".to_string(),
            file: PathBuf::from("test.rs"),
            line: Some(1),
            column: None,
            snippet: None,
            recommendation: None,
        }];

        let report = SecurityReport::new(PathBuf::from("."), findings);
        let json = generate(&report).unwrap();
        assert!(json.contains("Test Finding"));
        assert!(json.contains("high")); // Severity is serialized as lowercase
    }
}
