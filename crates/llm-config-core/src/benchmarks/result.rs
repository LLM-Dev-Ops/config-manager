//! Canonical BenchmarkResult struct for standardized benchmark output
//!
//! This module provides the canonical benchmark result structure used across
//! all 25 benchmark-target repositories in the LLM-Dev-Ops ecosystem.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical benchmark result structure with standardized fields.
///
/// This struct is the standard format used across all LLM-Dev-Ops benchmark targets
/// to ensure consistent benchmark result reporting and aggregation.
///
/// # Fields
/// - `target_id`: Unique identifier for the benchmark target
/// - `metrics`: Flexible JSON structure containing benchmark measurements
/// - `timestamp`: UTC timestamp when the benchmark was executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Unique identifier for the benchmark target (e.g., "config_merge", "secret_load")
    pub target_id: String,

    /// Flexible JSON structure containing benchmark metrics
    /// Common metrics include:
    /// - `duration_ns`: Execution time in nanoseconds
    /// - `throughput_ops_per_sec`: Operations per second
    /// - `memory_bytes`: Memory usage in bytes
    /// - `iterations`: Number of iterations performed
    pub metrics: serde_json::Value,

    /// UTC timestamp when the benchmark was executed
    pub timestamp: DateTime<Utc>,
}

impl BenchmarkResult {
    /// Create a new benchmark result with the current timestamp
    pub fn new(target_id: impl Into<String>, metrics: serde_json::Value) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp: Utc::now(),
        }
    }

    /// Create a benchmark result with a specific timestamp
    pub fn with_timestamp(
        target_id: impl Into<String>,
        metrics: serde_json::Value,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            metrics,
            timestamp,
        }
    }

    /// Create a simple timing result
    pub fn timing(target_id: impl Into<String>, duration_ns: u128) -> Self {
        Self::new(
            target_id,
            serde_json::json!({
                "duration_ns": duration_ns,
                "duration_ms": duration_ns as f64 / 1_000_000.0,
                "duration_s": duration_ns as f64 / 1_000_000_000.0
            }),
        )
    }

    /// Create a throughput result
    pub fn throughput(
        target_id: impl Into<String>,
        duration_ns: u128,
        operations: u64,
    ) -> Self {
        let duration_s = duration_ns as f64 / 1_000_000_000.0;
        let ops_per_sec = if duration_s > 0.0 {
            operations as f64 / duration_s
        } else {
            0.0
        };

        Self::new(
            target_id,
            serde_json::json!({
                "duration_ns": duration_ns,
                "duration_ms": duration_ns as f64 / 1_000_000.0,
                "duration_s": duration_s,
                "operations": operations,
                "throughput_ops_per_sec": ops_per_sec
            }),
        )
    }

    /// Add additional metrics to the result
    pub fn with_metric(mut self, key: &str, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.metrics {
            map.insert(key.to_string(), value);
        }
        self
    }

    /// Get a metric value by key
    pub fn get_metric(&self, key: &str) -> Option<&serde_json::Value> {
        self.metrics.get(key)
    }

    /// Get duration in nanoseconds if available
    pub fn duration_ns(&self) -> Option<u128> {
        self.metrics
            .get("duration_ns")
            .and_then(|v| v.as_u64())
            .map(|v| v as u128)
    }

    /// Get throughput in ops/sec if available
    pub fn throughput_ops_per_sec(&self) -> Option<f64> {
        self.metrics
            .get("throughput_ops_per_sec")
            .and_then(|v| v.as_f64())
    }
}

impl std::fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} - {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.target_id,
            self.metrics
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result_new() {
        let result = BenchmarkResult::new(
            "test_target",
            serde_json::json!({"duration_ns": 1000}),
        );
        assert_eq!(result.target_id, "test_target");
        assert_eq!(result.metrics["duration_ns"], 1000);
    }

    #[test]
    fn test_benchmark_result_timing() {
        let result = BenchmarkResult::timing("config_get", 1_000_000);
        assert_eq!(result.target_id, "config_get");
        assert_eq!(result.duration_ns(), Some(1_000_000));
    }

    #[test]
    fn test_benchmark_result_throughput() {
        let result = BenchmarkResult::throughput("config_set", 1_000_000_000, 1000);
        assert_eq!(result.target_id, "config_set");
        assert_eq!(result.throughput_ops_per_sec(), Some(1000.0));
    }

    #[test]
    fn test_benchmark_result_with_metric() {
        let result = BenchmarkResult::timing("test", 1000)
            .with_metric("memory_bytes", serde_json::json!(1024));
        assert_eq!(result.get_metric("memory_bytes"), Some(&serde_json::json!(1024)));
    }

    #[test]
    fn test_benchmark_result_serialization() {
        let result = BenchmarkResult::timing("test", 1000);
        let json = serde_json::to_string(&result).unwrap();
        let parsed: BenchmarkResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.target_id, result.target_id);
    }
}
