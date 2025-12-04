//! Environment Variable Configuration Providers
//!
//! This module provides adapters for loading configuration from:
//! - Raw system environment variables
//! - `.env` files (with optional parsing)
//!
//! # Key Naming Convention
//!
//! Environment variables are mapped to namespace/key pairs using configurable
//! conventions. By default:
//! - Namespace and key are joined with `__` (double underscore)
//! - All names are uppercased
//! - Example: namespace="database", key="host" → `DATABASE__HOST`
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::{EnvProvider, DotEnvProvider};
//!
//! // Load from system environment
//! let env = EnvProvider::new();
//! let value = env.get("database", "host").await?;
//!
//! // Load from .env file
//! let dotenv = DotEnvProvider::from_file(".env")?;
//! let secret = dotenv.get("app", "secret_key").await?;
//! ```

use super::traits::{ConfigProvider, ProviderError, ProviderResult, ProviderValue, ProviderHealth};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Configuration for environment variable naming
#[derive(Debug, Clone)]
pub struct EnvNamingConfig {
    /// Separator between namespace and key (default: "__")
    pub separator: String,
    /// Prefix for all environment variables (optional)
    pub prefix: Option<String>,
    /// Whether to uppercase variable names (default: true)
    pub uppercase: bool,
}

impl Default for EnvNamingConfig {
    fn default() -> Self {
        Self {
            separator: "__".to_string(),
            prefix: None,
            uppercase: true,
        }
    }
}

impl EnvNamingConfig {
    /// Create a naming config with a prefix
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
            ..Default::default()
        }
    }

    /// Build the environment variable name from namespace and key
    pub fn build_name(&self, namespace: &str, key: &str) -> String {
        let mut name = if let Some(ref prefix) = self.prefix {
            format!("{}{}{}{}{}", prefix, self.separator, namespace, self.separator, key)
        } else {
            format!("{}{}{}", namespace, self.separator, key)
        };

        // Replace common separators with the configured one
        name = name.replace('/', &self.separator);
        name = name.replace('.', &self.separator);
        name = name.replace('-', "_");

        if self.uppercase {
            name.to_uppercase()
        } else {
            name
        }
    }

    /// Parse an environment variable name into namespace and key
    pub fn parse_name(&self, name: &str) -> Option<(String, String)> {
        let name = if self.uppercase {
            name.to_uppercase()
        } else {
            name.to_string()
        };

        // Strip prefix if present
        let name = if let Some(ref prefix) = self.prefix {
            let prefix_with_sep = format!("{}{}", prefix.to_uppercase(), self.separator);
            name.strip_prefix(&prefix_with_sep)?.to_string()
        } else {
            name
        };

        // Split on separator
        let parts: Vec<&str> = name.splitn(2, &self.separator).collect();
        if parts.len() == 2 {
            Some((parts[0].to_lowercase(), parts[1].to_lowercase()))
        } else {
            None
        }
    }
}

/// Provider for system environment variables
///
/// This provider reads configuration values from the process environment.
/// It's read-only and reflects the environment at query time.
#[derive(Debug)]
pub struct EnvProvider {
    naming: EnvNamingConfig,
}

impl EnvProvider {
    /// Create a new environment provider with default naming
    pub fn new() -> Self {
        Self {
            naming: EnvNamingConfig::default(),
        }
    }

    /// Create an environment provider with a prefix
    ///
    /// Only environment variables starting with the prefix will be considered.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            naming: EnvNamingConfig::with_prefix(prefix),
        }
    }

    /// Create an environment provider with custom naming configuration
    pub fn with_config(naming: EnvNamingConfig) -> Self {
        Self { naming }
    }
}

impl Default for EnvProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ConfigProvider for EnvProvider {
    fn name(&self) -> &str {
        "env"
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        let var_name = self.naming.build_name(namespace, key);

        match std::env::var(&var_name) {
            Ok(value) => {
                let mut pv = ProviderValue::new(value, "env");
                // Mark as secret if the key contains sensitive words
                if key.to_lowercase().contains("secret")
                    || key.to_lowercase().contains("password")
                    || key.to_lowercase().contains("token")
                    || key.to_lowercase().contains("key")
                {
                    pv.metadata.is_secret = true;
                }
                Ok(pv)
            }
            Err(std::env::VarError::NotPresent) => {
                Err(ProviderError::NotFound {
                    namespace: namespace.to_string(),
                    key: key.to_string(),
                })
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(ProviderError::ConfigurationError(
                    format!("Environment variable {} contains invalid UTF-8", var_name)
                ))
            }
        }
    }

