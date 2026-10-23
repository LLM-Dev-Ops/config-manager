//! Unsafe code detection scanner.
//!
//! Scans Rust source files for unsafe code blocks and provides analysis.

use crate::error::Result;
use crate::security::{Finding, Severity};
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

/// Scanner for detecting unsafe code blocks.
pub struct UnsafeCodeDetector {
    project_root: PathBuf,
}

impl UnsafeCodeDetector {
    /// Create a new unsafe code detector.
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// Scan for unsafe code blocks.
    ///
    /// # Errors
    /// Returns an error when the project tree cannot be traversed.
    pub fn scan(&self) -> Result<Vec<Finding>> {
        tracing::info!("Scanning for unsafe code blocks");

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

        tracing::info!("Found {} unsafe code blocks", findings.len());
        Ok(findings)
    }

    fn is_excluded(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.contains("target/") || path_str.contains("node_modules/")
    }

    fn scan_file(&self, path: &Path, content: &str) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.trim_start().starts_with("unsafe") {
                let relative_path = path
                    .strip_prefix(&self.project_root)
                    .unwrap_or(path)
                    .to_path_buf();

                findings.push(Finding {
                    severity: Severity::Medium,
                    category: "unsafe_code".to_string(),
                    title: "Unsafe code block detected".to_string(),
                    message: "Found unsafe code block. Ensure proper safety invariants are maintained.".to_string(),
                    file: relative_path,
                    line: Some(line_num + 1),
                    column: None,
                    snippet: Some(line.trim().to_string()),
                    recommendation: Some(
                        "Review unsafe code for correctness. Add safety documentation comments explaining why the unsafe code is sound.".to_string(),
                    ),
                });
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
    fn test_unsafe_code_detector() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(
            &file_path,
            r#"
fn safe_function() {
    println!("Safe");
}

unsafe fn unsafe_function() {
    // Unsafe operation
}

fn another_safe() {
    unsafe {
        // Unsafe block
    }
}
"#,
        )
        .unwrap();

        let detector = UnsafeCodeDetector::new(dir.path());
        let findings = detector.scan().unwrap();

        // Should find 2 unsafe blocks (function and block)
        assert!(findings.len() >= 2);
        assert!(findings.iter().all(|f| f.category == "unsafe_code"));
    }

    #[test]
    fn test_no_unsafe_code() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(
            &file_path,
            r#"
fn safe_function() {
    println!("All safe!");
}
"#,
        )
        .unwrap();

        let detector = UnsafeCodeDetector::new(dir.path());
        let findings = detector.scan().unwrap();

        assert_eq!(findings.len(), 0);
    }
}
