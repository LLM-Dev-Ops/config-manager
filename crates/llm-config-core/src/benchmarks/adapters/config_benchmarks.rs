//! Configuration benchmarks
//!
//! Benchmark targets for core Config Manager operations including
//! get, set, list, merge, and environment override resolution.

use super::BenchTarget;
use crate::benchmarks::result::BenchmarkResult;
use crate::{ConfigManager, ConfigValue, Environment};
use std::time::Instant;
use tempfile::TempDir;

/// Benchmark for config get operations
pub struct ConfigGetBenchmark {
    iterations: u32,
}

impl ConfigGetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for ConfigGetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ConfigGetBenchmark {
    fn id(&self) -> &str {
        "config_get"
    }

    fn description(&self) -> &str {
        "Measures single configuration value retrieval time"
    }

    fn category(&self) -> &str {
        "config"
    }

    fn run(&self) -> BenchmarkResult {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager = ConfigManager::new(temp_dir.path()).expect("Failed to create manager");

        // Setup: create a config entry
        manager
            .set(
                "bench/ns",
                "test_key",
                ConfigValue::String("test_value".to_string()),
                Environment::Development,
                "benchmark",
            )
            .expect("Failed to set config");

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = manager.get("bench/ns", "test_key", Environment::Development);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = manager.get("bench/ns", "test_key", Environment::Development);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("get"))
    }
}

/// Benchmark for config set operations
pub struct ConfigSetBenchmark {
    iterations: u32,
}

impl ConfigSetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for ConfigSetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ConfigSetBenchmark {
    fn id(&self) -> &str {
        "config_set"
    }

    fn description(&self) -> &str {
        "Measures configuration value write time including versioning"
    }

    fn category(&self) -> &str {
        "config"
    }

    fn run(&self) -> BenchmarkResult {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager = ConfigManager::new(temp_dir.path()).expect("Failed to create manager");

        // Warmup
        for i in 0..self.warmup_iterations() {
            let _ = manager.set(
                "warmup/ns",
                format!("key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
                "benchmark",
            );
        }

        // Measure
        let start = Instant::now();
        for i in 0..self.iterations {
            let _ = manager.set(
                "bench/ns",
                format!("key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
                "benchmark",
            );
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("set"))
    }
}

/// Benchmark for config list operations
pub struct ConfigListBenchmark {
    iterations: u32,
    entry_count: u32,
}

impl ConfigListBenchmark {
    pub fn new() -> Self {
        Self {
            iterations: 50,
            entry_count: 100,
        }
    }
}

impl Default for ConfigListBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ConfigListBenchmark {
    fn id(&self) -> &str {
        "config_list"
    }

    fn description(&self) -> &str {
        "Measures configuration listing time for namespace with 100 entries"
    }

    fn category(&self) -> &str {
        "config"
    }

    fn run(&self) -> BenchmarkResult {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager = ConfigManager::new(temp_dir.path()).expect("Failed to create manager");

        // Setup: create multiple entries
        for i in 0..self.entry_count {
            let _ = manager.set(
                "bench/ns",
                format!("key_{}", i),
                ConfigValue::String(format!("value_{}", i)),
                Environment::Development,
                "benchmark",
            );
        }

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = manager.list("bench/ns", Environment::Development);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = manager.list("bench/ns", Environment::Development);
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

/// Benchmark for cascading configuration merge
pub struct ConfigMergeBenchmark {
    iterations: u32,
}

impl ConfigMergeBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for ConfigMergeBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ConfigMergeBenchmark {
    fn id(&self) -> &str {
        "config_merge"
    }

    fn description(&self) -> &str {
        "Measures cascading configuration merge speed across environments"
    }

    fn category(&self) -> &str {
        "config"
    }

    fn run(&self) -> BenchmarkResult {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager = ConfigManager::new(temp_dir.path()).expect("Failed to create manager");

        // Setup: create entries across all environments
        let environments = [
            Environment::Base,
            Environment::Development,
            Environment::Staging,
            Environment::Production,
        ];

        for (i, env) in environments.iter().enumerate() {
            let _ = manager.set(
                "bench/ns",
                "merge_key",
                ConfigValue::String(format!("value_env_{}", i)),
                *env,
                "benchmark",
            );
        }

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = manager.get_with_overrides("bench/ns", "merge_key", Environment::Production);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = manager.get_with_overrides("bench/ns", "merge_key", Environment::Production);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("merge"))
        .with_metric("environment_count", serde_json::json!(environments.len()))
    }
}

/// Benchmark for environment override resolution
pub struct ConfigOverrideBenchmark {
    iterations: u32,
}

impl ConfigOverrideBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for ConfigOverrideBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for ConfigOverrideBenchmark {
    fn id(&self) -> &str {
        "config_override"
    }

    fn description(&self) -> &str {
        "Measures environment-specific override resolution time"
    }

    fn category(&self) -> &str {
        "config"
    }

    fn run(&self) -> BenchmarkResult {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let manager = ConfigManager::new(temp_dir.path()).expect("Failed to create manager");

        // Setup: create base and environment-specific configs
        let _ = manager.set(
            "bench/ns",
            "override_key",
            ConfigValue::String("base_value".to_string()),
            Environment::Base,
            "benchmark",
        );
        let _ = manager.set(
            "bench/ns",
            "override_key",
            ConfigValue::String("prod_value".to_string()),
            Environment::Production,
            "benchmark",
        );

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = manager.get_with_overrides("bench/ns", "override_key", Environment::Production);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = manager.get_with_overrides("bench/ns", "override_key", Environment::Production);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("override"))
    }
}

/// Benchmark for secret set operations
pub struct SecretSetBenchmark {
    iterations: u32,
}

impl SecretSetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 50 }
    }
}

