//! Provider Chain
//!
//! This module provides a mechanism to combine multiple configuration
//! providers into a priority-ordered chain. When fetching a value, providers
//! are tried in order until one returns a value.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::{ProviderChain, EnvProvider, JsonProvider};
//!
//! // Create a chain with environment variables taking priority over JSON file
//! let chain = ProviderChain::new()
//!     .with_provider(EnvProvider::new())        // Highest priority
//!     .with_provider(JsonProvider::from_file("config.json")?);  // Fallback
//!
//! // First provider to return a value wins
//! let value = chain.get("database", "host").await?;
//! ```

use super::traits::{ConfigProvider, ProviderError, ProviderResult, ProviderValue, ProviderHealth};
use std::collections::HashMap;
use std::sync::Arc;

/// A chain of configuration providers with priority ordering
///
/// Providers are tried in the order they were added. The first provider
/// to return a value (not `NotFound`) wins. Other errors stop the chain.
#[derive(Default)]
pub struct ProviderChain {
    providers: Vec<Arc<dyn ConfigProvider>>,
}

impl std::fmt::Debug for ProviderChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderChain")
            .field("providers", &self.providers.iter().map(|p| p.name()).collect::<Vec<_>>())
            .finish()
    }
}

impl ProviderChain {
    /// Create a new empty provider chain
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a provider to the chain (builder pattern)
    ///
    /// Providers added first have higher priority.
    pub fn with_provider<P: ConfigProvider + 'static>(mut self, provider: P) -> Self {
        self.providers.push(Arc::new(provider));
        self
    }

    /// Add a provider to the chain
    pub fn add_provider<P: ConfigProvider + 'static>(&mut self, provider: P) {
        self.providers.push(Arc::new(provider));
    }

    /// Add a pre-wrapped Arc provider
    pub fn add_arc_provider(&mut self, provider: Arc<dyn ConfigProvider>) {
        self.providers.push(provider);
    }

    /// Get the number of providers in the chain
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Get a list of provider names in priority order
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    /// Check health of all providers
    pub fn health_check_all(&self) -> Vec<ProviderHealth> {
        self.providers
            .iter()
            .filter_map(|p| p.health_check().ok())
            .collect()
    }

    /// Get health status summary
    pub fn health_summary(&self) -> ChainHealthSummary {
        let statuses = self.health_check_all();
        let total = statuses.len();
        let healthy = statuses.iter().filter(|h| h.healthy).count();

        ChainHealthSummary {
            total_providers: total,
            healthy_providers: healthy,
            unhealthy_providers: total - healthy,
            providers: statuses,
        }
    }
}

/// Health summary for the entire provider chain
#[derive(Debug, Clone)]
pub struct ChainHealthSummary {
    pub total_providers: usize,
    pub healthy_providers: usize,
    pub unhealthy_providers: usize,
    pub providers: Vec<ProviderHealth>,
}

impl ChainHealthSummary {
    /// Check if all providers are healthy
    pub fn all_healthy(&self) -> bool {
        self.unhealthy_providers == 0
    }

    /// Check if at least one provider is healthy
    pub fn any_healthy(&self) -> bool {
        self.healthy_providers > 0
    }
}

#[async_trait::async_trait]
impl ConfigProvider for ProviderChain {
    fn name(&self) -> &str {
        "chain"
    }

    async fn is_available(&self) -> bool {
        // Chain is available if any provider is available
        for provider in &self.providers {
            if provider.is_available().await {
                return true;
            }
        }
        false
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        if self.providers.is_empty() {
            return Err(ProviderError::Unavailable(
                "No providers configured in chain".to_string()
            ));
        }

        let mut last_error = None;

        for provider in &self.providers {
            match provider.get(namespace, key).await {
                Ok(value) => return Ok(value),
                Err(ProviderError::NotFound { .. }) => {
                    // Try next provider
                    continue;
                }
                Err(e) => {
                    // Log error but continue to next provider
                    tracing::debug!(
                        provider = provider.name(),
                        namespace = namespace,
                        key = key,
                        error = %e,
                        "Provider returned error, trying next"
                    );
                    last_error = Some(e);
                }
            }
        }

        // No provider had the value
        Err(last_error.unwrap_or_else(|| ProviderError::NotFound {
            namespace: namespace.to_string(),
            key: key.to_string(),
        }))
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        let mut result = HashMap::new();

        // Collect from all providers, later providers override earlier ones
        // (reverse priority - first provider's values take precedence)
        for provider in self.providers.iter().rev() {
            if let Ok(values) = provider.list(namespace, prefix).await {
                result.extend(values);
            }
        }

        Ok(result)
    }

