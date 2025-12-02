//! Core logic for LLM Config Manager

pub mod config;
pub mod manager;
pub mod version;
pub mod error_utils;
pub mod benchmarks;

pub use config::*;
pub use manager::*;
pub use version::*;
pub use error_utils::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Storage error: {0}")]
    StorageError(#[from] llm_config_storage::StorageError),

    #[error("Crypto error: {0}")]
    CryptoError(#[from] llm_config_crypto::CryptoError),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
