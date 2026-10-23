//! Configuration manager - core business logic

use crate::{ConfigEntry, ConfigValue, Environment, Result, VersionControl};
use chrono::Utc;
use llm_config_crypto::{decrypt, encrypt, SecretKey};
use llm_config_storage::file::FileStorage;
use std::path::Path;

/// Main configuration manager
pub struct ConfigManager {
    storage: FileStorage,
    version_control: VersionControl,
    encryption_key: Option<SecretKey>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(storage_path: impl AsRef<Path>) -> Result<Self> {
        let storage = FileStorage::new(storage_path)?;
        let version_control = VersionControl::new(storage.clone());

        Ok(Self {
            storage,
            version_control,
            encryption_key: None,
        })
    }

    /// Set the encryption key for secrets
    pub fn with_encryption_key(mut self, key: SecretKey) -> Self {
        self.encryption_key = Some(key);
        self
    }

    /// Get a single configuration value
    pub fn get(&self, namespace: &str, key: &str, env: Environment) -> Result<Option<ConfigEntry>> {
        let mut entry = self.storage.get(namespace, key, env)?;

        // Decrypt secrets if encryption key is available
        if let Some(ref mut config) = entry {
            if let Some(ref key) = self.encryption_key {
                self.decrypt_entry(config, key)?;
            }
        }

        Ok(entry)
    }

    /// Get a configuration with environment overrides applied
    pub fn get_with_overrides(
        &self,
        namespace: &str,
        key: &str,
        env: Environment,
    ) -> Result<Option<ConfigValue>> {
        // Start with base configuration
        let mut value = self
            .storage
            .get(namespace, key, Environment::Base)?
            .map(|e| e.value);

        // Apply environment-specific overrides in order
        let envs = match env {
            Environment::Base => vec![],
            Environment::Development => vec![Environment::Development],
            Environment::Staging => vec![Environment::Development, Environment::Staging],
            Environment::Production => vec![
                Environment::Development,
                Environment::Staging,
                Environment::Production,
            ],
            Environment::Edge => vec![Environment::Edge],
        };

        for override_env in envs {
            if let Some(override_entry) = self.storage.get(namespace, key, override_env)? {
                value = Some(override_entry.value);
            }
        }

        // Decrypt if it's a secret
        if let Some(ConfigValue::Secret(ref encrypted)) = value {
            if let Some(ref key) = self.encryption_key {
                let plaintext = decrypt(key, encrypted)?;
                let plaintext_str = String::from_utf8(plaintext)
                    .map_err(|e| crate::ConfigError::ValidationError(e.to_string()))?;
                return Ok(Some(ConfigValue::String(plaintext_str)));
            }
        }

        Ok(value)
    }

    /// Set a configuration value
    pub fn set(
        &self,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: ConfigValue,
        env: Environment,
        user: impl Into<String>,
    ) -> Result<ConfigEntry> {
        let namespace = namespace.into();
        let key_str = key.into();
        let user = user.into();

        // Check if config exists to determine if this is an update
        let existing = self.storage.get(&namespace, &key_str, env)?;

        let mut entry = if let Some(mut existing_entry) = existing {
            // Update existing
            existing_entry.value = value;
            existing_entry.version += 1;
            existing_entry.metadata.updated_at = Utc::now();
            existing_entry.metadata.updated_by = user;
            existing_entry
        } else {
            // Create new
            let mut entry = ConfigEntry::new(namespace.clone(), key_str.clone(), value, env);
            entry.metadata.created_by = user.clone();
            entry.metadata.updated_by = user;
            entry
        };

        // Encrypt if the value is marked as a secret and we have an encryption key
        if let Some(ref key) = self.encryption_key {
            self.encrypt_entry(&mut entry, key)?;
        }

        // Save to storage
        self.storage.set(entry.clone())?;

        // Create version snapshot
        self.version_control
            .create_snapshot(&entry, Some("Configuration updated".to_string()))?;

        Ok(entry)
    }

    /// Set a secret value (automatically encrypted)
    pub fn set_secret(
        &self,
        namespace: impl Into<String>,
        key: impl Into<String>,
        plaintext: impl AsRef<[u8]>,
        env: Environment,
        user: impl Into<String>,
    ) -> Result<ConfigEntry> {
        let encryption_key = self.encryption_key.as_ref().ok_or_else(|| {
            crate::ConfigError::ValidationError("Encryption key not configured".to_string())
        })?;

        let encrypted = encrypt(encryption_key, plaintext.as_ref(), None)?;
        let value = ConfigValue::Secret(encrypted);

        self.set(namespace, key, value, env, user)
    }

    /// Get and decrypt a secret value
    pub fn get_secret(
        &self,
        namespace: &str,
        key: &str,
        env: Environment,
    ) -> Result<Option<Vec<u8>>> {
        let encryption_key = self.encryption_key.as_ref().ok_or_else(|| {
            crate::ConfigError::ValidationError("Encryption key not configured".to_string())
        })?;

        let entry = match self.storage.get(namespace, key, env)? {
            Some(e) => e,
            None => return Ok(None),
        };

        match entry.value {
            ConfigValue::Secret(ref encrypted_data) => {
                let plaintext = decrypt(encryption_key, encrypted_data)?;
                Ok(Some(plaintext))
            }
            _ => Err(crate::ConfigError::ValidationError(
                "Not a secret value".to_string(),
            )),
        }
    }

    /// List all configurations in a namespace
    pub fn list(&self, namespace: &str, env: Environment) -> Result<Vec<ConfigEntry>> {
        let mut entries = self.storage.list(namespace, env)?;

        // Decrypt secrets if encryption key is available
        if let Some(ref key) = self.encryption_key {
            for entry in &mut entries {
                self.decrypt_entry(entry, key)?;
            }
        }

        Ok(entries)
    }