    async fn exists(&self, namespace: &str, key: &str) -> ProviderResult<bool> {
        for provider in &self.providers {
            match provider.exists(namespace, key).await {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(ProviderError::NotFound { .. }) => continue,
                Err(_) => continue, // Skip errors, try next
            }
        }
        Ok(false)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        for provider in &self.providers {
            // Refresh all providers, ignore individual errors
            let _ = provider.refresh().await;
        }
        Ok(())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        let summary = self.health_summary();
        if summary.any_healthy() {
            Ok(ProviderHealth::healthy("chain"))
        } else {
            Ok(ProviderHealth::unhealthy("chain", "No healthy providers"))
        }
    }
}

/// Builder for creating provider chains with common patterns
pub struct ProviderChainBuilder {
    chain: ProviderChain,
}

impl ProviderChainBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            chain: ProviderChain::new(),
        }
    }

    /// Add environment variable provider
    pub fn with_env(self) -> Self {
        self.with_provider(super::env::EnvProvider::new())
    }

    /// Add environment variable provider with prefix
    pub fn with_prefixed_env(self, prefix: impl Into<String>) -> Self {
        self.with_provider(super::env::EnvProvider::with_prefix(prefix))
    }

    /// Add a .env file provider if the file exists
    pub fn with_dotenv_if_exists(self, path: impl AsRef<std::path::Path>) -> Self {
        match super::env::DotEnvProvider::from_file(path) {
            Ok(provider) => self.with_provider(provider),
            Err(_) => self,
        }
    }

    /// Add a JSON config file if it exists
    pub fn with_json_if_exists(self, path: impl AsRef<std::path::Path>) -> Self {
        match super::bundles::JsonProvider::from_file(path) {
            Ok(provider) => self.with_provider(provider),
            Err(_) => self,
        }
    }

    /// Add a TOML config file if it exists
    pub fn with_toml_if_exists(self, path: impl AsRef<std::path::Path>) -> Self {
        match super::bundles::TomlProvider::from_file(path) {
            Ok(provider) => self.with_provider(provider),
            Err(_) => self,
        }
    }

    /// Add a YAML config file if it exists
    pub fn with_yaml_if_exists(self, path: impl AsRef<std::path::Path>) -> Self {
        match super::bundles::YamlProvider::from_file(path) {
            Ok(provider) => self.with_provider(provider),
            Err(_) => self,
        }
    }

    /// Add a keyring provider
    pub fn with_keyring(self, service: impl Into<String>) -> Self {
        self.with_provider(super::keyring::KeyringProvider::new(service))
    }

    /// Add any provider
    pub fn with_provider<P: ConfigProvider + 'static>(mut self, provider: P) -> Self {
        self.chain.add_provider(provider);
        self
    }

    /// Build the provider chain
    pub fn build(self) -> ProviderChain {
        self.chain
    }
}

impl Default for ProviderChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a standard development provider chain
///
/// This creates a chain suitable for local development:
/// 1. Environment variables (highest priority)
/// 2. .env file (if exists)
/// 3. config.yaml or config.toml (if exists)
pub fn development_chain() -> ProviderChain {
    ProviderChainBuilder::new()
        .with_env()
        .with_dotenv_if_exists(".env")
        .with_dotenv_if_exists(".env.local")
        .with_yaml_if_exists("config.yaml")
        .with_yaml_if_exists("config.yml")
        .with_toml_if_exists("config.toml")
        .with_json_if_exists("config.json")
        .build()
}