    async fn list(&self, namespace: &str, prefix: Option<&str>) -> ProviderResult<HashMap<String, ProviderValue>> {
        let mut result = HashMap::new();
        let ns_prefix = self.naming.build_name(namespace, "");

        for (name, value) in std::env::vars() {
            let name_upper = if self.naming.uppercase {
                name.to_uppercase()
            } else {
                name.clone()
            };

            if name_upper.starts_with(&ns_prefix) {
                if let Some((_, key)) = self.naming.parse_name(&name) {
                    // Apply prefix filter if specified
                    if let Some(p) = prefix {
                        if !key.starts_with(&p.to_lowercase()) {
                            continue;
                        }
                    }
                    result.insert(key, ProviderValue::new(value, "env"));
                }
            }
        }

        Ok(result)
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        Ok(ProviderHealth::healthy("env"))
    }
}

/// Provider for .env file configuration
///
/// This provider loads configuration from a `.env` file, parsing it into
/// key-value pairs. The file is loaded once and cached in memory.
///
/// # File Format
///
/// Standard .env format is supported:
/// ```text
/// # Comment
/// KEY=value
/// NAMESPACE__KEY=value
/// QUOTED="value with spaces"
/// MULTILINE="line1\nline2"
/// ```
#[derive(Debug)]
pub struct DotEnvProvider {
    /// Path to the .env file
    path: PathBuf,
    /// Naming configuration
    naming: EnvNamingConfig,
    /// Cached values from the file
    cache: RwLock<HashMap<String, String>>,
    /// Whether the cache has been loaded
    loaded: RwLock<bool>,
}

