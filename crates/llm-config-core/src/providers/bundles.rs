//! Configuration Bundle Providers
//!
//! This module provides adapters for loading configuration from standard
//! file formats: JSON, TOML, and YAML. These are read-only providers that
//! parse static configuration files.
//!
//! # Structure Convention
//!
//! All bundle formats expect a similar structure:
//!
//! ```yaml
//! namespace1:
//!   key1: value1
//!   key2: value2
//! namespace2:
//!   nested:
//!     key: value
//! ```
//!
//! Nested keys are flattened using dots: `nested.key`
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::{JsonProvider, TomlProvider, YamlProvider};
//!
//! let json = JsonProvider::from_file("config.json")?;
//! let toml = TomlProvider::from_file("config.toml")?;
//! let yaml = YamlProvider::from_file("config.yaml")?;
//!
//! // Or auto-detect format
//! let bundle = BundleProvider::from_file("config.yaml")?;
//! ```

use super::traits::{
    ConfigProvider, ProviderError, ProviderResult, ProviderValue, ProviderHealth,
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Parsed configuration structure
#[derive(Debug, Default, Clone)]
struct ParsedConfig {
    /// Namespace -> Key -> Value mapping
    namespaces: HashMap<String, HashMap<String, String>>,
}

impl ParsedConfig {
    /// Parse from JSON value
    fn from_json(value: JsonValue) -> ProviderResult<Self> {
        let mut config = Self::default();

        if let JsonValue::Object(root) = value {
            for (namespace, ns_value) in root {
                if let JsonValue::Object(ns_obj) = ns_value {
                    let mut ns_map = HashMap::new();
                    Self::flatten_object(&ns_obj, "", &mut ns_map);
                    config.namespaces.insert(namespace, ns_map);
                }
            }
        }

        Ok(config)
    }

    /// Flatten nested objects into dot-separated keys
    fn flatten_object(
        obj: &serde_json::Map<String, JsonValue>,
        prefix: &str,
        result: &mut HashMap<String, String>,
    ) {
        for (key, value) in obj {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            match value {
                JsonValue::Object(nested) => {
                    Self::flatten_object(nested, &full_key, result);
                }
                JsonValue::Array(arr) => {
                    // Store array as JSON string
                    result.insert(full_key, serde_json::to_string(arr).unwrap_or_default());
                }
                JsonValue::String(s) => {
                    result.insert(full_key, s.clone());
                }
                JsonValue::Number(n) => {
                    result.insert(full_key, n.to_string());
                }
                JsonValue::Bool(b) => {
                    result.insert(full_key, b.to_string());
                }
                JsonValue::Null => {
                    result.insert(full_key, "null".to_string());
                }
            }
        }
    }

    fn get(&self, namespace: &str, key: &str) -> Option<&String> {
        self.namespaces.get(namespace)?.get(key)
    }

    fn list_namespace(&self, namespace: &str) -> Option<&HashMap<String, String>> {
        self.namespaces.get(namespace)
    }
}

/// JSON configuration file provider
#[derive(Debug)]
pub struct JsonProvider {
    path: PathBuf,
    cache: RwLock<Option<ParsedConfig>>,
}

impl JsonProvider {
    /// Create a provider from a JSON file
    pub fn from_file(path: impl AsRef<Path>) -> ProviderResult<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(ProviderError::ConfigurationError(
                format!("JSON file not found: {}", path.display())
            ));
        }

        Ok(Self {
            path,
            cache: RwLock::new(None),
        })
    }

    /// Parse JSON from a string
    pub fn from_string(content: &str) -> ProviderResult<Self> {
        let value: JsonValue = serde_json::from_str(content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        let config = ParsedConfig::from_json(value)?;

        Ok(Self {
            path: PathBuf::new(),
            cache: RwLock::new(Some(config)),
        })
    }

    fn load(&self) -> ProviderResult<ParsedConfig> {
        let content = std::fs::read_to_string(&self.path)?;
        let value: JsonValue = serde_json::from_str(&content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;
        ParsedConfig::from_json(value)
    }

    fn ensure_loaded(&self) -> ProviderResult<()> {
        let loaded = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?
            .is_some();

        if !loaded && !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConfigProvider for JsonProvider {
    fn name(&self) -> &str {
        "json"
    }

    async fn is_available(&self) -> bool {
        self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false)
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        match config.get(namespace, key) {
            Some(value) => Ok(ProviderValue::new(value.clone(), "json")),
            None => Err(ProviderError::NotFound {
                namespace: namespace.to_string(),
                key: key.to_string(),
            }),
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        let mut result = HashMap::new();

        if let Some(ns_content) = config.list_namespace(namespace) {
            for (key, value) in ns_content {
                if let Some(p) = prefix {
                    if !key.starts_with(p) {
                        continue;
                    }
                }
                result.insert(key.clone(), ProviderValue::new(value.clone(), "json"));
            }
        }

        Ok(result)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        if !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false) {
            Ok(ProviderHealth::healthy("json"))
        } else {
            Ok(ProviderHealth::unhealthy("json", "File not found"))
        }
    }
}

/// TOML configuration file provider
#[derive(Debug)]
pub struct TomlProvider {
    path: PathBuf,
    cache: RwLock<Option<ParsedConfig>>,
}

impl TomlProvider {
    /// Create a provider from a TOML file
    pub fn from_file(path: impl AsRef<Path>) -> ProviderResult<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(ProviderError::ConfigurationError(
                format!("TOML file not found: {}", path.display())
            ));
        }

        Ok(Self {
            path,
            cache: RwLock::new(None),
        })
    }

    /// Parse TOML from a string
    pub fn from_string(content: &str) -> ProviderResult<Self> {
        let value: toml::Value = toml::from_str(content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        // Convert TOML to JSON for uniform handling
        let json_value = toml_to_json(value);
        let config = ParsedConfig::from_json(json_value)?;

        Ok(Self {
            path: PathBuf::new(),
            cache: RwLock::new(Some(config)),
        })
    }

    fn load(&self) -> ProviderResult<ParsedConfig> {
        let content = std::fs::read_to_string(&self.path)?;
        let value: toml::Value = toml::from_str(&content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;
        let json_value = toml_to_json(value);
        ParsedConfig::from_json(json_value)
    }

    fn ensure_loaded(&self) -> ProviderResult<()> {
        let loaded = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?
            .is_some();

        if !loaded && !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }
}

/// Convert TOML value to JSON value
fn toml_to_json(toml: toml::Value) -> JsonValue {
    match toml {
        toml::Value::String(s) => JsonValue::String(s),
        toml::Value::Integer(i) => JsonValue::Number(serde_json::Number::from(i)),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null)
        }
        toml::Value::Boolean(b) => JsonValue::Bool(b),
        toml::Value::Array(arr) => {
            JsonValue::Array(arr.into_iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, JsonValue> = table
                .into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect();
            JsonValue::Object(map)
        }
        toml::Value::Datetime(dt) => JsonValue::String(dt.to_string()),
    }
}

#[async_trait::async_trait]
impl ConfigProvider for TomlProvider {
    fn name(&self) -> &str {
        "toml"
    }

    async fn is_available(&self) -> bool {
        self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false)
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        match config.get(namespace, key) {
            Some(value) => Ok(ProviderValue::new(value.clone(), "toml")),
            None => Err(ProviderError::NotFound {
                namespace: namespace.to_string(),
                key: key.to_string(),
            }),
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        let mut result = HashMap::new();

        if let Some(ns_content) = config.list_namespace(namespace) {
            for (key, value) in ns_content {
                if let Some(p) = prefix {
                    if !key.starts_with(p) {
                        continue;
                    }
                }
                result.insert(key.clone(), ProviderValue::new(value.clone(), "toml"));
            }
        }

        Ok(result)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        if !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false) {
            Ok(ProviderHealth::healthy("toml"))
        } else {
            Ok(ProviderHealth::unhealthy("toml", "File not found"))
        }
    }
}

