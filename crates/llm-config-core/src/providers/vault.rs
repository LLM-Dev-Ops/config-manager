//! HashiCorp Vault Provider
//!
//! This module provides an adapter for consuming secrets from HashiCorp Vault,
//! a popular secrets management and data protection tool.
//!
//! # Supported Auth Methods
//!
//! - **Token**: Direct token authentication (VAULT_TOKEN)
//! - **AppRole**: Machine-friendly authentication
//! - **Kubernetes**: Pod-based authentication for K8s workloads
//!
//! # Secret Engines
//!
//! Currently supports the KV secrets engine (v1 and v2):
//! - v1: Simple key-value storage
//! - v2: Versioned key-value storage with metadata
//!
//! # Path Convention
//!
//! Secrets are accessed as: `{mount}/{namespace}/{key}`
//!
//! For example:
//! - `secret/data/production/database` (KV v2)
//! - `secret/staging/api-key` (KV v1)
//!
//! # Stub Implementation
//!
//! This is a stub interface that falls back to environment variables with
//! the pattern `VAULT_{NAMESPACE}_{KEY}` for local development. Replace
//! with actual HTTP calls to Vault for production use.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::{VaultProvider, VaultConfig};
//!
//! let config = VaultConfig::from_env();
//! let vault = VaultProvider::new(config)?;
//! let secret = vault.get("production", "database/password").await?;
//! ```

use super::traits::{
    ConfigProvider, SecretProvider, ProviderError, ProviderResult,
    ProviderValue, ProviderHealth, ValueMetadata,
};
use std::collections::HashMap;
use std::time::Duration;

/// Authentication method for Vault
#[derive(Debug, Clone)]
pub enum VaultAuthMethod {
    /// Direct token authentication
    Token(String),
    /// AppRole authentication with role_id and secret_id
    AppRole {
        role_id: String,
        secret_id: String,
    },
    /// Kubernetes service account authentication
    Kubernetes {
        role: String,
        jwt_path: Option<String>,
    },
    /// No authentication configured (stub mode)
    None,
}

impl Default for VaultAuthMethod {
    fn default() -> Self {
        VaultAuthMethod::None
    }
}

/// Configuration for HashiCorp Vault provider
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Vault server address (e.g., "https://vault.example.com:8200")
    pub address: Option<String>,
    /// Authentication method
    pub auth: VaultAuthMethod,
    /// Secrets engine mount point (default: "secret")
    pub mount: String,
    /// KV engine version (1 or 2, default: 2)
    pub kv_version: u8,
    /// Namespace for Vault Enterprise (optional)
    pub namespace: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Skip TLS verification (not recommended for production)
    pub skip_tls_verify: bool,
    /// Custom CA certificate path
    pub ca_cert_path: Option<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            address: None,
            auth: VaultAuthMethod::None,
            mount: "secret".to_string(),
            kv_version: 2,
            namespace: None,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            skip_tls_verify: false,
            ca_cert_path: None,
        }
    }
}

impl VaultConfig {
    /// Load configuration from environment variables
    ///
    /// Reads:
    /// - VAULT_ADDR: Vault server address
    /// - VAULT_TOKEN: Authentication token
    /// - VAULT_ROLE_ID + VAULT_SECRET_ID: AppRole authentication
    /// - VAULT_NAMESPACE: Enterprise namespace
    /// - VAULT_MOUNT: Secrets engine mount point
    /// - VAULT_KV_VERSION: KV engine version (1 or 2)
    /// - VAULT_SKIP_VERIFY: Skip TLS verification
    /// - VAULT_CACERT: CA certificate path
    pub fn from_env() -> Self {
        let auth = if let Ok(token) = std::env::var("VAULT_TOKEN") {
            VaultAuthMethod::Token(token)
        } else if let (Ok(role_id), Ok(secret_id)) = (
            std::env::var("VAULT_ROLE_ID"),
            std::env::var("VAULT_SECRET_ID"),
        ) {
            VaultAuthMethod::AppRole { role_id, secret_id }
        } else if let Ok(role) = std::env::var("VAULT_K8S_ROLE") {
            VaultAuthMethod::Kubernetes {
                role,
                jwt_path: std::env::var("VAULT_K8S_JWT_PATH").ok(),
            }
        } else {
            VaultAuthMethod::None
        };

        Self {
            address: std::env::var("VAULT_ADDR").ok(),
            auth,
            mount: std::env::var("VAULT_MOUNT").unwrap_or_else(|_| "secret".to_string()),
            kv_version: std::env::var("VAULT_KV_VERSION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2),
            namespace: std::env::var("VAULT_NAMESPACE").ok(),
            skip_tls_verify: std::env::var("VAULT_SKIP_VERIFY")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
            ca_cert_path: std::env::var("VAULT_CACERT").ok(),
            ..Default::default()
        }
    }

    /// Set the Vault server address
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Set token authentication
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.auth = VaultAuthMethod::Token(token.into());
        self
    }

