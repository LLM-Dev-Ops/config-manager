//! Integration Health Agent entry point
//!
//! Deterministic external adapter health monitoring with integration_health_signal emission.

use clap::{Parser, Subcommand};
use integration_health::contracts::*;
use integration_health::engine::HealthCheckEngine;
use integration_health::handler::{create_router, AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "integration-health")]
#[command(about = "Integration Health Agent - external adapter health monitoring")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8082", env = "PORT")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },

    /// Check health of an adapter
    Check {
        /// Adapter type
        #[arg(short = 't', long)]
        adapter_type: String,

        /// Endpoint URL
        #[arg(short, long)]
        endpoint: String,

        /// Timeout in milliseconds
        #[arg(long, default_value = "500")]
        timeout: u64,
    },

    /// Probe multiple adapters from config file
    Probe {
        /// Path to adapters config file (JSON/YAML)
        #[arg(short, long)]
        file: String,

        /// Run checks in parallel
        #[arg(long, default_value = "true")]
        parallel: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, host } => {
            let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
            let state = Arc::new(AppState::new());
            let router = create_router(state);

            tracing::info!(
                "Starting Integration Health Agent on {}",
                addr
            );
            tracing::info!(
                "Agent ID: {}, Version: {}",
                IntegrationHealthSignal::AGENT_ID,
                IntegrationHealthSignal::AGENT_VERSION
            );

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, router).await?;
        }

        Commands::Check {
            adapter_type,
            endpoint,
            timeout,
        } => {
            let adapter_type = match adapter_type.to_lowercase().as_str() {
                "http" => AdapterType::Http,
                "redis" => AdapterType::Redis,
                "postgres" => AdapterType::Postgres,
                "mysql" => AdapterType::Mysql,
                "vault" => AdapterType::HashicorpVault,
                "tcp" => AdapterType::Tcp,
                "kafka" => AdapterType::Kafka,
                "rabbitmq" => AdapterType::Rabbitmq,
                _ => {
                    eprintln!("Unknown adapter type: {}", adapter_type);
                    std::process::exit(1);
                }
            };

            let adapter = AdapterConfig {
                id: "cli-check".to_string(),
                adapter_type,
                endpoint,
                auth: None,
                health_path: None,
                properties: std::collections::HashMap::new(),
            };

            let mut input = HealthCheckEngine::create_input(vec![adapter], "cli".to_string());
            input.options.timeout_ms = timeout;

            let engine = HealthCheckEngine::new();
            let result = engine.check(&input).await;

            if let Some(r) = result.adapter_results.first() {
                println!(
                    "{}",
                    serde_json::json!({
                        "adapter_id": r.adapter_id,
                        "status": r.status,
                        "latency_ms": r.latency_ms,
                        "error": r.error,
                    })
                );

                if r.status == HealthStatus::Unhealthy {
                    std::process::exit(1);
                }
            }
        }

        Commands::Probe { file, parallel } => {
            let content = std::fs::read_to_string(&file)?;
            let adapters: Vec<AdapterConfig> = if file.ends_with(".yaml") || file.ends_with(".yml")
            {
                serde_yaml::from_str(&content)?
            } else {
                serde_json::from_str(&content)?
            };

            let mut input = HealthCheckEngine::create_input(adapters, "cli".to_string());
            input.options.parallel = parallel;

            let engine = HealthCheckEngine::new();
            let result = engine.check(&input).await;

            println!("{}", serde_json::to_string_pretty(&result)?);

            if !result.is_healthy {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
