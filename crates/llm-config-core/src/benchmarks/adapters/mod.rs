//! Benchmark adapters module
//!
//! This module implements the canonical BenchTarget trait and provides
//! a registry of all benchmark targets for the Config Manager.
//!
//! Note: Cache benchmarks have been moved to the llm-config-cache crate
//! to avoid circular dependencies in the workspace.

mod config_benchmarks;
mod crypto_benchmarks;
mod storage_benchmarks;

pub use config_benchmarks::*;
pub use crypto_benchmarks::*;
pub use storage_benchmarks::*;

use super::result::BenchmarkResult;

/// The canonical BenchTarget trait for benchmark adapters.
///
/// All benchmark targets must implement this trait to be registered
/// in the benchmark system.
pub trait BenchTarget: Send + Sync {
    /// Returns the unique identifier for this benchmark target.
    ///
    /// The ID should be a lowercase, snake_case string that uniquely
    /// identifies this benchmark (e.g., "config_merge_speed", "secret_load_latency").
    fn id(&self) -> &str;

    /// Execute the benchmark and return the result.
    ///
    /// This method should:
    /// 1. Set up any necessary test fixtures
    /// 2. Run the benchmark operation
    /// 3. Collect timing/throughput metrics
    /// 4. Clean up test fixtures
    /// 5. Return a BenchmarkResult with the collected metrics
    fn run(&self) -> BenchmarkResult;

    /// Optional: Return a description of what this benchmark measures
    fn description(&self) -> &str {
        "No description provided"
    }

    /// Optional: Return the category this benchmark belongs to
    fn category(&self) -> &str {
        "general"
    }

    /// Optional: Number of warmup iterations before measurement
    fn warmup_iterations(&self) -> u32 {
        10
    }

    /// Optional: Number of measurement iterations
    fn measurement_iterations(&self) -> u32 {
        100
    }
}

/// Registry of all benchmark targets.
///
/// Returns a vector of boxed trait objects implementing BenchTarget.
/// This is the canonical entry point for discovering all available benchmarks.
///
/// Note: Cache benchmarks are available in the llm-config-cache crate.
pub fn all_targets() -> Vec<Box<dyn BenchTarget>> {
    vec![
        // Config Manager benchmarks
        Box::new(ConfigGetBenchmark::new()),
        Box::new(ConfigSetBenchmark::new()),
        Box::new(ConfigListBenchmark::new()),
        Box::new(ConfigMergeBenchmark::new()),
        Box::new(ConfigOverrideBenchmark::new()),

        // Secret loading benchmarks
        Box::new(SecretSetBenchmark::new()),
        Box::new(SecretGetBenchmark::new()),

        // Crypto benchmarks
        Box::new(EncryptBenchmark::new()),
        Box::new(DecryptBenchmark::new()),
        Box::new(KeyGenerationBenchmark::new()),

        // Storage benchmarks
        Box::new(StorageWriteBenchmark::new()),
        Box::new(StorageReadBenchmark::new()),
        Box::new(StorageListBenchmark::new()),
    ]
}

/// Get a specific benchmark target by ID
pub fn get_target(id: &str) -> Option<Box<dyn BenchTarget>> {
    all_targets().into_iter().find(|t| t.id() == id)
}

/// Get all benchmark targets in a specific category
pub fn targets_by_category(category: &str) -> Vec<Box<dyn BenchTarget>> {
    all_targets()
        .into_iter()
        .filter(|t| t.category() == category)
        .collect()
}

/// List all available benchmark IDs
pub fn list_target_ids() -> Vec<String> {
    all_targets().iter().map(|t| t.id().to_string()).collect()
}

/// List all available categories
pub fn list_categories() -> Vec<String> {
    let mut categories: Vec<_> = all_targets()
        .iter()
        .map(|t| t.category().to_string())
        .collect();
    categories.sort();
    categories.dedup();
    categories
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_targets_not_empty() {
        let targets = all_targets();
        assert!(!targets.is_empty());
    }

    #[test]
    fn test_all_targets_have_unique_ids() {
        let targets = all_targets();
        let ids: Vec<_> = targets.iter().map(|t| t.id()).collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique_ids.len(), "Duplicate target IDs found");
    }

    #[test]
    fn test_get_target() {
        let target = get_target("config_get");
        assert!(target.is_some());
        assert_eq!(target.unwrap().id(), "config_get");
    }

    #[test]
    fn test_get_target_not_found() {
        let target = get_target("nonexistent");
        assert!(target.is_none());
    }

    #[test]
    fn test_list_target_ids() {
        let ids = list_target_ids();
        assert!(!ids.is_empty());
        assert!(ids.contains(&"config_get".to_string()));
    }

    #[test]
    fn test_list_categories() {
        let categories = list_categories();
        assert!(!categories.is_empty());
    }
}
