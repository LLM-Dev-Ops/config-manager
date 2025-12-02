//! Canonical Benchmark Module for LLM Config Manager
//!
//! This module provides the standardized benchmark interface used across
//! all 25 benchmark-target repositories in the LLM-Dev-Ops ecosystem.
//!
//! The canonical benchmark interface requires:
//! - `run_all_benchmarks()` entrypoint returning `Vec<BenchmarkResult>`
//! - `BenchmarkResult` struct with `target_id: String`, `metrics: serde_json::Value`, `timestamp: DateTime<Utc>`
//! - Module files: `benchmarks/mod.rs`, `benchmarks/result.rs`, `benchmarks/markdown.rs`, `benchmarks/io.rs`
//! - Output directories: `benchmarks/output/`, `benchmarks/output/raw/`
//! - Summary file: `benchmarks/output/summary.md`
//! - Adapters module with `BenchTarget` trait and `all_targets()` registry
//!
//! # Usage
//!
//! ```rust,ignore
//! use llm_config_core::benchmarks::{run_all_benchmarks, BenchmarkResult};
//!
//! // Run all benchmarks
//! let results: Vec<BenchmarkResult> = run_all_benchmarks();
//!
//! // Each result has:
//! // - target_id: String (e.g., "config_get", "cache_l1_get")
//! // - metrics: serde_json::Value (flexible metrics structure)
//! // - timestamp: chrono::DateTime<chrono::Utc>
//! ```
//!
//! # CLI Integration
//!
//! The CLI provides a `run` subcommand for executing benchmarks:
//!
//! ```bash
//! # Run all benchmarks
//! llm-config run --all
//!
//! # Run benchmarks by category
//! llm-config run --category config
//!
//! # Run a specific benchmark
//! llm-config run --target config_get
//!
//! # List available benchmarks
//! llm-config run --list
//! ```

// Re-export the benchmark module from llm-config-core
pub use llm_config_core::benchmarks::*;
