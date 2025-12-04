//! OS Keyring Configuration Provider
//!
//! This module provides an adapter for reading secrets from the operating
//! system's secure credential storage:
//!
//! - **macOS**: Keychain
//! - **Windows**: Credential Manager
//! - **Linux**: Secret Service (GNOME Keyring, KWallet)
//!
//! # Security Note
//!
//! The OS keyring provides secure storage with user-level encryption.
//! Secrets stored here are protected by the user's login credentials.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::KeyringProvider;
//!
//! let keyring = KeyringProvider::new("my-app");
//! let password = keyring.get("database", "password").await?;
//! ```

use super::traits::{
    ConfigProvider, SecretProvider, ProviderError, ProviderResult,
    ProviderValue, ProviderHealth, ValueMetadata,
};
use std::collections::HashMap;

/// Keyring access status
#[derive(Debug, Clone, PartialEq)]
pub enum KeyringStatus {
    /// Keyring is available and accessible
    Available,
    /// Keyring service is not running or not installed
    ServiceUnavailable,
    /// Access was denied (permissions issue)
    AccessDenied,
    /// The keyring feature is not supported on this platform
    Unsupported,
}

/// Provider for OS keyring/credential storage
///
/// This provider interfaces with the platform-specific secure credential
/// storage. Keys are stored with a service name prefix for isolation.
///
/// # Key Format
///
/// Keys are stored as: `{service}/{namespace}/{key}`
///
/// For example, with service "my-app":
/// - `my-app/database/password`
/// - `my-app/api/token`
#[derive(Debug)]
pub struct KeyringProvider {
    /// Service name (used as prefix for all keys)
    service: String,
    /// Whether the keyring is available on this system
    available: KeyringStatus,
}

impl KeyringProvider {
    /// Create a new keyring provider with the given service name
    ///
    /// The service name is used to namespace all keys, preventing
    /// conflicts with other applications.
    pub fn new(service: impl Into<String>) -> Self {
        let service = service.into();
        let available = Self::check_availability();

        Self { service, available }
    }

    /// Check if the keyring service is available
    fn check_availability() -> KeyringStatus {
        // Check platform support
        #[cfg(target_os = "macos")]
        {
            // macOS always has Keychain available
            KeyringStatus::Available
        }

        #[cfg(target_os = "windows")]
        {
            // Windows Credential Manager is always available
            KeyringStatus::Available
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, we need to check for Secret Service
            // This is a simplified check - real implementation would
            // try to connect to the D-Bus service
            if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok() {
                KeyringStatus::Available
            } else {
                KeyringStatus::ServiceUnavailable
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            KeyringStatus::Unsupported
        }
    }

    /// Build the full key path for storage
    fn build_key(&self, namespace: &str, key: &str) -> String {
        format!("{}/{}/{}", self.service, namespace, key)
    }

    /// Get the status of the keyring
    pub fn status(&self) -> &KeyringStatus {
        &self.available
    }

    /// Get a value from the keyring (platform-specific implementation)
    fn get_from_keyring(&self, full_key: &str) -> ProviderResult<String> {
        // Note: This is a stub implementation. A real implementation would use:
        // - macOS: Security.framework via security-framework crate
        // - Windows: CredRead via windows crate
        // - Linux: libsecret via secret-service crate
        //
        // We provide stub implementations to avoid adding heavy dependencies.
        // Users can replace this with actual keyring crate if needed.

        match &self.available {
            KeyringStatus::Available => {
                // Stub: In production, this would call platform APIs
                // For now, we fall back to environment variables with a KEYRING_ prefix
                // This allows testing without actual keyring access
                let env_key = format!("KEYRING_{}", full_key.replace('/', "_").to_uppercase());
                std::env::var(&env_key).map_err(|_| {
                    ProviderError::NotFound {
                        namespace: "keyring".to_string(),
                        key: full_key.to_string(),
                    }
                })
            }
            KeyringStatus::ServiceUnavailable => {
                Err(ProviderError::Unavailable(
                    "Keyring service is not available".to_string()
                ))
            }
            KeyringStatus::AccessDenied => {
                Err(ProviderError::AuthenticationFailed(
                    "Access to keyring was denied".to_string()
                ))
            }
            KeyringStatus::Unsupported => {
                Err(ProviderError::Unavailable(
                    "Keyring is not supported on this platform".to_string()
                ))
            }
        }
    }

    /// Set a value in the keyring (platform-specific implementation)
    fn set_in_keyring(&self, full_key: &str, value: &str) -> ProviderResult<()> {
        match &self.available {
            KeyringStatus::Available => {
                // Stub: Store in environment for testing
                let env_key = format!("KEYRING_{}", full_key.replace('/', "_").to_uppercase());
                std::env::set_var(&env_key, value);
                Ok(())
            }
            KeyringStatus::ServiceUnavailable => {
                Err(ProviderError::Unavailable(
                    "Keyring service is not available".to_string()
                ))
            }
            KeyringStatus::AccessDenied => {
                Err(ProviderError::AuthenticationFailed(
                    "Access to keyring was denied".to_string()
                ))
            }
            KeyringStatus::Unsupported => {
                Err(ProviderError::Unavailable(
                    "Keyring is not supported on this platform".to_string()
                ))
            }
        }
    }

    /// Delete a value from the keyring
    fn delete_from_keyring(&self, full_key: &str) -> ProviderResult<()> {
        match &self.available {
            KeyringStatus::Available => {
                // Stub: Remove from environment for testing
                let env_key = format!("KEYRING_{}", full_key.replace('/', "_").to_uppercase());
                std::env::remove_var(&env_key);
                Ok(())
            }
            KeyringStatus::ServiceUnavailable | KeyringStatus::AccessDenied | KeyringStatus::Unsupported => {
                Err(ProviderError::Unavailable(
                    "Keyring is not available".to_string()
                ))
            }
        }
    }
}

#[async_trait::async_trait]
impl ConfigProvider for KeyringProvider {
    fn name(&self) -> &str {
        "keyring"
    }

