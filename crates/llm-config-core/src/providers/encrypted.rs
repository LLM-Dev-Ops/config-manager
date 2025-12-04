//! Encrypted File Configuration Provider
//!
//! This module provides an adapter for loading configuration from local
//! encrypted files. It uses the existing llm-config-crypto encryption
//! primitives for secure storage.
//!
//! # File Format
//!
//! Encrypted files use a JSON structure wrapped with encryption metadata:
//! ```json
//! {
//!   "version": 1,
//!   "encrypted": {
//!     "algorithm": "aes-256-gcm",
//!     "nonce": "...",
//!     "ciphertext": "...",
//!     "key_version": 1
//!   }
//! }
//! ```
//!
//! The decrypted content is a JSON object mapping keys to values.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::EncryptedFileProvider;
//! use llm_config_crypto::SecretKey;
//!
//! let key = SecretKey::from_hex("...")?;
//! let provider = EncryptedFileProvider::new("secrets.enc", key)?;
//! let value = provider.get("database", "password").await?;
//! ```

use super::traits::{
    ConfigProvider, SecretProvider, ProviderError, ProviderResult,
    ProviderValue, ProviderHealth, ValueMetadata,
};
use llm_config_crypto::{decrypt, encrypt, EncryptedData, SecretKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// File format version for forward compatibility
const FILE_VERSION: u32 = 1;

/// Encrypted file structure
#[derive(Debug, Serialize, Deserialize)]
struct EncryptedFile {
    version: u32,
    encrypted: EncryptedData,
}

/// Decrypted configuration content
#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigContent {
    /// Namespace -> Key -> Value mapping
    #[serde(flatten)]
    namespaces: HashMap<String, HashMap<String, serde_json::Value>>,
}

impl ConfigContent {
    fn get(&self, namespace: &str, key: &str) -> Option<&serde_json::Value> {
        self.namespaces.get(namespace)?.get(key)
    }

    fn set(&mut self, namespace: &str, key: &str, value: serde_json::Value) {
        self.namespaces
            .entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    fn remove(&mut self, namespace: &str, key: &str) -> Option<serde_json::Value> {
        self.namespaces.get_mut(namespace)?.remove(key)
    }

    fn list_namespace(&self, namespace: &str) -> Option<&HashMap<String, serde_json::Value>> {
        self.namespaces.get(namespace)
    }
}

/// Provider for encrypted local configuration files
///
/// This provider loads configuration from an AES-256-GCM encrypted file.
/// The file is decrypted on first access and cached in memory.
///
/// # Thread Safety
///
/// The provider uses internal locking to ensure thread-safe access to
/// the cached configuration. Multiple reads can occur concurrently.
#[derive(Debug)]
pub struct EncryptedFileProvider {
    /// Path to the encrypted file
    path: PathBuf,
    /// Encryption key
    key: SecretKey,
    /// Cached decrypted content
    cache: RwLock<Option<ConfigContent>>,
    /// Whether auto-save is enabled on modifications
    auto_save: bool,
}

impl EncryptedFileProvider {
    /// Create a new encrypted file provider
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the encrypted configuration file
    /// * `key` - The encryption key to use for decryption
    ///
    /// # Returns
    ///
    /// A new provider instance, or an error if the file doesn't exist.
    pub fn new(path: impl AsRef<Path>, key: SecretKey) -> ProviderResult<Self> {
        let path = path.as_ref().to_path_buf();

        Ok(Self {
            path,
            key,
            cache: RwLock::new(None),
            auto_save: true,
        })
    }

    /// Create a new encrypted file, overwriting any existing file
    ///
    /// This creates an empty encrypted configuration file.
    pub fn create(path: impl AsRef<Path>, key: SecretKey) -> ProviderResult<Self> {
        let provider = Self {
            path: path.as_ref().to_path_buf(),
            key,
            cache: RwLock::new(Some(ConfigContent::default())),
            auto_save: true,
        };

        provider.save()?;
        Ok(provider)
    }

    /// Disable auto-save (useful for batch operations)
    pub fn with_auto_save(mut self, enabled: bool) -> Self {
        self.auto_save = enabled;
        self
    }

