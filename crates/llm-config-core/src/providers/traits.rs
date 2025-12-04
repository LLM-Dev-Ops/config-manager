//! Core traits for external configuration providers
//!
//! This module defines the fundamental traits that all configuration
//! providers must implement. The design follows the provider pattern,
//! allowing multiple sources to be composed and prioritized.

use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

/// Errors that can occur when interacting with configuration providers
#[derive(Error, Debug)]
pub enum ProviderError {
    /// The requested key was not found in this provider
    #[error("Key not found: {namespace}/{key}")]
    NotFound {
        namespace: String,
        key: String,
    },

    /// The provider is not available or not configured
    #[error("Provider not available: {0}")]
    Unavailable(String),

    /// Authentication or authorization failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Network or connection error (for remote providers)
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Configuration or parsing error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// I/O error (for file-based providers)
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Encryption/decryption error
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    /// Rate limiting or throttling
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// Timeout error
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Generic provider error
    #[error("Provider error: {0}")]
    Other(String),
}

/// Result type for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;

/// Metadata about a configuration value from a provider
#[derive(Debug, Clone, Default)]
pub struct ValueMetadata {
    /// The source provider that returned this value
    pub source: String,
    /// Whether the value is marked as a secret
    pub is_secret: bool,
    /// Version or revision identifier (if available)
    pub version: Option<String>,
    /// Last modified timestamp (if available)
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    /// Additional provider-specific metadata
    pub extra: HashMap<String, String>,
}

/// A configuration value with associated metadata
#[derive(Debug, Clone)]
pub struct ProviderValue {
    /// The raw string value
    pub value: String,
    /// Metadata about the value
    pub metadata: ValueMetadata,
}

impl ProviderValue {
    /// Create a new provider value with minimal metadata
    pub fn new(value: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            metadata: ValueMetadata {
                source: source.into(),
                ..Default::default()
            },
        }
    }

    /// Create a secret value
    pub fn secret(value: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            metadata: ValueMetadata {
                source: source.into(),
                is_secret: true,
                ..Default::default()
            },
        }
    }

    /// Add version metadata
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.metadata.version = Some(version.into());
        self
    }

    /// Add last modified metadata
    pub fn with_last_modified(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.metadata.last_modified = Some(timestamp);
        self
    }
}

/// Core trait for configuration providers
///
/// A `ConfigProvider` can fetch configuration values from an external source.
/// Implementations should be lightweight and thread-safe.
///
/// # Async Design
///
/// All methods are async to support remote providers (cloud services, etc.)
/// without blocking. Local providers can simply return immediately.
///
/// # Error Handling
///
/// Providers should return `ProviderError::NotFound` when a key doesn't exist,
/// allowing provider chains to try the next provider. Other errors indicate
/// a problem that should stop the chain.
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync + fmt::Debug {
    /// Returns the unique name of this provider
    fn name(&self) -> &str;

    /// Check if the provider is available and properly configured
    async fn is_available(&self) -> bool {
        true
    }

    /// Get a single configuration value
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace or path prefix for the key
    /// * `key` - The configuration key name
    ///
    /// # Returns
    ///
    /// The configuration value with metadata, or `ProviderError::NotFound`
    /// if the key doesn't exist in this provider.
    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue>;

    /// Get multiple configuration values by prefix
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace to list values from
    /// * `prefix` - Optional key prefix filter
    ///
    /// # Returns
    ///
    /// A map of key names to values. Default implementation returns empty map.
    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        let _ = (namespace, prefix);
        Ok(HashMap::new())
    }

    /// Check if a key exists without fetching its value
    ///
    /// Default implementation tries to get the value and checks for NotFound error.
    async fn exists(&self, namespace: &str, key: &str) -> ProviderResult<bool> {
        match self.get(namespace, key).await {
            Ok(_) => Ok(true),
            Err(ProviderError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Refresh any cached data from the underlying source
    ///
    /// Default implementation does nothing (for providers without caching).
    async fn refresh(&self) -> ProviderResult<()> {
        Ok(())
    }

    /// Get provider-specific health/status information
    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        Ok(ProviderHealth::healthy(self.name()))
    }
}

/// Trait for providers that can also store/write secrets
///
/// This extends `ConfigProvider` for providers that support write operations.
/// Not all providers support this - environment variables are read-only,
/// while cloud secret managers typically support full CRUD operations.
#[async_trait::async_trait]
pub trait SecretProvider: ConfigProvider {
    /// Store a secret value
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace for the secret
    /// * `key` - The secret key name
    /// * `value` - The secret value (will be encrypted by the provider)
    ///
    /// # Returns
    ///
    /// Metadata about the stored secret, or an error.
    async fn set_secret(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> ProviderResult<ValueMetadata>;

    /// Delete a secret
    async fn delete_secret(&self, namespace: &str, key: &str) -> ProviderResult<()>;

    /// Rotate a secret (generate new value and store it)
    ///
    /// Default implementation returns an error - override for providers
    /// that support automatic rotation.
    async fn rotate_secret(&self, _namespace: &str, _key: &str) -> ProviderResult<ValueMetadata> {
        Err(ProviderError::Other("Secret rotation not supported".into()))
    }
}

/// Health status for a provider
#[derive(Debug, Clone)]
pub struct ProviderHealth {
    /// Provider name
    pub provider: String,
    /// Whether the provider is healthy
    pub healthy: bool,
    /// Optional status message
    pub message: Option<String>,
    /// Response time in milliseconds (for remote providers)
    pub latency_ms: Option<u64>,
}

impl ProviderHealth {
    /// Create a healthy status
    pub fn healthy(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            healthy: true,
            message: None,
            latency_ms: None,
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            healthy: false,
            message: Some(message.into()),
            latency_ms: None,
        }
    }

    /// Add latency information
    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = Some(ms);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_value_creation() {
        let value = ProviderValue::new("test_value", "env");
        assert_eq!(value.value, "test_value");
        assert_eq!(value.metadata.source, "env");
        assert!(!value.metadata.is_secret);
    }

    #[test]
    fn test_secret_value_creation() {
        let value = ProviderValue::secret("secret_value", "keyring");
        assert_eq!(value.value, "secret_value");
        assert!(value.metadata.is_secret);
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::NotFound {
            namespace: "app".into(),
            key: "db_password".into(),
        };
        assert!(err.to_string().contains("app/db_password"));
    }

    #[test]
    fn test_provider_health() {
        let health = ProviderHealth::healthy("test_provider");
        assert!(health.healthy);

        let unhealthy = ProviderHealth::unhealthy("test_provider", "Connection failed");
        assert!(!unhealthy.healthy);
        assert_eq!(unhealthy.message.unwrap(), "Connection failed");
    }
}
