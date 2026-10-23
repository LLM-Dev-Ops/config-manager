//! External Configuration Providers
//!
//! This module provides trait-based interfaces for consuming configuration
//! and secrets from multiple external sources. These are additive adapters
//! that extend Config Manager's capabilities without modifying existing
//! storage or configuration logic.
//!
//! # Supported Providers
//!
//! - **Environment Variables**: Load from `.env` files and raw environment
//! - **OS Keyring**: Platform-specific secure credential storage
//! - **Local Encrypted Files**: AES-encrypted local config files
//! - **Config Bundles**: JSON, TOML, and YAML configuration files
//! - **Cloud Secret Managers**: AWS SSM/Secrets Manager, GCP Secret Manager, Azure Key Vault
//!
//! # Architecture
//!
//! All providers implement the `ConfigProvider` trait, enabling a unified
//! interface for consuming configuration from any external source. Providers
//! are runtime-selectable and do not add compile-time dependencies unless
//! cloud SDK features are explicitly enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use llm_config_core::providers::{ConfigProvider, EnvProvider, ProviderChain};
//!
//! // Create a chain of providers with priority ordering
//! let chain = ProviderChain::new()
//!     .add_provider(EnvProvider::new())
//!     .add_provider(FileProvider::from_path("config.toml")?);
//!
//! // Fetch a value - first provider to return wins
//! let value = chain.get("database", "connection_string").await?;
//! ```

pub mod bundles;
pub mod chain;
pub mod cloud;
pub mod encrypted;
pub mod env;
pub mod keyring;
pub mod traits;
pub mod vault;

// Re-export core types
pub use chain::ProviderChain;
pub use traits::{ConfigProvider, ProviderError, ProviderResult, SecretProvider};

// Re-export provider implementations
pub use bundles::{BundleProvider, JsonProvider, TomlProvider, YamlProvider};
pub use cloud::{
    AwsSecretsManagerProvider, AwsSsmProvider, AzureKeyVaultProvider, CloudProviderConfig,
    GcpSecretManagerProvider,
};
pub use encrypted::EncryptedFileProvider;
pub use env::{DotEnvProvider, EnvProvider};
pub use keyring::KeyringProvider;
pub use vault::{VaultAuthMethod, VaultConfig, VaultProvider};
