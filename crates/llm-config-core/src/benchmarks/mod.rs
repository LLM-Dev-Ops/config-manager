//! Canonical Benchmark Module for LLM Config Manager
//!
//! This module provides the standardized benchmark interface used across
//! all 25 benchmark-target repositories in the LLM-Dev-Ops ecosystem.
//!
//! # Structure
//!
//! - `result`: The canonical `BenchmarkResult` struct with standardized fields
//! - `io`: I/O operations for reading/writing benchmark results
//! - `markdown`: Markdown report generation
//! - `adapters`: Benchmark target implementations using the `BenchTarget` trait
//!
//! # Usage
//!
//! ```rust,ignore
//! use llm_config_core::benchmarks::{run_all_benchmarks, adapters::all_targets};
//!
//! // Run all benchmarks
//! let results = run_all_benchmarks();
//!
//! // Or get specific targets
//! let targets = all_targets();
//! for target in targets {
//!     println!("{}: {}", target.id(), target.description());
//!     let result = target.run();
//!     println!("  Duration: {:?}ms", result.duration_ns().map(|n| n / 1_000_000));
//! }
//! ```

pub mod result;
pub mod io;
pub mod markdown;
pub mod adapters;

pub use result::BenchmarkResult;
pub use adapters::{BenchTarget, all_targets, get_target, list_target_ids};

use std::path::Path;

/// Run all registered benchmarks and return their results.
///
/// This is the canonical entrypoint for the benchmark system, returning
/// a `Vec<BenchmarkResult>` with standardized fields:
/// - `target_id: String` - Unique identifier for the benchmark
/// - `metrics: serde_json::Value` - Flexible metrics structure
/// - `timestamp: chrono::DateTime<chrono::Utc>` - When the benchmark ran
///
/// # Example
///
/// ```rust,ignore
/// use llm_config_core::benchmarks::run_all_benchmarks;
///
/// let results = run_all_benchmarks();
/// for result in results {
///     println!("{}: {:?}", result.target_id, result.metrics);
/// }
/// ```
pub fn run_all_benchmarks() -> Vec<BenchmarkResult> {
    let targets = all_targets();
    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        tracing::info!(target_id = target.id(), "Running benchmark");
        let result = target.run();
        tracing::info!(
            target_id = target.id(),
            duration_ns = ?result.duration_ns(),
            "Benchmark complete"
        );
        results.push(result);
    }

    results
}

/// Run benchmarks for a specific category.
pub fn run_benchmarks_by_category(category: &str) -> Vec<BenchmarkResult> {
    let targets = adapters::targets_by_category(category);
    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        tracing::info!(target_id = target.id(), category = category, "Running benchmark");
        let result = target.run();
        results.push(result);
    }

    results
}

/// Run a specific benchmark by ID.
pub fn run_benchmark(target_id: &str) -> Option<BenchmarkResult> {
    get_target(target_id).map(|target| {
        tracing::info!(target_id = target.id(), "Running benchmark");
        target.run()
    })
}

/// Run all benchmarks and write results to the canonical output directories.
///
/// This function:
/// 1. Ensures output directories exist
/// 2. Runs all benchmarks
/// 3. Writes raw results to `benchmarks/output/raw/`
/// 4. Updates `benchmarks/output/summary.md`
///
/// Returns the benchmark results.
pub fn run_and_save(base_path: &Path) -> std::io::Result<Vec<BenchmarkResult>> {
    // Ensure output directories exist
    io::ensure_output_dirs(base_path)?;

    // Run all benchmarks
    let results = run_all_benchmarks();

    // Write results
    io::write_benchmark_run(base_path, &results)?;

    // Update summary
    markdown::update_summary(base_path)?;

    Ok(results)
}

/// Get a summary of all available benchmarks.
pub fn list_benchmarks() -> Vec<BenchmarkInfo> {
    all_targets()
        .iter()
        .map(|t| BenchmarkInfo {
            id: t.id().to_string(),
            description: t.description().to_string(),
            category: t.category().to_string(),
        })
        .collect()
}

/// Information about an available benchmark.
#[derive(Debug, Clone)]
pub struct BenchmarkInfo {
    pub id: String,
    pub description: String,
    pub category: String,
}

impl std::fmt::Display for BenchmarkInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.category, self.id, self.description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_all_benchmarks() {
        let results = run_all_benchmarks();
        assert!(!results.is_empty());

        // Verify each result has the canonical fields
        for result in &results {
            assert!(!result.target_id.is_empty());
            assert!(result.metrics.is_object());
        }
    }

    #[test]
    fn test_run_benchmark() {
        let result = run_benchmark("config_get");
        assert!(result.is_some());
        assert_eq!(result.unwrap().target_id, "config_get");
    }

    #[test]
    fn test_run_benchmark_not_found() {
        let result = run_benchmark("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_run_and_save() {
        let temp_dir = TempDir::new().unwrap();
        let results = run_and_save(temp_dir.path()).unwrap();

        assert!(!results.is_empty());

        // Verify output files exist
        assert!(temp_dir.path().join(io::OUTPUT_DIR).exists());
        assert!(temp_dir.path().join(io::RAW_OUTPUT_DIR).exists());
        assert!(temp_dir.path().join(io::SUMMARY_FILE).exists());
    }

    #[test]
    fn test_list_benchmarks() {
        let benchmarks = list_benchmarks();
        assert!(!benchmarks.is_empty());

        // Verify we have benchmarks from multiple categories
        let categories: std::collections::HashSet<_> = benchmarks.iter().map(|b| &b.category).collect();
        assert!(categories.len() > 1);
    }

    #[test]
    fn test_run_benchmarks_by_category() {
        let config_results = run_benchmarks_by_category("config");
        assert!(!config_results.is_empty());

        // All results should be from the config category
        for result in &config_results {
            assert!(
                result.target_id.starts_with("config_")
                    || result.target_id.starts_with("secret_")
            );
        }
    }
}