    /// Set AppRole authentication
    pub fn with_approle(mut self, role_id: impl Into<String>, secret_id: impl Into<String>) -> Self {
        self.auth = VaultAuthMethod::AppRole {
            role_id: role_id.into(),
            secret_id: secret_id.into(),
        };
        self
    }

    /// Set the secrets engine mount point
    pub fn with_mount(mut self, mount: impl Into<String>) -> Self {
        self.mount = mount.into();
        self
    }

    /// Set the KV engine version
    pub fn with_kv_version(mut self, version: u8) -> Self {
        self.kv_version = version;
        self
    }

    /// Set the Vault Enterprise namespace
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// HashiCorp Vault secrets provider
///
/// This provider reads secrets from HashiCorp Vault. It supports the KV
/// secrets engine (v1 and v2) and multiple authentication methods.
///
/// # Stub Implementation
///
/// Falls back to `VAULT_{NAMESPACE}_{KEY}` environment variables for
/// local development when Vault is not configured.
#[derive(Debug)]
pub struct VaultProvider {
    config: VaultConfig,
}

impl VaultProvider {
    /// Create a new Vault provider
    pub fn new(config: VaultConfig) -> ProviderResult<Self> {
        Ok(Self { config })
    }

    /// Create a provider from environment variables
    pub fn from_env() -> ProviderResult<Self> {
        Self::new(VaultConfig::from_env())
    }

    /// Build the Vault API path for a secret
    ///
    /// For KV v2: `{mount}/data/{namespace}/{key}`
    /// For KV v1: `{mount}/{namespace}/{key}`
    #[allow(dead_code)]
    fn build_path(&self, namespace: &str, key: &str) -> String {
        let data_prefix = if self.config.kv_version == 2 { "/data" } else { "" };
        format!(
            "{}{}/{}/{}",
            self.config.mount,
            data_prefix,
            namespace,
            key
        )
    }

    /// Stub: Get secret from environment variable fallback
    fn get_stub(&self, namespace: &str, key: &str) -> ProviderResult<String> {
        // For local development, fall back to env vars
        let env_key = format!(
            "VAULT_{}_{}",
            namespace.to_uppercase().replace('/', "_").replace('-', "_"),
            key.to_uppercase().replace('/', "_").replace('-', "_")
        );

        std::env::var(&env_key).map_err(|_| ProviderError::NotFound {
            namespace: namespace.to_string(),
            key: key.to_string(),
        })
    }

    /// Check if Vault is properly configured
    fn is_configured(&self) -> bool {
        self.config.address.is_some() && !matches!(self.config.auth, VaultAuthMethod::None)
    }
}

#[async_trait::async_trait]
impl ConfigProvider for VaultProvider {
    fn name(&self) -> &str {
        "vault"
    }

    async fn is_available(&self) -> bool {
        self.is_configured()
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        // Stub implementation - in production, use HTTP client:
        //
        // let url = format!("{}/v1/{}", self.config.address.as_ref().unwrap(), self.build_path(namespace, key));
        // let response = client.get(&url)
        //     .header("X-Vault-Token", token)
        //     .header("X-Vault-Namespace", namespace) // For Enterprise
        //     .send()
        //     .await?;
        //
        // For KV v2, the response structure is:
        // {
        //   "data": {
        //     "data": { "key": "value" },
        //     "metadata": { "version": 1, "created_time": "..." }
        //   }
        // }

        let value = self.get_stub(namespace, key)?;
        let path = self.build_path(namespace, key);

        Ok(ProviderValue::secret(value, "vault")
            .with_version(format!("path:{}", path)))
    }

    async fn list(&self, namespace: &str, _prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        // Stub: Vault LIST operation would be used here
        // GET {mount}/metadata/{namespace}?list=true (for KV v2)
        let _ = namespace;
        Ok(HashMap::new())
    }