impl Default for SecretSetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for SecretSetBenchmark {
    fn id(&self) -> &str {
        "secret_set"
    }

    fn description(&self) -> &str {
        "Measures secret encryption and storage time"
    }

    fn category(&self) -> &str {
        "secrets"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_crypto::{Algorithm, SecretKey};

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let key = SecretKey::generate(Algorithm::Aes256Gcm).expect("Failed to generate key");
        let manager = ConfigManager::new(temp_dir.path())
            .expect("Failed to create manager")
            .with_encryption_key(key);

        let secret_data = b"super-secret-password-12345";

        // Warmup
        for i in 0..self.warmup_iterations() {
            let _ = manager.set_secret(
                "warmup/ns",
                format!("secret_{}", i),
                secret_data,
                Environment::Production,
                "benchmark",
            );
        }

        // Measure
        let start = Instant::now();
        for i in 0..self.iterations {
            let _ = manager.set_secret(
                "bench/ns",
                format!("secret_{}", i),
                secret_data,
                Environment::Production,
                "benchmark",
            );
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("secret_set"))
        .with_metric("secret_size_bytes", serde_json::json!(secret_data.len()))
    }
}

/// Benchmark for secret get operations
pub struct SecretGetBenchmark {
    iterations: u32,
}

impl SecretGetBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for SecretGetBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for SecretGetBenchmark {
    fn id(&self) -> &str {
        "secret_get"
    }

    fn description(&self) -> &str {
        "Measures secret retrieval and decryption time"
    }

    fn category(&self) -> &str {
        "secrets"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_crypto::{Algorithm, SecretKey};

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let key = SecretKey::generate(Algorithm::Aes256Gcm).expect("Failed to generate key");
        let manager = ConfigManager::new(temp_dir.path())
            .expect("Failed to create manager")
            .with_encryption_key(key);

        // Setup: create a secret
        let secret_data = b"super-secret-password-12345";
        manager
            .set_secret(
                "bench/ns",
                "test_secret",
                secret_data,
                Environment::Production,
                "benchmark",
            )
            .expect("Failed to set secret");

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = manager.get_secret("bench/ns", "test_secret", Environment::Production);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = manager.get_secret("bench/ns", "test_secret", Environment::Production);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("secret_get"))
        .with_metric("secret_size_bytes", serde_json::json!(secret_data.len()))
    }
}
