//! I/O operations for benchmark results
//!
//! This module provides functionality for reading and writing benchmark results
//! to the canonical output directories.

use super::result::BenchmarkResult;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Default paths for benchmark output
pub const OUTPUT_DIR: &str = "benchmarks/output";
pub const RAW_OUTPUT_DIR: &str = "benchmarks/output/raw";
pub const SUMMARY_FILE: &str = "benchmarks/output/summary.md";

/// Write benchmark results to a JSON file in the raw output directory
pub fn write_raw_results(
    base_path: &Path,
    results: &[BenchmarkResult],
    filename: &str,
) -> io::Result<PathBuf> {
    let raw_dir = base_path.join(RAW_OUTPUT_DIR);
    fs::create_dir_all(&raw_dir)?;

    let file_path = raw_dir.join(format!("{}.json", filename));
    let json = serde_json::to_string_pretty(results)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    fs::write(&file_path, json)?;
    Ok(file_path)
}

/// Write a single benchmark result to the raw output directory
pub fn write_raw_result(
    base_path: &Path,
    result: &BenchmarkResult,
) -> io::Result<PathBuf> {
    let raw_dir = base_path.join(RAW_OUTPUT_DIR);
    fs::create_dir_all(&raw_dir)?;

    let timestamp = result.timestamp.format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.json", result.target_id, timestamp);
    let file_path = raw_dir.join(&filename);

    let json = serde_json::to_string_pretty(result)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    fs::write(&file_path, json)?;
    Ok(file_path)
}

/// Read all raw benchmark results from the output directory
pub fn read_raw_results(base_path: &Path) -> io::Result<Vec<BenchmarkResult>> {
    let raw_dir = base_path.join(RAW_OUTPUT_DIR);

    if !raw_dir.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    for entry in fs::read_dir(&raw_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            let content = fs::read_to_string(&path)?;

            // Try to parse as a single result or an array
            if let Ok(result) = serde_json::from_str::<BenchmarkResult>(&content) {
                results.push(result);
            } else if let Ok(arr) = serde_json::from_str::<Vec<BenchmarkResult>>(&content) {
                results.extend(arr);
            }
        }
    }

    // Sort by timestamp (most recent first)
    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(results)
}

/// Read the latest result for a specific target
pub fn read_latest_result(
    base_path: &Path,
    target_id: &str,
) -> io::Result<Option<BenchmarkResult>> {
    let results = read_raw_results(base_path)?;

    Ok(results
        .into_iter()
        .find(|r| r.target_id == target_id))
}

/// Write the benchmark run results and return the output path
pub fn write_benchmark_run(
    base_path: &Path,
    results: &[BenchmarkResult],
) -> io::Result<PathBuf> {
    let output_dir = base_path.join(OUTPUT_DIR);
    fs::create_dir_all(&output_dir)?;

    // Write individual raw results
    for result in results {
        write_raw_result(base_path, result)?;
    }

    // Write combined results file
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let combined_path = write_raw_results(
        base_path,
        results,
        &format!("run_{}", timestamp),
    )?;

    Ok(combined_path)
}

/// Ensure the canonical output directory structure exists
pub fn ensure_output_dirs(base_path: &Path) -> io::Result<()> {
    fs::create_dir_all(base_path.join(OUTPUT_DIR))?;
    fs::create_dir_all(base_path.join(RAW_OUTPUT_DIR))?;

    // Create empty summary.md if it doesn't exist
    let summary_path = base_path.join(SUMMARY_FILE);
    if !summary_path.exists() {
        let mut file = fs::File::create(&summary_path)?;
        writeln!(file, "# Benchmark Summary")?;
        writeln!(file)?;
        writeln!(file, "This file contains benchmark results for LLM Config Manager.")?;
        writeln!(file)?;
        writeln!(file, "## Latest Results")?;
        writeln!(file)?;
        writeln!(file, "_No benchmarks have been run yet._")?;
    }

    Ok(())
}

/// Get the canonical output directory path
pub fn output_dir(base_path: &Path) -> PathBuf {
    base_path.join(OUTPUT_DIR)
}

/// Get the canonical raw output directory path
pub fn raw_output_dir(base_path: &Path) -> PathBuf {
    base_path.join(RAW_OUTPUT_DIR)
}

/// Get the canonical summary file path
pub fn summary_file(base_path: &Path) -> PathBuf {
    base_path.join(SUMMARY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ensure_output_dirs() {
        let temp_dir = TempDir::new().unwrap();
        ensure_output_dirs(temp_dir.path()).unwrap();

        assert!(temp_dir.path().join(OUTPUT_DIR).exists());
        assert!(temp_dir.path().join(RAW_OUTPUT_DIR).exists());
        assert!(temp_dir.path().join(SUMMARY_FILE).exists());
    }

    #[test]
    fn test_write_and_read_raw_result() {
        let temp_dir = TempDir::new().unwrap();
        let result = BenchmarkResult::timing("test_target", 1000);

        let path = write_raw_result(temp_dir.path(), &result).unwrap();
        assert!(path.exists());

        let results = read_raw_results(temp_dir.path()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].target_id, "test_target");
    }

    #[test]
    fn test_write_and_read_raw_results() {
        let temp_dir = TempDir::new().unwrap();
        let results = vec![
            BenchmarkResult::timing("target1", 1000),
            BenchmarkResult::timing("target2", 2000),
        ];

        write_raw_results(temp_dir.path(), &results, "test_run").unwrap();

        let read_results = read_raw_results(temp_dir.path()).unwrap();
        assert_eq!(read_results.len(), 2);
    }

    #[test]
    fn test_read_latest_result() {
        let temp_dir = TempDir::new().unwrap();

        // Write two results with same target
        let result1 = BenchmarkResult::timing("test_target", 1000);
        write_raw_result(temp_dir.path(), &result1).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let result2 = BenchmarkResult::timing("test_target", 2000);
        write_raw_result(temp_dir.path(), &result2).unwrap();

        let latest = read_latest_result(temp_dir.path(), "test_target").unwrap();
        assert!(latest.is_some());
        // Latest should have duration 2000
        assert_eq!(latest.unwrap().duration_ns(), Some(2000));
    }
}