/// Create a standard production provider chain
///
/// This creates a chain suitable for production:
/// 1. Environment variables (highest priority)
/// 2. No file-based providers (secrets should come from env or secret managers)
pub fn production_chain() -> ProviderChain {
    ProviderChainBuilder::new()
        .with_env()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::env::EnvProvider;
    use super::super::bundles::JsonProvider;

    #[tokio::test]
    async fn test_empty_chain() {
        let chain = ProviderChain::new();
        assert!(chain.is_empty());

        let result = chain.get("ns", "key").await;
        assert!(matches!(result, Err(ProviderError::Unavailable(_))));
    }

    #[tokio::test]
    async fn test_chain_priority() {
        // Set up env var
        std::env::set_var("CHAIN_TEST__DATABASE__HOST", "from-env");

        // Create JSON provider with different value
        let json = JsonProvider::from_string(r#"{"database": {"host": "from-json"}}"#).unwrap();

        // Chain with env first (higher priority)
        let chain = ProviderChain::new()
            .with_provider(EnvProvider::with_prefix("CHAIN_TEST"))
            .with_provider(json);

        // Should get env value
        let value = chain.get("database", "host").await.unwrap();
        assert_eq!(value.value, "from-env");
        assert_eq!(value.metadata.source, "env");

        std::env::remove_var("CHAIN_TEST__DATABASE__HOST");
    }

    #[tokio::test]
    async fn test_chain_fallback() {
        // No env var set
        let json = JsonProvider::from_string(r#"{"database": {"host": "from-json"}}"#).unwrap();

        let chain = ProviderChain::new()
            .with_provider(EnvProvider::with_prefix("FALLBACK_TEST"))
            .with_provider(json);

        // Should fall back to JSON
        let value = chain.get("database", "host").await.unwrap();
        assert_eq!(value.value, "from-json");
        assert_eq!(value.metadata.source, "json");
    }

    #[tokio::test]
    async fn test_chain_not_found() {
        let json = JsonProvider::from_string(r#"{"database": {"host": "localhost"}}"#).unwrap();

        let chain = ProviderChain::new()
            .with_provider(EnvProvider::with_prefix("NF_TEST"))
            .with_provider(json);

        // Key doesn't exist in any provider
        let result = chain.get("nonexistent", "key").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_chain_list() {
        let json1 = JsonProvider::from_string(r#"{"db": {"host": "host1", "port": "5432"}}"#).unwrap();
        let json2 = JsonProvider::from_string(r#"{"db": {"host": "host2", "user": "admin"}}"#).unwrap();

        let chain = ProviderChain::new()
            .with_provider(json1)  // Higher priority
            .with_provider(json2);

        let values = chain.list("db", None).await.unwrap();

        // Should have all keys, with json1's values taking priority
        assert_eq!(values.len(), 3);
        assert_eq!(values.get("host").unwrap().value, "host1"); // From json1
        assert_eq!(values.get("port").unwrap().value, "5432");  // From json1
        assert_eq!(values.get("user").unwrap().value, "admin"); // From json2
    }

    #[tokio::test]
    async fn test_chain_exists() {
        let json = JsonProvider::from_string(r#"{"database": {"host": "localhost"}}"#).unwrap();

        let chain = ProviderChain::new()
            .with_provider(json);

        assert!(chain.exists("database", "host").await.unwrap());
        assert!(!chain.exists("database", "nonexistent").await.unwrap());
    }

    #[test]
    fn test_provider_names() {
        let json = JsonProvider::from_string(r#"{}"#).unwrap();
        let env = EnvProvider::new();

        let chain = ProviderChain::new()
            .with_provider(env)
            .with_provider(json);

        let names = chain.provider_names();
        assert_eq!(names, vec!["env", "json"]);
    }

    #[test]
    fn test_health_summary() {
        let json = JsonProvider::from_string(r#"{}"#).unwrap();
        let env = EnvProvider::new();

        let chain = ProviderChain::new()
            .with_provider(env)
            .with_provider(json);

        let summary = chain.health_summary();
        assert_eq!(summary.total_providers, 2);
        assert!(summary.any_healthy());
    }

    #[tokio::test]
    async fn test_builder() {
        std::env::set_var("BUILD_TEST__APP__NAME", "test-app");

        let chain = ProviderChainBuilder::new()
            .with_prefixed_env("BUILD_TEST")
            .build();

        let value = chain.get("app", "name").await.unwrap();
        assert_eq!(value.value, "test-app");

        std::env::remove_var("BUILD_TEST__APP__NAME");
    }

    #[test]
    fn test_development_chain() {
        let chain = development_chain();
        assert!(!chain.is_empty());
        assert!(chain.provider_names().contains(&"env"));
    }

    #[test]
    fn test_production_chain() {
        let chain = production_chain();
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.provider_names(), vec!["env"]);
    }
}
