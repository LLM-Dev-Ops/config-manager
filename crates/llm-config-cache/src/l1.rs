//! L1 in-memory cache with LRU eviction

use crate::{CacheError, Result};
use chrono::{DateTime, Utc};
use llm_config_core::ConfigEntry;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cached entry with metadata
#[derive(Debug, Clone)]
struct CachedEntry {
    entry: ConfigEntry,
    accessed_at: DateTime<Utc>,
    access_count: u64,
}

/// L1 in-memory cache with LRU eviction policy
pub struct L1Cache {
    cache: Arc<RwLock<HashMap<String, CachedEntry>>>,
    max_size: usize,
    hit_count: Arc<RwLock<u64>>,
    miss_count: Arc<RwLock<u64>>,
}

impl L1Cache {
    /// Create a new L1 cache with specified max size
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            hit_count: Arc::new(RwLock::new(0)),
            miss_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Generate cache key from namespace, key, and environment
    fn cache_key(namespace: &str, key: &str, env: &str) -> String {
        format!("{}:{}:{}", namespace, key, env)
    }

    /// Get an entry from the cache
    pub fn get(&self, namespace: &str, key: &str, env: &str) -> Result<ConfigEntry> {
        let cache_key = Self::cache_key(namespace, key, env);

        let mut cache = self.cache.write().unwrap();

        if let Some(cached) = cache.get_mut(&cache_key) {
            // Update access metadata
            cached.accessed_at = Utc::now();
            cached.access_count += 1;

            // Increment hit counter
            *self.hit_count.write().unwrap() += 1;

            Ok(cached.entry.clone())
        } else {
            // Increment miss counter
            *self.miss_count.write().unwrap() += 1;

            Err(CacheError::CacheMiss(cache_key))
        }
    }

    /// Put an entry into the cache
    pub fn put(&self, entry: ConfigEntry) -> Result<()> {
        let cache_key =
            Self::cache_key(&entry.namespace, &entry.key, &entry.environment.to_string());

        let mut cache = self.cache.write().unwrap();

        // Check if we need to evict
        if cache.len() >= self.max_size && !cache.contains_key(&cache_key) {
            self.evict_lru(&mut cache)?;
        }

        // Insert or update entry
        cache.insert(
            cache_key,
            CachedEntry {
                entry,
                accessed_at: Utc::now(),
                access_count: 1,
            },
        );

        Ok(())
    }

    /// Evict the least recently used entry
    fn evict_lru(&self, cache: &mut HashMap<String, CachedEntry>) -> Result<()> {
        if cache.is_empty() {
            return Ok(());
        }

        // Find LRU entry
        let lru_key = cache
            .iter()
            .min_by_key(|(_, entry)| entry.accessed_at)
            .map(|(k, _)| k.clone())
            .ok_or_else(|| CacheError::Eviction("Failed to find LRU entry".to_string()))?;

        cache.remove(&lru_key);
        Ok(())
    }

    /// Invalidate a specific entry
    pub fn invalidate(&self, namespace: &str, key: &str, env: &str) {
        let cache_key = Self::cache_key(namespace, key, env);
        let mut cache = self.cache.write().unwrap();
        cache.remove(&cache_key);
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        *self.hit_count.write().unwrap() = 0;
        *self.miss_count.write().unwrap() = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        let hit_count = *self.hit_count.read().unwrap();
        let miss_count = *self.miss_count.read().unwrap();

        CacheStats {
            size: cache.len(),
            max_size: self.max_size,
            hit_count,
            miss_count,
            hit_rate: if hit_count + miss_count > 0 {
                hit_count as f64 / (hit_count + miss_count) as f64
            } else {
                0.0
            },
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_config_core::{ConfigMetadata, ConfigValue, Environment};
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
                created_at: Utc::now(),
                created_by: "test".to_string(),
                updated_at: Utc::now(),
                updated_by: "test".to_string(),
                tags: vec![],
                description: None,
            },
        }
    }

    #[test]
    fn test_cache_creation() {
        let cache = L1Cache::new(100);
        let stats = cache.stats();
        assert_eq!(stats.size, 0);
        assert_eq!(stats.max_size, 100);
    }

    #[test]
    fn test_put_and_get() {
        let cache = L1Cache::new(100);
        let entry = create_test_entry("ns", "key1", Environment::Development);

        cache.put(entry.clone()).unwrap();

        let retrieved = cache.get("ns", "key1", "development").unwrap();
        assert_eq!(retrieved.id, entry.id);
    }

    #[test]
    fn test_cache_miss() {
        let cache = L1Cache::new(100);
        let result = cache.get("ns", "nonexistent", "development");
        assert!(result.is_err());
    }

    #[test]
    fn test_lru_eviction() {
        let cache = L1Cache::new(3);

        // Fill cache
        for i in 0..3 {
            let entry = create_test_entry("ns", &format!("key{}", i), Environment::Development);
            cache.put(entry).unwrap();
        }

        // Access key1 to make it more recently used
        cache.get("ns", "key1", "development").unwrap();

        // Add new entry, should evict key0 (least recently used)
        let entry = create_test_entry("ns", "key3", Environment::Development);
        cache.put(entry).unwrap();

        // key0 should be evicted
        assert!(cache.get("ns", "key0", "development").is_err());

        // key1, key2, key3 should still be present
        assert!(cache.get("ns", "key1", "development").is_ok());
        assert!(cache.get("ns", "key2", "development").is_ok());
        assert!(cache.get("ns", "key3", "development").is_ok());
    }

    #[test]
    fn test_invalidate() {
        let cache = L1Cache::new(100);
        let entry = create_test_entry("ns", "key1", Environment::Development);

        cache.put(entry).unwrap();
        assert!(cache.get("ns", "key1", "development").is_ok());

        cache.invalidate("ns", "key1", "development");
        assert!(cache.get("ns", "key1", "development").is_err());
    }

    #[test]
    fn test_clear() {
        let cache = L1Cache::new(100);

        for i in 0..10 {
            let entry = create_test_entry("ns", &format!("key{}", i), Environment::Development);
            cache.put(entry).unwrap();
        }

        assert_eq!(cache.stats().size, 10);

        cache.clear();

        assert_eq!(cache.stats().size, 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = L1Cache::new(100);
        let entry = create_test_entry("ns", "key1", Environment::Development);

        cache.put(entry).unwrap();

        // Generate some hits and misses
        cache.get("ns", "key1", "development").unwrap(); // hit
        cache.get("ns", "key1", "development").unwrap(); // hit
        let _ = cache.get("ns", "nonexistent", "development"); // miss

        let stats = cache.stats();
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
        assert!((stats.hit_rate - 0.666).abs() < 0.01);
    }
}
