//! Core storage models

use chrono::{DateTime, Utc};
use llm_config_crypto::EncryptedData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Environment type for configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Base,
    Development,
    Staging,
    Production,
    Edge,
}

impl std::str::FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "base" => Ok(Environment::Base),
            "dev" | "development" => Ok(Environment::Development),
            "staging" | "stage" => Ok(Environment::Staging),
            "prod" | "production" => Ok(Environment::Production),
            "edge" => Ok(Environment::Edge),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Base => write!(f, "base"),
            Environment::Development => write!(f, "development"),
            Environment::Staging => write!(f, "staging"),
            Environment::Production => write!(f, "production"),
            Environment::Edge => write!(f, "edge"),
        }
    }
}

/// Configuration value that can be a simple type or a secret
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
    Secret(EncryptedData),
}

impl ConfigValue {
    /// Check if this value is a secret
    pub fn is_secret(&self) -> bool {
        matches!(self, ConfigValue::Secret(_))
    }

    /// Get as string if possible
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer if possible
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as float if possible
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get as boolean if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Default for ConfigMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            created_by: "system".to_string(),
            updated_at: now,
            updated_by: "system".to_string(),
            tags: Vec::new(),
            description: None,
        }
    }
}

/// A configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub id: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: ConfigValue,
    pub environment: Environment,
    pub version: u64,
    pub metadata: ConfigMetadata,
}

impl ConfigEntry {
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: ConfigValue,
        environment: Environment,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            namespace: namespace.into(),
            key: key.into(),
            value,
            environment,
            version: 1,
            metadata: ConfigMetadata::default(),
        }
    }

    /// Get the full path (namespace + key)
    pub fn full_path(&self) -> String {
        format!("{}/{}", self.namespace, self.key)
    }
}

/// Version history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub version: u64,
    pub config_id: Uuid,
    pub namespace: String,
    pub key: String,
    pub value: ConfigValue,
    pub environment: Environment,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub change_description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_parsing() {
        assert_eq!(
            "dev".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "production".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert!("invalid".parse::<Environment>().is_err());
    }

    #[test]
    fn test_config_value_types() {
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(string_val.as_str(), Some("test"));

        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.as_i64(), Some(42));

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.as_bool(), Some(true));
    }

    #[test]
    fn test_config_entry_creation() {
        let entry = ConfigEntry::new(
            "test/namespace",
            "config.key",
            ConfigValue::String("value".to_string()),
            Environment::Development,
        );

        assert_eq!(entry.namespace, "test/namespace");
        assert_eq!(entry.key, "config.key");
        assert_eq!(entry.version, 1);
        assert_eq!(entry.full_path(), "test/namespace/config.key");
    }
}
