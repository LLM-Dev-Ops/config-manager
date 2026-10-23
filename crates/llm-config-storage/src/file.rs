//! File-based storage backend with atomic operations

use crate::{ConfigEntry, Environment, Result, StorageError, VersionEntry};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// File-based storage backend
#[derive(Clone)]
pub struct FileStorage {
    base_path: PathBuf,
    /// In-memory index for fast lookups
    index: Arc<RwLock<HashMap<String, ConfigEntry>>>,
}

impl FileStorage {
    /// Create a new file storage at the given path
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();

        // Create directories
        fs::create_dir_all(&base_path)?;
        fs::create_dir_all(base_path.join("configs"))?;
        fs::create_dir_all(base_path.join("versions"))?;

        let storage = Self {
            base_path,
            index: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load existing configs into index
        storage.rebuild_index()?;

        Ok(storage)
    }

    /// Rebuild the index from disk
    fn rebuild_index(&self) -> Result<()> {
        let configs_dir = self.base_path.join("configs");
        if !configs_dir.exists() {
            return Ok(());
        }

        let mut index = self.index.write().unwrap();
        index.clear();

        for entry in fs::read_dir(&configs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(config) = self.load_config_from_file(&path) {
                    let key = self.make_key(&config.namespace, &config.key, config.environment);
                    index.insert(key, config);
                }
            }
        }

