//! SARIF (Static Analysis Results Interchange Format) report generator.
//!
//! SARIF is the format used by GitHub Security tab to display security findings.
//! Specification: <https://sarifweb.azurewebsites.net>/

use crate::error::Result;
use crate::security::{SecurityReport, Severity};
use serde_json::json;

/// Generate a SARIF report.
///
/// # Errors
/// Returns an error when the SARIF document cannot be serialized.
pub fn generate(report: &SecurityReport) -> Result<String> {
    let mut results = Vec::new();

    for finding in &report.findings {
        let rule_id = format!(
            "{}_{}",
            finding.category,
            finding.severity.to_string().to_lowercase()
        );

        let level = match finding.severity {
            Severity::Critical | Severity::High => "error",
            Severity::Medium => "warning",
            Severity::Low => "note",
        };

        let mut locations = Vec::new();

        if let Some(line) = finding.line {
            let location = json!({
                "physicalLocation": {
                    "artifactLocation": {
                        "uri": finding.file.display().to_string(),
                    },
                    "region": {
                        "startLine": line,
                        "startColumn": finding.column.unwrap_or(1),
                    }
                }
            });
            locations.push(location);
        }

        let result = json!({
            "ruleId": rule_id,
            "level": level,
            "message": {
                "text": finding.message.clone(),
            },
            "locations": locations,
        });

        results.push(result);
    }

    let sarif = json!({
        "version": "2.1.0",
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "llm-config-devtools",
                        "informationUri": "https://github.com/globalbusinessadvisors/llm-config-manager",
                        "version": env!("CARGO_PKG_VERSION"),
                        "rules": [
                            {
                                "id": "unsafe_code",
                                "name": "UnsafeCode",
                                "shortDescription": {
                                    "text": "Unsafe code block detected"
                                },
                                "fullDescription": {
                                    "text": "Unsafe code requires careful review to ensure memory safety"
                                },
                                "helpUri": "https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html"
                            },
                            {
                                "id": "secret",
                                "name": "HardcodedSecret",
                                "shortDescription": {
                                    "text": "Potential hardcoded secret"
                                },
                                "fullDescription": {
                                    "text": "Hardcoded secrets should never be committed to version control"
                                },
                                "helpUri": "https://owasp.org/www-community/vulnerabilities/Use_of_hard-coded_password"
                            },
                            {
                                "id": "sql_injection",
                                "name": "SqlInjection",
                                "shortDescription": {
                                    "text": "Potential SQL injection vulnerability"
                                },
                                "fullDescription": {
                                    "text": "SQL queries constructed with string concatenation may be vulnerable to injection attacks"
                                },
                                "helpUri": "https://owasp.org/www-community/attacks/SQL_Injection"
                            }
                        ]
                    }
                },
                "results": results,
                "properties": {
                    "timestamp": report.timestamp.to_rfc3339(),
                    "projectRoot": report.project_root.display().to_string(),
                }
            }
        ]
    });

    let json = serde_json::to_string_pretty(&sarif)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::Finding;
    use std::path::PathBuf;

    #[test]
    fn test_sarif_generation() {
        let findings = vec![Finding {
            severity: Severity::High,
            category: "test".to_string(),
            title: "Test Finding".to_string(),
            message: "Test message".to_string(),
            file: PathBuf::from("test.rs"),
            line: Some(42),
            column: Some(10),
            snippet: None,
            recommendation: None,
        }];

        let report = SecurityReport::new(PathBuf::from("."), findings);
        let sarif = generate(&report).unwrap();

        assert!(sarif.contains("version"));
        assert!(sarif.contains("2.1.0"));
        assert!(sarif.contains("llm-config-devtools"));
        assert!(sarif.contains("test.rs"));
        assert!(sarif.contains("\"startLine\": 42"));
    }

    #[test]
    fn test_sarif_empty_report() {
        let report = SecurityReport::new(PathBuf::from("."), vec![]);
        let sarif = generate(&report).unwrap();

        // Should still be valid SARIF with empty results
        assert!(sarif.contains("version"));
        assert!(sarif.contains("results"));
        let parsed: serde_json::Value = serde_json::from_str(&sarif).unwrap();
        assert_eq!(parsed["runs"][0]["results"].as_array().unwrap().len(), 0);
    }
}