/// YAML configuration file provider
#[derive(Debug)]
pub struct YamlProvider {
    path: PathBuf,
    cache: RwLock<Option<ParsedConfig>>,
}

impl YamlProvider {
    /// Create a provider from a YAML file
    pub fn from_file(path: impl AsRef<Path>) -> ProviderResult<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(ProviderError::ConfigurationError(
                format!("YAML file not found: {}", path.display())
            ));
        }

        Ok(Self {
            path,
            cache: RwLock::new(None),
        })
    }

    /// Parse YAML from a string
    pub fn from_string(content: &str) -> ProviderResult<Self> {
        let value: serde_yaml::Value = serde_yaml::from_str(content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;

        // Convert YAML to JSON for uniform handling
        let json_value = yaml_to_json(value);
        let config = ParsedConfig::from_json(json_value)?;

        Ok(Self {
            path: PathBuf::new(),
            cache: RwLock::new(Some(config)),
        })
    }

    fn load(&self) -> ProviderResult<ParsedConfig> {
        let content = std::fs::read_to_string(&self.path)?;
        let value: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| ProviderError::SerializationError(e.to_string()))?;
        let json_value = yaml_to_json(value);
        ParsedConfig::from_json(json_value)
    }

    fn ensure_loaded(&self) -> ProviderResult<()> {
        let loaded = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?
            .is_some();

        if !loaded && !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }
}