        Ok(())
    }

    /// Make a storage key from namespace, key, and environment
    fn make_key(&self, namespace: &str, key: &str, env: Environment) -> String {
        format!("{}::{}::{}", namespace, key, env)
    }

    /// Get the file path for a config entry
    fn config_file_path(&self, namespace: &str, key: &str, env: Environment) -> PathBuf {
        let safe_namespace = namespace.replace('/', "_");
        let safe_key = key.replace('/', "_");
        let filename = format!("{}_{}_{}.json", safe_namespace, safe_key, env);
        self.base_path.join("configs").join(filename)
    }

    /// Get the file path for a version entry
    fn version_file_path(&self, version_id: Uuid) -> PathBuf {
        self.base_path
            .join("versions")
            .join(format!("{}.json", version_id))
    }

    /// Load a config from a file
    fn load_config_from_file(&self, path: &Path) -> Result<ConfigEntry> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        serde_json::from_str(&contents).map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    /// Atomically write a config to a file
    fn write_config_atomically(&self, config: &ConfigEntry) -> Result<()> {
        let path = self.config_file_path(&config.namespace, &config.key, config.environment);

        // Serialize to JSON
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        // Write to temporary file first
        let temp_path = path.with_extension("tmp");
        {
            let mut temp_file = File::create(&temp_path)?;
            temp_file.write_all(json.as_bytes())?;
            temp_file.sync_all()?; // Ensure data is written to disk
        }

        // Atomic rename
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Store a configuration
    pub fn set(&self, config: ConfigEntry) -> Result<()> {
        // Write to disk atomically
        self.write_config_atomically(&config)?;

        // Update index
        let key = self.make_key(&config.namespace, &config.key, config.environment);
        let mut index = self.index.write().unwrap();
        index.insert(key, config);

        Ok(())
    }

    /// Get a configuration
    pub fn get(&self, namespace: &str, key: &str, env: Environment) -> Result<Option<ConfigEntry>> {
        let storage_key = self.make_key(namespace, key, env);
        let index = self.index.read().unwrap();

        Ok(index.get(&storage_key).cloned())
    }

    /// List all configurations in a namespace
    pub fn list(&self, namespace: &str, env: Environment) -> Result<Vec<ConfigEntry>> {
        let index = self.index.read().unwrap();
        let prefix = format!("{}::", namespace);
        let suffix = format!("::{}", env);

        let configs: Vec<ConfigEntry> = index
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix) && k.ends_with(&suffix))
            .map(|(_, v)| v.clone())
            .collect();

        Ok(configs)
    }

    /// Delete a configuration
    pub fn delete(&self, namespace: &str, key: &str, env: Environment) -> Result<bool> {
        let storage_key = self.make_key(namespace, key, env);

        // Remove from index
        let mut index = self.index.write().unwrap();
        let removed = index.remove(&storage_key).is_some();

        if removed {
            // Delete file
            let path = self.config_file_path(namespace, key, env);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        Ok(removed)
    }

    /// Store a version entry
    pub fn store_version(&self, version: VersionEntry) -> Result<()> {
        let version_id = Uuid::new_v4();
        let path = self.version_file_path(version_id);

        let json = serde_json::to_string_pretty(&version)
            .map_err(|e| StorageError::SerializationError(e.to_string()))?;

        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }

    /// Get version history for a config
    pub fn get_versions(
        &self,
        namespace: &str,
        key: &str,
        env: Environment,
    ) -> Result<Vec<VersionEntry>> {
        let versions_dir = self.base_path.join("versions");
        if !versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();

        for entry in fs::read_dir(&versions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(mut file) = File::open(&path) {
                    let mut contents = String::new();
                    if file.read_to_string(&mut contents).is_ok() {
                        if let Ok(version) = serde_json::from_str::<VersionEntry>(&contents) {
                            if version.namespace == namespace
                                && version.key == key
                                && version.environment == env
                            {
                                versions.push(version);
                            }
                        }
                    }
                }
            }
        }

        // Sort by version number descending.
        versions.sort_by_key(|entry| std::cmp::Reverse(entry.version));

        Ok(versions)
    }

    /// Export all configurations to a directory
    pub fn export_all(&self, export_path: impl AsRef<Path>) -> Result<usize> {
        let export_path = export_path.as_ref();
        fs::create_dir_all(export_path)?;

        let index = self.index.read().unwrap();
        let count = index.len();

        for config in index.values() {
            let filename = format!(
                "{}_{}_{}_{}.json",
                config.namespace.replace('/', "_"),
                config.key.replace('/', "_"),
                config.environment,
                config.id
            );
            let path = export_path.join(filename);

            let json = serde_json::to_string_pretty(config)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?;

            let mut file = File::create(path)?;
            file.write_all(json.as_bytes())?;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigValue;
    use tempfile::TempDir;

    #[test]
    fn test_file_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let _storage = FileStorage::new(temp_dir.path()).unwrap();

        assert!(temp_dir.path().join("configs").exists());
        assert!(temp_dir.path().join("versions").exists());
    }

    #[test]
    fn test_set_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();

        let entry = ConfigEntry::new(
            "test/namespace",
            "config.key",
            ConfigValue::String("test value".to_string()),
            Environment::Development,
        );

        storage.set(entry.clone()).unwrap();

        let retrieved = storage
            .get("test/namespace", "config.key", Environment::Development)
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.namespace, entry.namespace);
        assert_eq!(retrieved.key, entry.key);
    }

    #[test]
    fn test_list_configs() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();

        let entry1 = ConfigEntry::new(
            "test/ns",
            "key1",
            ConfigValue::String("val1".to_string()),
            Environment::Development,
        );
        let entry2 = ConfigEntry::new(
            "test/ns",
            "key2",
            ConfigValue::String("val2".to_string()),
            Environment::Development,
        );

        storage.set(entry1).unwrap();
        storage.set(entry2).unwrap();

        let configs = storage.list("test/ns", Environment::Development).unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();

        let entry = ConfigEntry::new(
            "test/ns",
            "key",
            ConfigValue::String("val".to_string()),
            Environment::Development,
        );

        storage.set(entry).unwrap();
        assert!(storage
            .get("test/ns", "key", Environment::Development)
            .unwrap()
            .is_some());

        let deleted = storage
            .delete("test/ns", "key", Environment::Development)
            .unwrap();
        assert!(deleted);

        assert!(storage
            .get("test/ns", "key", Environment::Development)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        let entry = ConfigEntry::new(
            "test/ns",
            "key",
            ConfigValue::String("val".to_string()),
            Environment::Production,
        );

        // Create storage, add entry, drop it
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            storage.set(entry.clone()).unwrap();
        }

        // Create new storage instance and verify entry exists
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            let retrieved = storage
                .get("test/ns", "key", Environment::Production)
                .unwrap()
                .unwrap();
            assert_eq!(retrieved.key, entry.key);
        }
    }
}