    /// Load and decrypt the configuration file
    fn load(&self) -> ProviderResult<ConfigContent> {
        if !self.path.exists() {
            return Ok(ConfigContent::default());
        }

        let content = std::fs::read_to_string(&self.path)?;

        let encrypted_file: EncryptedFile = serde_json::from_str(&content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        if encrypted_file.version > FILE_VERSION {
            return Err(ProviderError::ConfigurationError(format!(
                "Unsupported file version: {} (max supported: {})",
                encrypted_file.version, FILE_VERSION
            )));
        }

        let decrypted = decrypt(&self.key, &encrypted_file.encrypted)
            .map_err(|e| ProviderError::EncryptionError(e.to_string()))?;

        let config: ConfigContent = serde_json::from_slice(&decrypted)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        Ok(config)
    }

    /// Save the current configuration to the encrypted file
    pub fn save(&self) -> ProviderResult<()> {
        let cache = self.cache.read().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        let content = cache.as_ref().ok_or_else(|| {
            ProviderError::Other("No configuration loaded".to_string())
        })?;

        let json = serde_json::to_vec(content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let encrypted = encrypt(&self.key, &json, Some("config"))
            .map_err(|e| ProviderError::EncryptionError(e.to_string()))?;

        let file = EncryptedFile {
            version: FILE_VERSION,
            encrypted,
        };

        let output = serde_json::to_string_pretty(&file)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        // Write atomically using temp file
        let temp_path = self.path.with_extension("tmp");
        std::fs::write(&temp_path, output)?;
        std::fs::rename(&temp_path, &self.path)?;

        Ok(())
    }

    /// Ensure configuration is loaded into cache
    fn ensure_loaded(&self) -> ProviderResult<()> {
        let loaded = {
            let cache = self.cache.read().map_err(|e| {
                ProviderError::Other(format!("Failed to acquire lock: {}", e))
            })?;
            cache.is_some()
        };

        if !loaded {
            let content = self.load()?;
            let mut cache = self.cache.write().map_err(|e| {
                ProviderError::Other(format!("Failed to acquire lock: {}", e))
            })?;
            *cache = Some(content);
        }

        Ok(())
    }

    /// Convert a JSON value to string
    fn value_to_string(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ConfigProvider for EncryptedFileProvider {
    fn name(&self) -> &str {
        "encrypted_file"
    }

    async fn is_available(&self) -> bool {
        self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false)
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        self.ensure_loaded()?;

        let cache = self.cache.read().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        let content = cache.as_ref().ok_or_else(|| {
            ProviderError::Other("Configuration not loaded".to_string())
        })?;

        match content.get(namespace, key) {
            Some(value) => {
                // All values from encrypted files are considered secrets
                Ok(ProviderValue::secret(
                    Self::value_to_string(value),
                    "encrypted_file",
                ))
            }
            None => Err(ProviderError::NotFound {
                namespace: namespace.to_string(),
                key: key.to_string(),
            }),
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        self.ensure_loaded()?;

        let cache = self.cache.read().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        let content = cache.as_ref().ok_or_else(|| {
            ProviderError::Other("Configuration not loaded".to_string())
        })?;

        let mut result = HashMap::new();

        if let Some(ns_content) = content.list_namespace(namespace) {
            for (key, value) in ns_content {
                if let Some(p) = prefix {
                    if !key.starts_with(p) {
                        continue;
                    }
                }
                result.insert(
                    key.clone(),
                    ProviderValue::secret(Self::value_to_string(value), "encrypted_file"),
                );
            }
        }

        Ok(result)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        let content = self.load()?;
        let mut cache = self.cache.write().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;
        *cache = Some(content);
        Ok(())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.path.exists() {
            // Try to load to verify key is correct
            match self.load() {
                Ok(_) => Ok(ProviderHealth::healthy("encrypted_file")),
                Err(e) => Ok(ProviderHealth::unhealthy("encrypted_file", e.to_string())),
            }
        } else {
            Ok(ProviderHealth::unhealthy(
                "encrypted_file",
                format!("File not found: {}", self.path.display()),
            ))
        }
    }
}

#[async_trait::async_trait]
impl SecretProvider for EncryptedFileProvider {
    async fn set_secret(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> ProviderResult<ValueMetadata> {
        self.ensure_loaded()?;

        {
            let mut cache = self.cache.write().map_err(|e| {
                ProviderError::Other(format!("Failed to acquire lock: {}", e))
            })?;

            let content = cache.as_mut().ok_or_else(|| {
                ProviderError::Other("Configuration not loaded".to_string())
            })?;

            content.set(namespace, key, serde_json::Value::String(value.to_string()));
        }

        if self.auto_save {
            self.save()?;
        }

        Ok(ValueMetadata {
            source: "encrypted_file".to_string(),
            is_secret: true,
            ..Default::default()
        })
    }

    async fn delete_secret(&self, namespace: &str, key: &str) -> ProviderResult<()> {
        self.ensure_loaded()?;

        {
            let mut cache = self.cache.write().map_err(|e| {
                ProviderError::Other(format!("Failed to acquire lock: {}", e))
            })?;

            let content = cache.as_mut().ok_or_else(|| {
                ProviderError::Other("Configuration not loaded".to_string())
            })?;

            content.remove(namespace, key);
        }

        if self.auto_save {
            self.save()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_config_crypto::Algorithm;
    use tempfile::TempDir;

    fn create_test_key() -> SecretKey {
        SecretKey::generate(Algorithm::Aes256Gcm).unwrap()
    }

    #[tokio::test]
    async fn test_create_encrypted_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key = create_test_key();

        let provider = EncryptedFileProvider::create(&path, key).unwrap();

        assert!(path.exists());
        assert!(provider.is_available().await);
    }

    #[tokio::test]
    async fn test_set_and_get_secret() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key = create_test_key();

        let provider = EncryptedFileProvider::create(&path, key.clone()).unwrap();

        // Set a secret
        provider
            .set_secret("database", "password", "super_secret")
            .await
            .unwrap();

        // Get it back
        let value = provider.get("database", "password").await.unwrap();
        assert_eq!(value.value, "super_secret");
        assert!(value.metadata.is_secret);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key = create_test_key();

        // Create and set value
        {
            let provider = EncryptedFileProvider::create(&path, key.clone()).unwrap();
            provider
                .set_secret("app", "api_key", "test_key_123")
                .await
                .unwrap();
        }

        // Load in new provider instance
        {
            let provider = EncryptedFileProvider::new(&path, key).unwrap();
            let value = provider.get("app", "api_key").await.unwrap();
            assert_eq!(value.value, "test_key_123");
        }
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key = create_test_key();

        let provider = EncryptedFileProvider::create(&path, key).unwrap();

        provider
            .set_secret("test", "key", "value")
            .await
            .unwrap();

        provider.delete_secret("test", "key").await.unwrap();

        let result = provider.get("test", "key").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_list_namespace() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key = create_test_key();

        let provider = EncryptedFileProvider::create(&path, key).unwrap();

        provider.set_secret("db", "host", "localhost").await.unwrap();
        provider.set_secret("db", "port", "5432").await.unwrap();
        provider.set_secret("db", "password", "secret").await.unwrap();
        provider.set_secret("other", "key", "value").await.unwrap();

        let values = provider.list("db", None).await.unwrap();
        assert_eq!(values.len(), 3);
        assert!(values.contains_key("host"));
        assert!(values.contains_key("port"));
        assert!(values.contains_key("password"));
    }

    #[tokio::test]
    async fn test_wrong_key_fails() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.enc");
        let key1 = create_test_key();
        let key2 = create_test_key();

        // Create with key1
        {
            let provider = EncryptedFileProvider::create(&path, key1).unwrap();
            provider.set_secret("test", "key", "value").await.unwrap();
        }

        // Try to load with key2
        let provider = EncryptedFileProvider::new(&path, key2).unwrap();
        let result = provider.get("test", "key").await;

        // Should fail due to decryption error
        assert!(result.is_err());
    }
}
