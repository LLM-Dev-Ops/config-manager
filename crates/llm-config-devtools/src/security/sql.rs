//! SQL injection vulnerability scanner.
//!
//! Scans for potential SQL injection vulnerabilities in Rust code.

use crate::error::Result;
use crate::security::{Finding, Severity};
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

lazy_static! {
    /// Regex patterns for detecting SQL injection vulnerabilities.
    static ref SQL_PATTERNS: Vec<SqlPattern> = vec![
        SqlPattern {
            name: "format! with SELECT",
            regex: Regex::new(r#"format!\s*\(\s*["'].*SELECT.*\{.*\}.*["']"#).unwrap(),
            severity: Severity::Critical,
        },
        SqlPattern {
            name: "format! with INSERT",
            regex: Regex::new(r#"format!\s*\(\s*["'].*INSERT.*\{.*\}.*["']"#).unwrap(),
            severity: Severity::Critical,
        },
        SqlPattern {
            name: "format! with UPDATE",
            regex: Regex::new(r#"format!\s*\(\s*["'].*UPDATE.*\{.*\}.*["']"#).unwrap(),
            severity: Severity::Critical,
        },
        SqlPattern {
            name: "format! with DELETE",
            regex: Regex::new(r#"format!\s*\(\s*["'].*DELETE.*\{.*\}.*["']"#).unwrap(),
            severity: Severity::Critical,
        },
        SqlPattern {
            name: "String concatenation with SELECT",
            regex: Regex::new(r"SELECT.*\+.*").unwrap(),
            severity: Severity::High,
        },
        SqlPattern {
            name: "String concatenation with INSERT",
            regex: Regex::new(r"INSERT.*\+.*").unwrap(),
            severity: Severity::High,
        },
        SqlPattern {
            name: "String interpolation with sql",
            regex: Regex::new(r#"&format!\s*\(\s*["'].*sql.*\{.*\}"#).unwrap(),
            severity: Severity::High,
        },
    ];
}

/// A pattern for detecting SQL injection vulnerabilities.
struct SqlPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
}

/// Scanner for detecting SQL injection vulnerabilities.
pub struct SqlInjectionScanner {
    project_root: PathBuf,
}

impl SqlInjectionScanner {
    /// Create a new SQL injection scanner.
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// Scan for SQL injection vulnerabilities.
    ///
    /// # Errors
    /// Returns an error when the project tree cannot be traversed.
    pub fn scan(&self) -> Result<Vec<Finding>> {
        tracing::info!("Scanning for SQL injection vulnerabilities");

        let mut findings = Vec::new();

        // Walk through all Rust source files
        for entry in WalkBuilder::new(&self.project_root)
            .filter_entry(|e| !Self::is_excluded(e.path()))
            .build()
        {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(content) = fs::read_to_string(path) {
                    findings.extend(self.scan_file(path, &content));
                }
            }
        }

        tracing::info!("Found {} potential SQL injection risks", findings.len());
        Ok(findings)
    }

    fn is_excluded(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.contains("target/")
            || path_str.contains("node_modules/")
            || path_str.contains("crates/llm-config-devtools/src/security/")
    }

    fn scan_file(&self, path: &Path, content: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip comments
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            for pattern in SQL_PATTERNS.iter() {
                if pattern.regex.is_match(line) {
                    let relative_path = path
                        .strip_prefix(&self.project_root)
                        .unwrap_or(path)
                        .to_path_buf();

                    findings.push(Finding {
                        severity: pattern.severity,
                        category: "sql_injection".to_string(),
                        title: format!("Potential SQL injection: {}", pattern.name),
                        message: format!(
                            "Found {} which may be vulnerable to SQL injection. Use parameterized queries instead.",
                            pattern.name
                        ),
                        file: relative_path,
                        line: Some(line_num + 1),
                        column: None,
                        snippet: Some(line.trim().to_string()),
                        recommendation: Some(
                            "Use parameterized queries or a query builder that prevents SQL injection (e.g., sqlx with compile-time checked queries, or diesel ORM).".to_string(),
                        ),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sql_injection_scanner_detects_format() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("db.rs");
        fs::write(
            &file_path,
            r#"
fn bad_query(user_input: &str) -> String {
    format!("SELECT * FROM users WHERE name = '{}'", user_input)
}
"#,
        )
        .unwrap();

        let scanner = SqlInjectionScanner::new(dir.path());
        let findings = scanner.scan().unwrap();

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.category == "sql_injection"));
        assert!(findings.iter().any(|f| f.severity == Severity::Critical));
    }

    #[test]
    fn test_sql_injection_scanner_safe_query() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("db.rs");
        fs::write(
            &file_path,
            r#"
fn safe_query(pool: &Pool) -> Result<Vec<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
}
"#,
        )
        .unwrap();

        let scanner = SqlInjectionScanner::new(dir.path());
        let findings = scanner.scan().unwrap();

        // Should not flag parameterized queries
        assert_eq!(findings.len(), 0);
    }
}
