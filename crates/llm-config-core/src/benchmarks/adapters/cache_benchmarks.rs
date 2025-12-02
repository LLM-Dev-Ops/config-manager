//! Cache benchmarks
//!
//! Benchmark targets for cache operations including L1/L2 cache
//! get/put operations, cache promotion, and mixed workloads.

use super::BenchTarget;
use crate::benchmarks::result::BenchmarkResult;
use crate::{ConfigEntry, ConfigValue, ConfigMetadata, Environment};
use std::time::Instant;

fn create_test_entry(namespace: &str, key: &str, env: Environment) -> ConfigEntry {
    ConfigEntry {
        id: uuid::Uuid::new_v4(),
        namespace: namespace.to_string(),
        key: key.to_string(),
        value: ConfigValue::String("test-value".to_string()),
        environment: env,
        version: 1,
        metadata: ConfigMetadata {
            created_at: chrono::Utc::now(),
            created_by: "benchmark".to_string(),
            updated_at: chrono::Utc::now(),
            updated_by: "benchmark".to_string(),
            tags: vec![],
            description: None,
        },
    }
}

/// Benchmark for L1 cache get operations
pub struct CacheL1GetBenchmark {
    iterations: u32,
}

impl CacheL1GetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 1000 }
    }
}

impl Default for CacheL1GetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CacheL1GetBenchmark {
    fn id(&self) -> &str {
        "cache_l1_get"
    }

    fn description(&self) -> &str {
        "Measures L1 (in-memory) cache lookup time"
    }

    fn category(&self) -> &str {
        "cache"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_cache::CacheManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheManager::new(1000, temp_dir.path())
            .expect("Failed to create cache");

        // Setup: put an entry
        let entry = create_test_entry("bench/ns", "cache_key", Environment::Development);
        cache.put(entry).expect("Failed to put cache entry");

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = cache.get("bench/ns", "cache_key", "development");
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = cache.get("bench/ns", "cache_key", "development");
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("l1_get"))
        .with_metric("cache_tier", serde_json::json!("l1"))
    }
}

/// Benchmark for L1 cache put operations
pub struct CacheL1PutBenchmark {
    iterations: u32,
}

impl CacheL1PutBenchmark {
    pub fn new() -> Self {
        Self { iterations: 500 }
    }
}

impl Default for CacheL1PutBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CacheL1PutBenchmark {
    fn id(&self) -> &str {
        "cache_l1_put"
    }

    fn description(&self) -> &str {
        "Measures L1 (in-memory) cache write time"
    }

    fn category(&self) -> &str {
        "cache"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_cache::CacheManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheManager::new(10000, temp_dir.path())
            .expect("Failed to create cache");

        // Warmup
        for i in 0..self.warmup_iterations() {
            let entry = create_test_entry("warmup/ns", &format!("key_{}", i), Environment::Development);
            let _ = cache.put(entry);
        }

        // Measure
        let start = Instant::now();
        for i in 0..self.iterations {
            let entry = create_test_entry("bench/ns", &format!("key_{}", i), Environment::Development);
            let _ = cache.put(entry);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("l1_put"))
        .with_metric("cache_tier", serde_json::json!("l1"))
    }
}

/// Benchmark for L2 cache get operations
pub struct CacheL2GetBenchmark {
    iterations: u32,
}

impl CacheL2GetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for CacheL2GetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CacheL2GetBenchmark {
    fn id(&self) -> &str {
        "cache_l2_get"
    }

    fn description(&self) -> &str {
        "Measures L2 (disk-backed) cache lookup time after L1 miss"
    }

    fn category(&self) -> &str {
        "cache"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_cache::CacheManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheManager::new(10, temp_dir.path())
            .expect("Failed to create cache");

        // Setup: put an entry then clear L1 to force L2 access
        let entry = create_test_entry("bench/ns", "l2_key", Environment::Development);
        cache.put(entry).expect("Failed to put cache entry");

        // Clear L1 cache to force L2 access
        cache.clear_l1();

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = cache.get("bench/ns", "l2_key", "development");
            cache.clear_l1();
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = cache.get("bench/ns", "l2_key", "development");
            cache.clear_l1();
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("l2_get"))
        .with_metric("cache_tier", serde_json::json!("l2"))
    }
}

/// Benchmark for cache promotion (L2 -> L1)
pub struct CachePromotionBenchmark {
    iterations: u32,
}

impl CachePromotionBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for CachePromotionBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CachePromotionBenchmark {
    fn id(&self) -> &str {
        "cache_promotion"
    }

    fn description(&self) -> &str {
        "Measures cache entry promotion time from L2 to L1"
    }

    fn category(&self) -> &str {
        "cache"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_cache::CacheManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheManager::new(100, temp_dir.path())
            .expect("Failed to create cache");

        // Setup: put entries
        for i in 0..self.iterations {
            let entry = create_test_entry("bench/ns", &format!("promo_key_{}", i), Environment::Development);
            cache.put(entry).expect("Failed to put cache entry");
        }

        // Clear L1 to force promotion on next access
        cache.clear_l1();

        // Measure promotion (first access after L1 clear promotes from L2)
        let start = Instant::now();
        for i in 0..self.iterations {
            let _ = cache.get("bench/ns", &format!("promo_key_{}", i), "development");
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("promotion"))
    }
}

/// Benchmark for mixed cache operations
pub struct CacheMixedBenchmark {
    iterations: u32,
}

impl CacheMixedBenchmark {
    pub fn new() -> Self {
        Self { iterations: 500 }
    }
}

impl Default for CacheMixedBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for CacheMixedBenchmark {
    fn id(&self) -> &str {
        "cache_mixed"
    }

    fn description(&self) -> &str {
        "Measures mixed cache workload (70% reads, 20% writes, 10% invalidations)"
    }

    fn category(&self) -> &str {
        "cache"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_cache::CacheManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheManager::new(1000, temp_dir.path())
            .expect("Failed to create cache");

        // Setup: populate cache
        for i in 0..100 {
            let entry = create_test_entry("bench/ns", &format!("mixed_key_{}", i), Environment::Development);
            let _ = cache.put(entry);
        }

        let reads = (self.iterations as f32 * 0.7) as u32;
        let writes = (self.iterations as f32 * 0.2) as u32;
        let invalidations = self.iterations - reads - writes;

        // Measure mixed workload
        let start = Instant::now();

        // Reads
        for i in 0..reads {
            let _ = cache.get("bench/ns", &format!("mixed_key_{}", i % 100), "development");
        }

        // Writes
        for i in 0..writes {
            let entry = create_test_entry("bench/ns", &format!("mixed_key_{}", i % 100), Environment::Development);
            let _ = cache.put(entry);
        }

        // Invalidations
        for i in 0..invalidations {
            let _ = cache.invalidate("bench/ns", &format!("mixed_key_{}", i % 100), "development");
        }

        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("mixed"))
        .with_metric("reads", serde_json::json!(reads))
        .with_metric("writes", serde_json::json!(writes))
        .with_metric("invalidations", serde_json::json!(invalidations))
    }
}