/// Convert YAML value to JSON value
fn yaml_to_json(yaml: serde_yaml::Value) -> JsonValue {
    match yaml {
        serde_yaml::Value::Null => JsonValue::Null,
        serde_yaml::Value::Bool(b) => JsonValue::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                JsonValue::Number(serde_json::Number::from(i))
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            } else {
                JsonValue::Null
            }
        }
        serde_yaml::Value::String(s) => JsonValue::String(s),
        serde_yaml::Value::Sequence(seq) => {
            JsonValue::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, JsonValue> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => other.as_str().map(|s| s.to_string())?,
                    };
                    Some((key, yaml_to_json(v)))
                })
                .collect();
            JsonValue::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

#[async_trait::async_trait]
impl ConfigProvider for YamlProvider {
    fn name(&self) -> &str {
        "yaml"
    }

    async fn is_available(&self) -> bool {
        self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false)
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        match config.get(namespace, key) {
            Some(value) => Ok(ProviderValue::new(value.clone(), "yaml")),
            None => Err(ProviderError::NotFound {
                namespace: namespace.to_string(),
                key: key.to_string(),
            }),
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        self.ensure_loaded()?;

        let cache = self.cache.read()
            .map_err(|e| ProviderError::Other(e.to_string()))?;

        let config = cache.as_ref()
            .ok_or_else(|| ProviderError::Other("Config not loaded".to_string()))?;

        let mut result = HashMap::new();

        if let Some(ns_content) = config.list_namespace(namespace) {
            for (key, value) in ns_content {
                if let Some(p) = prefix {
                    if !key.starts_with(p) {
                        continue;
                    }
                }
                result.insert(key.clone(), ProviderValue::new(value.clone(), "yaml"));
            }
        }

        Ok(result)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        if !self.path.as_os_str().is_empty() {
            let config = self.load()?;
            *self.cache.write()
                .map_err(|e| ProviderError::Other(e.to_string()))? = Some(config);
        }
        Ok(())
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.path.exists() || self.cache.read().map(|c| c.is_some()).unwrap_or(false) {
            Ok(ProviderHealth::healthy("yaml"))
        } else {
            Ok(ProviderHealth::unhealthy("yaml", "File not found"))
        }
    }
}

/// Auto-detecting bundle provider
///
/// This provider automatically detects the file format based on extension
/// and delegates to the appropriate provider.
#[derive(Debug)]
pub enum BundleProvider {
    Json(JsonProvider),
    Toml(TomlProvider),
    Yaml(YamlProvider),
}

impl BundleProvider {
    /// Create a bundle provider, auto-detecting format from file extension
    pub fn from_file(path: impl AsRef<Path>) -> ProviderResult<Self> {
        let path = path.as_ref();
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "json" => Ok(BundleProvider::Json(JsonProvider::from_file(path)?)),
            "toml" => Ok(BundleProvider::Toml(TomlProvider::from_file(path)?)),
            "yaml" | "yml" => Ok(BundleProvider::Yaml(YamlProvider::from_file(path)?)),
            _ => Err(ProviderError::ConfigurationError(
                format!("Unknown file format: {}", extension)
            )),
        }
    }
}

#[async_trait::async_trait]
impl ConfigProvider for BundleProvider {
    fn name(&self) -> &str {
        match self {
            BundleProvider::Json(p) => p.name(),
            BundleProvider::Toml(p) => p.name(),
            BundleProvider::Yaml(p) => p.name(),
        }
    }

