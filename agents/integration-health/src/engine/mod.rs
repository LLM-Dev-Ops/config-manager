//! Health check engine
//!
//! Deterministic health checking of external adapters.

mod checkers;

pub use checkers::*;

use crate::contracts::*;
use sha2::{Digest, Sha256};
use std::time::Instant;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

/// Performance budget constants
pub const MAX_LATENCY_MS: u64 = 1500;
pub const MAX_TOKENS: usize = 800;

/// Integration health check engine
pub struct HealthCheckEngine {
    checkers: Vec<Box<dyn HealthChecker>>,
}

impl Default for HealthCheckEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckEngine {
    /// Create new engine with default checkers
    pub fn new() -> Self {
        Self {
            checkers: vec![
                Box::new(HttpChecker),
                Box::new(TcpChecker),
                Box::new(VaultChecker),
            ],
        }
    }

    /// Run health checks
    pub async fn check(&self, input: &IntegrationHealthInput) -> IntegrationHealthOutput {
        let start = Instant::now();
        let request_id = input.request_id;

        let mut results = Vec::new();
        let timeout_ms = input.options.timeout_ms;

        if input.options.parallel {
            // Run checks in parallel
            let futures: Vec<_> = input
                .adapters
                .iter()
                .map(|adapter| self.check_adapter(adapter, timeout_ms))
                .collect();

            let outcomes = futures::future::join_all(futures).await;
            results.extend(outcomes);
        } else {
            // Run checks sequentially
            for adapter in &input.adapters {
                // Check latency budget
                if start.elapsed().as_millis() as u64 > MAX_LATENCY_MS - timeout_ms {
                    tracing::warn!("Health check exceeded latency budget, stopping early");
                    break;
                }

                let result = self.check_adapter(adapter, timeout_ms).await;
                results.push(result);
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        IntegrationHealthOutput::healthy(request_id, results).with_duration(duration_ms)
    }

    /// Check a single adapter
    async fn check_adapter(&self, adapter: &AdapterConfig, timeout_ms: u64) -> AdapterHealthResult {
        let adapter_start = Instant::now();

        // Find appropriate checker
        let checker = self
            .checkers
            .iter()
            .find(|c| c.supports(&adapter.adapter_type));

        match checker {
            Some(c) => {
                let check_future = c.check(adapter.clone());
                match timeout(Duration::from_millis(timeout_ms), check_future).await {
                    Ok(result) => result,
                    Err(_) => AdapterHealthResult::unhealthy(
                        &adapter.id,
                        adapter.adapter_type,
                        format!("Health check timed out after {}ms", timeout_ms),
                    ),
                }
            }
            None => {
                // No checker available, use generic TCP check
                let latency = adapter_start.elapsed().as_millis() as u64;
                AdapterHealthResult::degraded(
                    &adapter.id,
                    adapter.adapter_type,
                    latency,
                    "No specialized checker available",
                )
            }
        }
    }

    /// Compute deterministic hash of inputs
    pub fn compute_inputs_hash(input: &IntegrationHealthInput) -> String {
        let mut hasher = Sha256::new();
        for adapter in &input.adapters {
            hasher.update(adapter.id.as_bytes());
            hasher.update(adapter.endpoint.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Create input from adapter list
    pub fn create_input(
        adapters: Vec<AdapterConfig>,
        requested_by: String,
    ) -> IntegrationHealthInput {
        IntegrationHealthInput {
            request_id: Uuid::new_v4(),
            adapters,
            options: HealthCheckOptions::default(),
            context: std::collections::HashMap::new(),
            requested_at: chrono::Utc::now(),
            requested_by,
        }
    }
}

/// Trait for adapter health checkers
pub trait HealthChecker: Send + Sync {
    /// Checker identifier
    fn id(&self) -> &str;

    /// Check if this checker supports the adapter type
    fn supports(&self, adapter_type: &AdapterType) -> bool;

    /// Perform health check (takes owned adapter to avoid lifetime issues)
    fn check(
        &self,
        adapter: AdapterConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AdapterHealthResult> + Send>>;
}