    /// Delete a configuration
    pub fn delete(&self, namespace: &str, key: &str, env: Environment) -> Result<bool> {
        Ok(self.storage.delete(namespace, key, env)?)
    }

    /// Get version history
    pub fn get_history(
        &self,
        namespace: &str,
        key: &str,
        env: Environment,
    ) -> Result<Vec<llm_config_storage::VersionEntry>> {
        self.version_control.get_history(namespace, key, env)
    }

    /// Rollback to a specific version
    pub fn rollback(
        &self,
        namespace: &str,
        key: &str,
        env: Environment,
        version: u64,
    ) -> Result<Option<ConfigEntry>> {
        self.version_control.rollback(namespace, key, env, version)
    }

    /// Decrypt secrets in a config entry
    fn decrypt_entry(&self, entry: &mut ConfigEntry, key: &SecretKey) -> Result<()> {
        if let ConfigValue::Secret(ref encrypted) = entry.value {
            let plaintext = decrypt(key, encrypted)?;
            // For now, assume secrets are UTF-8 strings
            let plaintext_str = String::from_utf8(plaintext)
                .map_err(|e| crate::ConfigError::ValidationError(e.to_string()))?;
            entry.value = ConfigValue::String(plaintext_str);
        }
        Ok(())
    }

    /// Encrypt secrets in a config entry
    fn encrypt_entry(&self, _entry: &mut ConfigEntry, _key: &SecretKey) -> Result<()> {
        // Check if the value should be encrypted (e.g., if it's already a Secret or if it matches patterns)
        // For now, we don't auto-encrypt strings, only when explicitly set via set_secret
        Ok(())
    }

    /// Export all configurations
    pub fn export_all(&self, export_path: impl AsRef<Path>) -> Result<usize> {
        Ok(self.storage.export_all(export_path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_config_crypto::Algorithm;
    use tempfile::TempDir;

    #[test]
    fn test_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();
        assert!(manager.encryption_key.is_none());
    }

    #[test]
    fn test_set_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        let entry = manager
            .set(
                "test/ns",
                "config.key",
                ConfigValue::String("value".to_string()),
                Environment::Development,
                "test-user",
            )
            .unwrap();

        assert_eq!(entry.version, 1);

        let retrieved = manager
            .get("test/ns", "config.key", Environment::Development)
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.key, "config.key");
        assert_eq!(retrieved.value.as_str().unwrap(), "value");
    }

    #[test]
    fn test_environment_overrides() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        // Set base value
        manager
            .set(
                "test/ns",
                "key",
                ConfigValue::String("base".to_string()),
                Environment::Base,
                "user",
            )
            .unwrap();

        // Set production override
        manager
            .set(
                "test/ns",
                "key",
                ConfigValue::String("production".to_string()),
                Environment::Production,
                "user",
            )
            .unwrap();

        // Get with overrides
        let value = manager
            .get_with_overrides("test/ns", "key", Environment::Production)
            .unwrap()
            .unwrap();

        assert_eq!(value.as_str().unwrap(), "production");

        // Development should still get base
        let dev_value = manager
            .get_with_overrides("test/ns", "key", Environment::Development)
            .unwrap()
            .unwrap();

        assert_eq!(dev_value.as_str().unwrap(), "base");
    }

    #[test]
    fn test_secret_encryption() {
        let temp_dir = TempDir::new().unwrap();
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let manager = ConfigManager::new(temp_dir.path())
            .unwrap()
            .with_encryption_key(key);

        let secret_value = b"my-secret-password";

        manager
            .set_secret(
                "test/ns",
                "db.password",
                secret_value,
                Environment::Production,
                "admin",
            )
            .unwrap();

        // Retrieve should decrypt automatically
        let retrieved = manager
            .get("test/ns", "db.password", Environment::Production)
            .unwrap()
            .unwrap();

        assert_eq!(
            retrieved.value.as_str().unwrap(),
            std::str::from_utf8(secret_value).unwrap()
        );
    }

    #[test]
    fn test_list_configs() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        manager
            .set(
                "test/ns",
                "key1",
                ConfigValue::String("val1".to_string()),
                Environment::Development,
                "user",
            )
            .unwrap();
        manager
            .set(
                "test/ns",
                "key2",
                ConfigValue::String("val2".to_string()),
                Environment::Development,
                "user",
            )
            .unwrap();

        let list = manager.list("test/ns", Environment::Development).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        manager
            .set(
                "test/ns",
                "key",
                ConfigValue::String("val".to_string()),
                Environment::Development,
                "user",
            )
            .unwrap();

        let deleted = manager
            .delete("test/ns", "key", Environment::Development)
            .unwrap();
        assert!(deleted);

        let retrieved = manager
            .get("test/ns", "key", Environment::Development)
            .unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_versioning() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        // Set initial value
        manager
            .set(
                "test/ns",
                "key",
                ConfigValue::String("v1".to_string()),
                Environment::Development,
                "user",
            )
            .unwrap();

        // Update value
        manager
            .set(
                "test/ns",
                "key",
                ConfigValue::String("v2".to_string()),
                Environment::Development,
                "user",
            )
            .unwrap();

        // Get history
        let history = manager
            .get_history("test/ns", "key", Environment::Development)
            .unwrap();
        assert!(history.len() >= 2);

        // Rollback to version 1
        let rolled_back = manager
            .rollback("test/ns", "key", Environment::Development, 1)
            .unwrap()
            .unwrap();

        assert_eq!(rolled_back.value.as_str().unwrap(), "v1");
    }
}
