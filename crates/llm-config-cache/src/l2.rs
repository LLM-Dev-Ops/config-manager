//! L2 persistent cache for warm restarts

use crate::{CacheError, Result};
use llm_config_core::ConfigEntry;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// L2 persistent cache
pub struct L2Cache {
    cache_dir: PathBuf,
    index: Arc<RwLock<HashMap<String, PathBuf>>>,
}

impl L2Cache {
    /// Create a new L2 cache
    pub fn new(cache_dir: impl AsRef<Path>) -> Result<Self> {
        let cache_dir = cache_dir.as_ref();
        fs::create_dir_all(cache_dir)?;

        let cache = Self {
            cache_dir: cache_dir.to_path_buf(),
            index: Arc::new(RwLock::new(HashMap::new())),
        };

        // Build index from existing cache files
        cache.rebuild_index()?;

        Ok(cache)
    }

    /// Generate cache key
    fn cache_key(namespace: &str, key: &str, env: &str) -> String {
        format!("{}:{}:{}", namespace, key, env)
    }

    /// Get cache file path for a key
    fn cache_file_path(&self, cache_key: &str) -> PathBuf {
        // Use hex encoding for safe filesystem names
        let encoded = hex::encode(cache_key.as_bytes());
        self.cache_dir.join(format!("{}.cache", encoded))
    }

    /// Rebuild the index from disk
    fn rebuild_index(&self) -> Result<()> {
        let mut index = self.index.write().unwrap();
        index.clear();

        if !self.cache_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("cache") {
                // Read the file to get the cache key
                if let Ok(file) = File::open(&path) {
                    let reader = BufReader::new(file);
                    if let Ok(cached_entry) = serde_json::from_reader::<_, ConfigEntry>(reader) {
                        let cache_key = Self::cache_key(
                            &cached_entry.namespace,
                            &cached_entry.key,
                            &cached_entry.environment.to_string(),
                        );
                        index.insert(cache_key, path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get an entry from the cache
    pub fn get(&self, namespace: &str, key: &str, env: &str) -> Result<ConfigEntry> {
        let cache_key = Self::cache_key(namespace, key, env);

        let index = self.index.read().unwrap();

        if let Some(path) = index.get(&cache_key) {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            let entry = serde_json::from_reader(reader)
                .map_err(|e| CacheError::Serialization(e.to_string()))?;
            Ok(entry)
        } else {
            Err(CacheError::CacheMiss(cache_key))
        }
    }

    /// Put an entry into the cache
    pub fn put(&self, entry: &ConfigEntry) -> Result<()> {
        let cache_key =
            Self::cache_key(&entry.namespace, &entry.key, &entry.environment.to_string());
        let path = self.cache_file_path(&cache_key);

        // Write to temp file first for atomicity
        let temp_path = path.with_extension("tmp");
        {
            let file = File::create(&temp_path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, entry)
                .map_err(|e| CacheError::Serialization(e.to_string()))?;
            writer.flush()?;
        }

        // Atomic rename
        fs::rename(&temp_path, &path)?;

        // Update index
        let mut index = self.index.write().unwrap();
        index.insert(cache_key, path);

        Ok(())
    }

    /// Invalidate a specific entry
    pub fn invalidate(&self, namespace: &str, key: &str, env: &str) -> Result<()> {
        let cache_key = Self::cache_key(namespace, key, env);

        let mut index = self.index.write().unwrap();

        if let Some(path) = index.remove(&cache_key) {
            let _ = fs::remove_file(path); // Ignore errors if file doesn't exist
        }

        Ok(())
    }

    /// Clear the entire cache
    pub fn clear(&self) -> Result<()> {
        let mut index = self.index.write().unwrap();
        index.clear();

        // Remove all cache files
        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("cache") {
                    let _ = fs::remove_file(path);
                }
            }
        }

        Ok(())
    }

    /// Get cache size (number of entries)
    pub fn size(&self) -> usize {
        self.index.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_config_core::{ConfigMetadata, ConfigValue, Environment};
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_entry(namespace: &str, key: &str, env: Environment) -> ConfigEntry {
        ConfigEntry {
            id: Uuid::new_v4(),
            namespace: namespace.to_string(),
            key: key.to_string(),
            value: ConfigValue::String("test-value".to_string()),
            environment: env,
            version: 1,
            metadata: ConfigMetadata {
                created_at: chrono::Utc::now(),
                created_by: "test".to_string(),
                updated_at: chrono::Utc::now(),
                updated_by: "test".to_string(),
                tags: vec![],
                description: None,
            },
        }
    }

    #[test]
    fn test_l2_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = L2Cache::new(temp_dir.path()).unwrap();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let cache = L2Cache::new(temp_dir.path()).unwrap();

        let entry = create_test_entry("ns", "key1", Environment::Development);
        cache.put(&entry).unwrap();

        let retrieved = cache.get("ns", "key1", "development").unwrap();
        assert_eq!(retrieved.id, entry.id);
    }

    #[test]
    fn test_cache_miss() {
        let temp_dir = TempDir::new().unwrap();
        let cache = L2Cache::new(temp_dir.path()).unwrap();

        let result = cache.get("ns", "nonexistent", "development");
        assert!(result.is_err());
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let entry = create_test_entry("ns", "key1", Environment::Development);

        // Create cache, add entry, drop it
        {
            let cache = L2Cache::new(temp_dir.path()).unwrap();
            cache.put(&entry).unwrap();
        }

        // Create new cache instance, entry should still be there
        {
            let cache = L2Cache::new(temp_dir.path()).unwrap();
            let retrieved = cache.get("ns", "key1", "development").unwrap();
            assert_eq!(retrieved.id, entry.id);
        }
    }

    #[test]
    fn test_invalidate() {
        let temp_dir = TempDir::new().unwrap();
        let cache = L2Cache::new(temp_dir.path()).unwrap();

        let entry = create_test_entry("ns", "key1", Environment::Development);
        cache.put(&entry).unwrap();

        assert!(cache.get("ns", "key1", "development").is_ok());

        cache.invalidate("ns", "key1", "development").unwrap();

        assert!(cache.get("ns", "key1", "development").is_err());
    }

    #[test]
    fn test_clear() {
        let temp_dir = TempDir::new().unwrap();
        let cache = L2Cache::new(temp_dir.path()).unwrap();

        for i in 0..10 {
            let entry = create_test_entry("ns", &format!("key{}", i), Environment::Development);
            cache.put(&entry).unwrap();
        }

        assert_eq!(cache.size(), 10);

        cache.clear().unwrap();

        assert_eq!(cache.size(), 0);
    }
}