impl DotEnvProvider {
    /// Create a provider from a .env file path
    pub fn from_file(path: impl AsRef<Path>) -> ProviderResult<Self> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(ProviderError::ConfigurationError(
                format!(".env file not found: {}", path.display())
            ));
        }

        Ok(Self {
            path,
            naming: EnvNamingConfig::default(),
            cache: RwLock::new(HashMap::new()),
            loaded: RwLock::new(false),
        })
    }

    /// Create a provider with custom naming configuration
    pub fn with_config(path: impl AsRef<Path>, naming: EnvNamingConfig) -> ProviderResult<Self> {
        let mut provider = Self::from_file(path)?;
        provider.naming = naming;
        Ok(provider)
    }

    /// Create a provider that will look for .env in standard locations
    pub fn auto() -> ProviderResult<Self> {
        // Try common locations
        let candidates = vec![
            PathBuf::from(".env"),
            PathBuf::from(".env.local"),
            std::env::current_dir()
                .ok()
                .map(|p| p.join(".env"))
                .unwrap_or_default(),
        ];

        for path in candidates {
            if path.exists() {
                return Self::from_file(path);
            }
        }

        Err(ProviderError::ConfigurationError(
            "No .env file found in standard locations".into()
        ))
    }

    /// Load and parse the .env file
    fn load_file(&self) -> ProviderResult<()> {
        let content = std::fs::read_to_string(&self.path)?;
        let mut cache = self.cache.write().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        cache.clear();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse KEY=value
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let mut value = line[eq_pos + 1..].trim().to_string();

                // Remove surrounding quotes
                if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value = value[1..value.len() - 1].to_string();
                }

                // Process escape sequences
                value = value
                    .replace("\\n", "\n")
                    .replace("\\t", "\t")
                    .replace("\\r", "\r");

                cache.insert(key, value);
            }
        }

        *self.loaded.write().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })? = true;

        Ok(())
    }

    /// Ensure the file is loaded
    fn ensure_loaded(&self) -> ProviderResult<()> {
        let loaded = *self.loaded.read().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        if !loaded {
            self.load_file()?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConfigProvider for DotEnvProvider {
    fn name(&self) -> &str {
        "dotenv"
    }

    async fn is_available(&self) -> bool {
        self.path.exists()
    }

    async fn get(&self, namespace: &str, key: &str) -> ProviderResult<ProviderValue> {
        self.ensure_loaded()?;

        let var_name = self.naming.build_name(namespace, key);
        let cache = self.cache.read().map_err(|e| {
            ProviderError::Other(format!("Failed to acquire lock: {}", e))
        })?;

        match cache.get(&var_name) {
            Some(value) => {
                let mut pv = ProviderValue::new(value.clone(), "dotenv");
                if key.to_lowercase().contains("secret")
                    || key.to_lowercase().contains("password")
                    || key.to_lowercase().contains("token")
                {
                    pv.metadata.is_secret = true;
                }
                Ok(pv)
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

        let mut result = HashMap::new();
        let ns_prefix = self.naming.build_name(namespace, "");

        for (name, value) in cache.iter() {
            if name.starts_with(&ns_prefix) {
                if let Some((_, key)) = self.naming.parse_name(name) {
                    if let Some(p) = prefix {
                        if !key.starts_with(&p.to_lowercase()) {
                            continue;
                        }
                    }
                    result.insert(key, ProviderValue::new(value.clone(), "dotenv"));
                }
            }
        }

        Ok(result)
    }

    async fn refresh(&self) -> ProviderResult<()> {
        self.load_file()
    }

    fn health_check(&self) -> ProviderResult<ProviderHealth> {
        if self.path.exists() {
            Ok(ProviderHealth::healthy("dotenv"))
        } else {
            Ok(ProviderHealth::unhealthy("dotenv", format!("File not found: {}", self.path.display())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naming_config_default() {
        let config = EnvNamingConfig::default();
        assert_eq!(config.build_name("database", "host"), "DATABASE__HOST");
        assert_eq!(config.build_name("app", "secret_key"), "APP__SECRET_KEY");
    }

    #[test]
    fn test_naming_config_with_prefix() {
        let config = EnvNamingConfig::with_prefix("MYAPP");
        assert_eq!(config.build_name("database", "host"), "MYAPP__DATABASE__HOST");
    }

    #[test]
    fn test_naming_config_parse() {
        let config = EnvNamingConfig::default();
        let (ns, key) = config.parse_name("DATABASE__HOST").unwrap();
        assert_eq!(ns, "database");
        assert_eq!(key, "host");
    }

    #[test]
    fn test_naming_handles_dots_and_slashes() {
        let config = EnvNamingConfig::default();
        assert_eq!(config.build_name("app.config", "db.host"), "APP__CONFIG__DB__HOST");
        assert_eq!(config.build_name("app/config", "db/host"), "APP__CONFIG__DB__HOST");
    }

    #[tokio::test]
    async fn test_env_provider_not_found() {
        let provider = EnvProvider::new();
        let result = provider.get("nonexistent", "key").await;
        assert!(matches!(result, Err(ProviderError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_env_provider_reads_env() {
        std::env::set_var("TEST_PROVIDER__DATABASE__HOST", "localhost");

        let provider = EnvProvider::with_prefix("TEST_PROVIDER");
        let result = provider.get("database", "host").await;

        std::env::remove_var("TEST_PROVIDER__DATABASE__HOST");

        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, "localhost");
    }

    #[test]
    fn test_dotenv_parsing() {
        use std::io::Write;

        // Create a temp .env file
        let temp_dir = std::env::temp_dir();
        let env_path = temp_dir.join("test_dotenv_parsing.env");

        let mut file = std::fs::File::create(&env_path).unwrap();
        writeln!(file, "# Comment").unwrap();
        writeln!(file, "DATABASE__HOST=localhost").unwrap();
        writeln!(file, "DATABASE__PORT=5432").unwrap();
        writeln!(file, "APP__SECRET=\"secret value\"").unwrap();
        drop(file);

        let provider = DotEnvProvider::from_file(&env_path).unwrap();

        // Test synchronous loading
        provider.load_file().unwrap();

        let cache = provider.cache.read().unwrap();
        assert_eq!(cache.get("DATABASE__HOST").unwrap(), "localhost");
        assert_eq!(cache.get("DATABASE__PORT").unwrap(), "5432");
        assert_eq!(cache.get("APP__SECRET").unwrap(), "secret value");

        // Cleanup
        std::fs::remove_file(&env_path).ok();
    }
}
