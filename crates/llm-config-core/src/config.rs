//! Core configuration types

pub use llm_config_storage::{ConfigEntry, ConfigMetadata, ConfigValue, Environment};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete configuration with merged environment overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub namespace: String,
    pub environment: Environment,
    pub data: HashMap<String, ConfigValue>,
    pub version: u64,
}

impl Config {
    pub fn new(namespace: impl Into<String>, environment: Environment) -> Self {
        Self {
            namespace: namespace.into(),
            environment,
            data: HashMap::new(),
            version: 1,
        }
    }

    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.data.get(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: ConfigValue) {
        self.data.insert(key.into(), value);
    }

    pub fn merge(&mut self, other: &Config) {
        for (key, value) in &other.data {
            self.data.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = Config::new("test/namespace", Environment::Development);
        assert_eq!(config.namespace, "test/namespace");
        assert_eq!(config.environment, Environment::Development);
        assert!(config.data.is_empty());
    }

    #[test]
    fn test_config_get_set() {
        let mut config = Config::new("test", Environment::Development);
        config.set("key", ConfigValue::String("value".to_string()));

        assert_eq!(config.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn test_config_merge() {
        let mut base_config = Config::new("test", Environment::Base);
        base_config.set("key1", ConfigValue::String("base".to_string()));
        base_config.set("key2", ConfigValue::String("base".to_string()));

        let mut override_config = Config::new("test", Environment::Production);
        override_config.set("key2", ConfigValue::String("override".to_string()));
        override_config.set("key3", ConfigValue::String("new".to_string()));

        base_config.merge(&override_config);

        assert_eq!(base_config.get("key1").unwrap().as_str().unwrap(), "base");
        assert_eq!(
            base_config.get("key2").unwrap().as_str().unwrap(),
            "override"
        );
        assert_eq!(base_config.get("key3").unwrap().as_str().unwrap(), "new");
    }
}
