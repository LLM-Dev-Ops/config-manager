//! LLM Config Manager API Server

use clap::Parser;
use llm_config_api::{serve, ServerConfig};
use llm_config_core::ConfigManager;
use llm_config_crypto::{Algorithm, SecretKey};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "llm-config-server")]
#[command(about = "LLM Config Manager REST API Server", long_about = None)]
#[command(version)]
struct Cli {
    /// Storage directory path
    #[arg(short, long, default_value = ".llm-config")]
    storage: PathBuf,

    /// Encryption key (base64 encoded)
    #[arg(short = 'k', long, env = "LLM_CONFIG_KEY")]
    encryption_key: Option<String>,

    /// Server host
    #[arg(long, env = "SERVER_HOST", default_value = "0.0.0.0")]
    host: String,

    /// Server port
    #[arg(short, long, env = "SERVER_PORT", default_value = "8080")]
    port: u16,

    /// Disable CORS
    #[arg(long)]
    no_cors: bool,

    /// Disable security middleware
    #[arg(long)]
    no_security: bool,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Create manager
    let mut manager = ConfigManager::new(&cli.storage)?;

    // Set encryption key if provided
    if let Some(key_str) = cli.encryption_key {
        let key = SecretKey::from_base64(Algorithm::Aes256Gcm, &key_str)?;
        manager = manager.with_encryption_key(key);
        tracing::info!("Encryption key configured");
    } else {
        tracing::warn!("No encryption key provided - secret operations will fail");
    }

    let manager = Arc::new(manager);

    // Create server configuration
    let config = ServerConfig {
        host: cli.host,
        port: cli.port,
        enable_cors: !cli.no_cors,
        enable_security: !cli.no_security,
    };

    tracing::info!(
        "Starting LLM Config Manager API Server on {}:{}",
        config.host,
        config.port
    );
    tracing::info!("Storage directory: {}", cli.storage.display());
    tracing::info!("CORS enabled: {}", config.enable_cors);
    tracing::info!("Security middleware enabled: {}", config.enable_security);

    // Start the server
    serve(manager, config).await?;

    Ok(())
}