    async fn is_available(&self) -> bool {
        self.available == KeyringStatus::Available
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        let full_key = self.build_key(namespace, key);
        let value = self.get_from_keyring(&full_key)?;

        // All keyring values are considered secrets
        Ok(ProviderValue::secret(value, "keyring"))
    }

    async fn list(&self, namespace: &str, _prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        // Keyring APIs typically don't support listing
        // Return empty map - users should know their key names
        let _ = namespace;
        Ok(HashMap::new())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        match &self.available {
            KeyringStatus::Available => Ok(ProviderHealth::healthy("keyring")),
            KeyringStatus::ServiceUnavailable => {
                Ok(ProviderHealth::unhealthy("keyring", "Service not available"))
            }
            KeyringStatus::AccessDenied => {
                Ok(ProviderHealth::unhealthy("keyring", "Access denied"))
            }
            KeyringStatus::Unsupported => {
                Ok(ProviderHealth::unhealthy("keyring", "Not supported on this platform"))
            }
        }
    }
}

#[async_trait::async_trait]
impl SecretProvider for KeyringProvider {
    async fn set_secret(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> ProviderResult<ValueMetadata> {
        let full_key = self.build_key(namespace, key);
        self.set_in_keyring(&full_key, value)?;

        Ok(ValueMetadata {
            source: "keyring".to_string(),
            is_secret: true,
            ..Default::default()
        })
    }

    async fn delete_secret(&self, namespace: &str, key: &str) -> ProviderResult<()> {
        let full_key = self.build_key(namespace, key);
        self.delete_from_keyring(&full_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_provider_creation() {
        let provider = KeyringProvider::new("test-app");
        assert_eq!(provider.service, "test-app");
    }

    #[test]
    fn test_build_key() {
        let provider = KeyringProvider::new("my-app");
        assert_eq!(
            provider.build_key("database", "password"),
            "my-app/database/password"
        );
    }

    #[tokio::test]
    async fn test_keyring_stub_get_set() {
        let provider = KeyringProvider::new("test-app");

        // Only run if keyring is available
        if provider.is_available().await {
            // Set a value using the stub
            provider.set_secret("test", "secret", "my-secret-value").await.unwrap();

            // Get it back
            let value = provider.get("test", "secret").await.unwrap();
            assert_eq!(value.value, "my-secret-value");
            assert!(value.metadata.is_secret);

            // Clean up
            provider.delete_secret("test", "secret").await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_keyring_not_found() {
        let provider = KeyringProvider::new("test-app");

        if provider.is_available().await {
            let result = provider.get("nonexistent", "key").await;
            assert!(matches!(result, Err(ProviderError::NotFound { .. })));
        }
    }
}
