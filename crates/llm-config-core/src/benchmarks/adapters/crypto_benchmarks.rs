//! Cryptography benchmarks
//!
//! Benchmark targets for cryptographic operations including
//! encryption, decryption, and key generation.

use super::BenchTarget;
use crate::benchmarks::result::BenchmarkResult;
use std::time::Instant;

/// Benchmark for encryption operations
pub struct EncryptBenchmark {
    iterations: u32,
    payload_size: usize,
}

impl EncryptBenchmark {
    pub fn new() -> Self {
        Self {
            iterations: 100,
            payload_size: 1024, // 1KB payload
        }
    }
}

impl Default for EncryptBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for EncryptBenchmark {
    fn id(&self) -> &str {
        "crypto_encrypt"
    }

    fn description(&self) -> &str {
        "Measures AES-256-GCM encryption time for 1KB payload"
    }

    fn category(&self) -> &str {
        "crypto"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_crypto::{encrypt, Algorithm, SecretKey};

        let key = SecretKey::generate(Algorithm::Aes256Gcm).expect("Failed to generate key");
        let plaintext: Vec<u8> = (0..self.payload_size).map(|i| (i % 256) as u8).collect();

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = encrypt(&key, &plaintext, None);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = encrypt(&key, &plaintext, None);
        }
        let duration = start.elapsed();

        let bytes_processed = self.payload_size as u64 * self.iterations as u64;
        let bytes_per_sec = if duration.as_secs_f64() > 0.0 {
            bytes_processed as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("encrypt"))
        .with_metric("algorithm", serde_json::json!("AES-256-GCM"))
        .with_metric("payload_size_bytes", serde_json::json!(self.payload_size))
        .with_metric("throughput_bytes_per_sec", serde_json::json!(bytes_per_sec))
    }
}

/// Benchmark for decryption operations
pub struct DecryptBenchmark {
    iterations: u32,
    payload_size: usize,
}

impl DecryptBenchmark {
    pub fn new() -> Self {
        Self {
            iterations: 100,
            payload_size: 1024, // 1KB payload
        }
    }
}

impl Default for DecryptBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for DecryptBenchmark {
    fn id(&self) -> &str {
        "crypto_decrypt"
    }

    fn description(&self) -> &str {
        "Measures AES-256-GCM decryption time for 1KB payload"
    }

    fn category(&self) -> &str {
        "crypto"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_crypto::{decrypt, encrypt, Algorithm, SecretKey};

        let key = SecretKey::generate(Algorithm::Aes256Gcm).expect("Failed to generate key");
        let plaintext: Vec<u8> = (0..self.payload_size).map(|i| (i % 256) as u8).collect();

        // Create encrypted data
        let encrypted = encrypt(&key, &plaintext, None).expect("Failed to encrypt");

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = decrypt(&key, &encrypted);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = decrypt(&key, &encrypted);
        }
        let duration = start.elapsed();

        let bytes_processed = self.payload_size as u64 * self.iterations as u64;
        let bytes_per_sec = if duration.as_secs_f64() > 0.0 {
            bytes_processed as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("decrypt"))
        .with_metric("algorithm", serde_json::json!("AES-256-GCM"))
        .with_metric("payload_size_bytes", serde_json::json!(self.payload_size))
        .with_metric("throughput_bytes_per_sec", serde_json::json!(bytes_per_sec))
    }
}

/// Benchmark for key generation
pub struct KeyGenerationBenchmark {
    iterations: u32,
}

impl KeyGenerationBenchmark {
    pub fn new() -> Self {
        Self { iterations: 100 }
    }
}

impl Default for KeyGenerationBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTarget for KeyGenerationBenchmark {
    fn id(&self) -> &str {
        "crypto_keygen"
    }

    fn description(&self) -> &str {
        "Measures AES-256-GCM key generation time"
    }

    fn category(&self) -> &str {
        "crypto"
    }

    fn run(&self) -> BenchmarkResult {
        use llm_config_crypto::{Algorithm, SecretKey};

        // Warmup
        for _ in 0..self.warmup_iterations() {
            let _ = SecretKey::generate(Algorithm::Aes256Gcm);
        }

        // Measure
        let start = Instant::now();
        for _ in 0..self.iterations {
            let _ = SecretKey::generate(Algorithm::Aes256Gcm);
        }
        let duration = start.elapsed();

        BenchmarkResult::throughput(
            self.id(),
            duration.as_nanos(),
            self.iterations as u64,
        )
        .with_metric("operation", serde_json::json!("keygen"))
        .with_metric("algorithm", serde_json::json!("AES-256-GCM"))
        .with_metric("key_size_bits", serde_json::json!(256))
    }
}