    async fn is_available(&self) -> bool {
        match self {
            BundleProvider::Json(p) => p.is_available().await,
            BundleProvider::Toml(p) => p.is_available().await,
            BundleProvider::Yaml(p) => p.is_available().await,
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        match self {
            BundleProvider::Json(p) => p.get(namespace, key).await,
            BundleProvider::Toml(p) => p.get(namespace, key).await,
            BundleProvider::Yaml(p) => p.get(namespace, key).await,
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        match self {
            BundleProvider::Json(p) => p.list(namespace, prefix).await,
            BundleProvider::Toml(p) => p.list(namespace, prefix).await,
            BundleProvider::Yaml(p) => p.list(namespace, prefix).await,
        }
    }

    async fn refresh(&self) -> ProviderResult<()> {
        match self {
            BundleProvider::Json(p) => p.refresh().await,
            BundleProvider::Toml(p) => p.refresh().await,
            BundleProvider::Yaml(p) => p.refresh().await,
        }
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        match self {
            BundleProvider::Json(p) => p.health_check(),
            BundleProvider::Toml(p) => p.health_check(),
            BundleProvider::Yaml(p) => p.health_check(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_json_provider_from_string() {
        let json = r#"{
            "database": {
                "host": "localhost",
                "port": "5432"
            },
            "app": {
                "name": "test-app"
            }
        }"#;

        let provider = JsonProvider::from_string(json).unwrap();

        let host = provider.get("database", "host").await.unwrap();
        assert_eq!(host.value, "localhost");

        let port = provider.get("database", "port").await.unwrap();
        assert_eq!(port.value, "5432");

        let name = provider.get("app", "name").await.unwrap();
        assert_eq!(name.value, "test-app");
    }

    #[tokio::test]
    async fn test_json_nested_keys() {
        let json = r#"{
            "database": {
                "primary": {
                    "host": "primary-host",
                    "port": "5432"
                },
                "replica": {
                    "host": "replica-host"
                }
            }
        }"#;

        let provider = JsonProvider::from_string(json).unwrap();

        let host = provider.get("database", "primary.host").await.unwrap();
        assert_eq!(host.value, "primary-host");

        let replica = provider.get("database", "replica.host").await.unwrap();
        assert_eq!(replica.value, "replica-host");
    }

    #[tokio::test]
    async fn test_toml_provider_from_string() {
        let toml = r#"
            [database]
            host = "localhost"
            port = 5432

            [app]
            name = "test-app"
        "#;

        let provider = TomlProvider::from_string(toml).unwrap();

        let host = provider.get("database", "host").await.unwrap();
        assert_eq!(host.value, "localhost");

        let port = provider.get("database", "port").await.unwrap();
        assert_eq!(port.value, "5432");
    }

    #[tokio::test]
    async fn test_yaml_provider_from_string() {
        let yaml = r#"
database:
  host: localhost
  port: 5432
app:
  name: test-app
        "#;

        let provider = YamlProvider::from_string(yaml).unwrap();

        let host = provider.get("database", "host").await.unwrap();
        assert_eq!(host.value, "localhost");

        let name = provider.get("app", "name").await.unwrap();
        assert_eq!(name.value, "test-app");
    }

    #[tokio::test]
    async fn test_list_namespace() {
        let json = r#"{
            "database": {
                "host": "localhost",
                "port": "5432",
                "user": "admin"
            }
        }"#;

        let provider = JsonProvider::from_string(json).unwrap();
        let values = provider.list("database", None).await.unwrap();

        assert_eq!(values.len(), 3);
        assert!(values.contains_key("host"));
        assert!(values.contains_key("port"));
        assert!(values.contains_key("user"));
    }

    #[tokio::test]
    async fn test_list_with_prefix() {
        let json = r#"{
            "database": {
                "primary.host": "primary",
                "primary.port": "5432",
                "replica.host": "replica"
            }
        }"#;

        let provider = JsonProvider::from_string(json).unwrap();
        let values = provider.list("database", Some("primary")).await.unwrap();

        assert_eq!(values.len(), 2);
        assert!(values.contains_key("primary.host"));
        assert!(values.contains_key("primary.port"));
    }

    #[tokio::test]
    async fn test_not_found() {
        let json = r#"{"database": {"host": "localhost"}}"#;
        let provider = JsonProvider::from_string(json).unwrap();

        let result = provider.get("database", "nonexistent").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));

        let result = provider.get("nonexistent", "key").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }
}
