//! Storage benchmarks
//!
//! Benchmark targets for storage operations including
//! file writes, reads, and listing.

use super::BenchTarget;
use crate::benchmarks::result::BenchmarkResult;
use std::time::Instant;

/// Benchmark for storage write operations
pub struct StorageWriteBenchmark {
    iterations: u32,
}

impl StorageWriteBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for StorageWriteBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for StorageWriteBenchmark {
    fn id(&self) -> &str {
        "storage_write"
    }

    fn description(&self) -> &str {
        "Measures atomic file storage write time"
    }

    fn category(&self) -> &str {
        "storage"
    }

    fn run(&self) -> BenchmarkResult {
        use crate::{ConfigEntry, ConfigValue, Environment};
        use llm_config_storage::file::FileStorage;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage = FileStorage::new(temp_dir.path()).expect("Failed to create storage");

        // Warmup
        for i in 0..self.warmup_iterations() {
            let entry = ConfigEntry::new(
                "warmup/ns".to_string(),
                format!("key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
            );
            let _ = storage.set(entry);
        }

        // Measure
        let start = Instant::now();
        for i in 0..self.iterations {
            let entry = ConfigEntry::new(
                "bench/ns".to_string(),
                format!("key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
            );
            let _ = storage.set(entry);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("write"))
    }
}

/// Benchmark for storage read operations
pub struct StorageReadBenchmark {
    iterations: u32,
}

impl StorageReadBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for StorageReadBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for StorageReadBenchmark {
    fn id(&self) -> &str {
        "storage_read"
    }

    fn description(&self) -> &str {
        "Measures file storage read time with in-memory index"
    }

    fn category(&self) -> &str {
        "storage"
    }

    fn run(&self) -> BenchmarkResult {
        use crate::{ConfigEntry, ConfigValue, Environment};
        use llm_config_storage::file::FileStorage;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage = FileStorage::new(temp_dir.path()).expect("Failed to create storage");

        // Setup: create an entry
        let entry = ConfigEntry::new(
            "bench/ns".to_string(),
            "read_key".to_string(),
            ConfigValue::String("read_value".to_string()),
            Environment::Development,
        );
        storage.set(entry).expect("Failed to set entry");

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = storage.get("bench/ns", "read_key", Environment::Development);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = storage.get("bench/ns", "read_key", Environment::Development);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("read"))
    }
}

/// Benchmark for storage list operations
pub struct StorageListBenchmark {
    iterations: u32,
    entry_count: u32,
}

impl StorageListBenchmark {
    pub fn new() -> Self {
        Self {
            iterations: 50,
            entry_count: 100,
        }
    }
}

impl Default for StorageListBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for StorageListBenchmark {
    fn id(&self) -> &str {
        "storage_list"
    }

    fn description(&self) -> &str {
        "Measures file storage listing time for namespace with 100 entries"
    }

    fn category(&self) -> &str {
        "storage"
    }

    fn run(&self) -> BenchmarkResult {
        use crate::{ConfigEntry, ConfigValue, Environment};
        use llm_config_storage::file::FileStorage;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage = FileStorage::new(temp_dir.path()).expect("Failed to create storage");

        // Setup: create multiple entries
        for i in 0..self.entry_count {
            let entry = ConfigEntry::new(
                "bench/ns".to_string(),
                format!("list_key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
            );
            storage.set(entry).expect("Failed to set entry");
        }

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = storage.list("bench/ns", Environment::Development);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = storage.list("bench/ns", Environment::Development);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("list"))
        .with_metric("entry_count", serde_json::json!(self.entry_count))
    }
}