    async fn exists(&self, namespace: &str, key: &str) -> ProviderResult<bool> {
        match self.get(namespace, key).await {
            Ok(_) => Ok(true),
            Err(ProviderError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.is_configured() {
            Ok(ProviderHealth::healthy("vault"))
        } else if self.config.address.is_some() {
            Ok(ProviderHealth::unhealthy("vault", "Authentication not configured"))
        } else {
            Ok(ProviderHealth::unhealthy("vault", "VAULT_ADDR not configured"))
        }
    }
}

#[async_trait::async_trait]
impl SecretProvider for VaultProvider {
    async fn set_secret(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> ProviderResult<ValueMetadata> {
        // Stub: In production, use HTTP POST to Vault
        //
        // For KV v2:
        // POST {mount}/data/{namespace}/{key}
        // Body: { "data": { "value": "..." } }
        //
        // For KV v1:
        // POST {mount}/{namespace}/{key}
        // Body: { "value": "..." }

        let env_key = format!(
            "VAULT_{}_{}",
            namespace.to_uppercase().replace('/', "_").replace('-', "_"),
            key.to_uppercase().replace('/', "_").replace('-', "_")
        );
        std::env::set_var(&env_key, value);

        Ok(ValueMetadata {
            source: "vault".to_string(),
            is_secret: true,
            version: Some("1".to_string()),
            ..Default::default()
        })
    }

    async fn delete_secret(&self, namespace: &str, key: &str) -> ProviderResult<()> {
        // Stub: In production, use HTTP DELETE
        //
        // For KV v2 (soft delete):
        // DELETE {mount}/data/{namespace}/{key}
        //
        // For permanent deletion:
        // DELETE {mount}/metadata/{namespace}/{key}

        let env_key = format!(
            "VAULT_{}_{}",
            namespace.to_uppercase().replace('/', "_").replace('-', "_"),
            key.to_uppercase().replace('/', "_").replace('-', "_")
        );
        std::env::remove_var(&env_key);
        Ok(())
    }

    async fn rotate_secret(&self, namespace: &str, key: &str) -> ProviderResult<ValueMetadata> {
        // Vault supports secret rotation through its dynamic secrets feature
        // For static secrets in KV, rotation is typically handled externally
        let _ = (namespace, key);

        Ok(ValueMetadata {
            source: "vault".to_string(),
            is_secret: true,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_config_from_env() {
        std::env::set_var("VAULT_ADDR", "https://vault.example.com:8200");
        std::env::set_var("VAULT_TOKEN", "s.test-token");
        std::env::set_var("VAULT_MOUNT", "kv");
        std::env::set_var("VAULT_KV_VERSION", "2");

        let config = VaultConfig::from_env();

        assert_eq!(config.address, Some("https://vault.example.com:8200".to_string()));
        assert!(matches!(config.auth, VaultAuthMethod::Token(_)));
        assert_eq!(config.mount, "kv");
        assert_eq!(config.kv_version, 2);

        std::env::remove_var("VAULT_ADDR");
        std::env::remove_var("VAULT_TOKEN");
        std::env::remove_var("VAULT_MOUNT");
        std::env::remove_var("VAULT_KV_VERSION");
    }

    #[test]
    fn test_vault_config_approle() {
        std::env::set_var("VAULT_ADDR", "https://vault.example.com:8200");
        std::env::set_var("VAULT_ROLE_ID", "role-123");
        std::env::set_var("VAULT_SECRET_ID", "secret-456");

        let config = VaultConfig::from_env();

        match config.auth {
            VaultAuthMethod::AppRole { role_id, secret_id } => {
                assert_eq!(role_id, "role-123");
                assert_eq!(secret_id, "secret-456");
            }
            _ => panic!("Expected AppRole auth method"),
        }

        std::env::remove_var("VAULT_ADDR");
        std::env::remove_var("VAULT_ROLE_ID");
        std::env::remove_var("VAULT_SECRET_ID");
    }

    #[test]
    fn test_vault_path_building() {
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_mount("secret")
            .with_kv_version(2);

        let provider = VaultProvider::new(config).unwrap();

        assert_eq!(
            provider.build_path("production", "database/password"),
            "secret/data/production/database/password"
        );
    }

    #[test]
    fn test_vault_path_kv_v1() {
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_mount("secret")
            .with_kv_version(1);

        let provider = VaultProvider::new(config).unwrap();

        assert_eq!(
            provider.build_path("production", "database/password"),
            "secret/production/database/password"
        );
    }

    #[tokio::test]
    async fn test_vault_stub() {
        std::env::set_var("VAULT_DATABASE_PASSWORD", "secret123");

        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_token("test-token");

        let provider = VaultProvider::new(config).unwrap();
        let value = provider.get("database", "password").await.unwrap();

        assert_eq!(value.value, "secret123");
        assert!(value.metadata.is_secret);
        assert_eq!(value.metadata.source, "vault");

        std::env::remove_var("VAULT_DATABASE_PASSWORD");
    }

    #[tokio::test]
    async fn test_vault_not_found() {
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_token("test-token");

        let provider = VaultProvider::new(config).unwrap();
        let result = provider.get("nonexistent", "key").await;

        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_vault_health_check() {
        // Unconfigured
        let config = VaultConfig::default();
        let provider = VaultProvider::new(config).unwrap();
        let health = provider.health_check().unwrap();
        assert!(!health.healthy);
        assert!(health.message.unwrap().contains("VAULT_ADDR"));

        // Address but no auth
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200");
        let provider = VaultProvider::new(config).unwrap();
        let health = provider.health_check().unwrap();
        assert!(!health.healthy);
        assert!(health.message.unwrap().contains("Authentication"));

        // Fully configured
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_token("test-token");
        let provider = VaultProvider::new(config).unwrap();
        let health = provider.health_check().unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_vault_secret_operations() {
        let config = VaultConfig::default()
            .with_address("https://vault.example.com:8200")
            .with_token("test-token");

        let provider = VaultProvider::new(config).unwrap();

        // Set secret
        let metadata = provider.set_secret("app", "api_key", "key123").await.unwrap();
        assert!(metadata.is_secret);
        assert_eq!(metadata.source, "vault");

        // Get secret
        let value = provider.get("app", "api_key").await.unwrap();
        assert_eq!(value.value, "key123");

        // Delete secret
        provider.delete_secret("app", "api_key").await.unwrap();

        // Verify deleted
        let result = provider.get("app", "api_key").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }
}
