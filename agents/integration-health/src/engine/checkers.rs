//! Health checker implementations
//!
//! Deterministic checkers for various adapter types.

use crate::contracts::*;
use crate::engine::HealthChecker;
use std::time::Instant;

/// HTTP health checker
pub struct HttpChecker;

impl HealthChecker for HttpChecker {
    fn id(&self) -> &str {
        "http"
    }

    fn supports(&self, adapter_type: &AdapterType) -> bool {
        matches!(adapter_type, AdapterType::Http | AdapterType::Grpc)
    }

    fn check(
        &self,
        adapter: AdapterConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AdapterHealthResult> + Send>> {
        Box::pin(async move {
            let start = Instant::now();
            let health_path = adapter
                .health_path
                .as_deref()
                .or_else(|| adapter.adapter_type.default_health_path())
                .unwrap_or("/health");

            let url = if adapter.endpoint.starts_with("http") {
                format!("{}{}", adapter.endpoint, health_path)
            } else {
                format!("https://{}{}", adapter.endpoint, health_path)
            };

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build();

            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    return AdapterHealthResult::unhealthy(
                        &adapter.id,
                        adapter.adapter_type,
                        format!("Failed to create HTTP client: {}", e),
                    );
                }
            };

            match client.get(&url).send().await {
                Ok(response) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let status = response.status();

                    if status.is_success() {
                        AdapterHealthResult::healthy(&adapter.id, adapter.adapter_type, latency)
                    } else if status.is_server_error() {
                        AdapterHealthResult::unhealthy(
                            &adapter.id,
                            adapter.adapter_type,
                            format!("Server error: {}", status),
                        )
                    } else {
                        AdapterHealthResult::degraded(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                            format!("Non-success status: {}", status),
                        )
                    }
                }
                Err(e) => AdapterHealthResult::unhealthy(
                    &adapter.id,
                    adapter.adapter_type,
                    format!("HTTP request failed: {}", e),
                ),
            }
        })
    }
}

/// TCP connectivity checker
pub struct TcpChecker;

impl HealthChecker for TcpChecker {
    fn id(&self) -> &str {
        "tcp"
    }

    fn supports(&self, adapter_type: &AdapterType) -> bool {
        matches!(
            adapter_type,
            AdapterType::Redis
                | AdapterType::Postgres
                | AdapterType::Mysql
                | AdapterType::Kafka
                | AdapterType::Rabbitmq
                | AdapterType::Tcp
        )
    }

    fn check(
        &self,
        adapter: AdapterConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AdapterHealthResult> + Send>> {
        Box::pin(async move {
            let start = Instant::now();

            // Parse endpoint
            let addr = if adapter.endpoint.contains(':') {
                adapter.endpoint.clone()
            } else {
                let port = adapter
                    .adapter_type
                    .default_port()
                    .unwrap_or(80);
                format!("{}:{}", adapter.endpoint, port)
            };

            match tokio::net::TcpStream::connect(&addr).await {
                Ok(_) => {
                    let latency = start.elapsed().as_millis() as u64;
                    AdapterHealthResult::healthy(&adapter.id, adapter.adapter_type, latency)
                }
                Err(e) => AdapterHealthResult::unhealthy(
                    &adapter.id,
                    adapter.adapter_type,
                    format!("TCP connection failed: {}", e),
                ),
            }
        })
    }
}

/// HashiCorp Vault health checker
pub struct VaultChecker;

impl HealthChecker for VaultChecker {
    fn id(&self) -> &str {
        "vault"
    }

    fn supports(&self, adapter_type: &AdapterType) -> bool {
        matches!(adapter_type, AdapterType::HashicorpVault)
    }

    fn check(
        &self,
        adapter: AdapterConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AdapterHealthResult> + Send>> {
        Box::pin(async move {
            let start = Instant::now();

            let health_path = adapter
                .health_path
                .as_deref()
                .unwrap_or("/v1/sys/health");

            let url = if adapter.endpoint.starts_with("http") {
                format!("{}{}", adapter.endpoint, health_path)
            } else {
                format!("https://{}{}", adapter.endpoint, health_path)
            };

            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    return AdapterHealthResult::unhealthy(
                        &adapter.id,
                        adapter.adapter_type,
                        format!("Failed to create HTTP client: {}", e),
                    );
                }
            };

            match client.get(&url).send().await {
                Ok(response) => {
                    let latency = start.elapsed().as_millis() as u64;
                    let status = response.status();

                    // Vault returns specific status codes
                    match status.as_u16() {
                        200 => AdapterHealthResult::healthy(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                        ),
                        429 => AdapterHealthResult::degraded(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                            "Vault is unsealed but in standby",
                        ),
                        472 => AdapterHealthResult::degraded(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                            "Vault is in recovery mode",
                        ),
                        473 => AdapterHealthResult::degraded(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                            "Vault is in performance standby",
                        ),
                        501 => AdapterHealthResult::unhealthy(
                            &adapter.id,
                            adapter.adapter_type,
                            "Vault is not initialized",
                        ),
                        503 => AdapterHealthResult::unhealthy(
                            &adapter.id,
                            adapter.adapter_type,
                            "Vault is sealed",
                        ),
                        _ => AdapterHealthResult::degraded(
                            &adapter.id,
                            adapter.adapter_type,
                            latency,
                            format!("Unexpected status: {}", status),
                        ),
                    }
                }
                Err(e) => AdapterHealthResult::unhealthy(
                    &adapter.id,
                    adapter.adapter_type,
                    format!("Vault health check failed: {}", e),
                ),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_adapter(adapter_type: AdapterType, endpoint: &str) -> AdapterConfig {
        AdapterConfig {
            id: "test-adapter".to_string(),
            adapter_type,
            endpoint: endpoint.to_string(),
            auth: None,
            health_path: None,
            properties: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_http_checker_supports() {
        let checker = HttpChecker;
        assert!(checker.supports(&AdapterType::Http));
        assert!(checker.supports(&AdapterType::Grpc));
        assert!(!checker.supports(&AdapterType::Redis));
    }

    #[tokio::test]
    async fn test_tcp_checker_supports() {
        let checker = TcpChecker;
        assert!(checker.supports(&AdapterType::Redis));
        assert!(checker.supports(&AdapterType::Postgres));
        assert!(!checker.supports(&AdapterType::Http));
    }

    #[tokio::test]
    async fn test_vault_checker_supports() {
        let checker = VaultChecker;
        assert!(checker.supports(&AdapterType::HashicorpVault));
        assert!(!checker.supports(&AdapterType::Http));
    }
}
