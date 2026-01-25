//! Schema Truth Agent entry point
//!
//! Deterministic schema validation with schema_violation_signal emission.

use clap::{Parser, Subcommand};
use schema_truth::contracts::*;
use schema_truth::engine::SchemaValidationEngine;
use schema_truth::handler::{create_router, AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "schema-truth")]
#[command(about = "Schema Truth Agent - deterministic schema validation")]
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
        #[arg(short, long, default_value = "8081", env = "PORT")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },

    /// Validate a schema file
    Validate {
        /// Path to schema file (JSON/YAML)
        #[arg(short, long)]
        file: String,

        /// Output format
        #[arg(short, long, default_value = "json")]
        output: String,
    },

    /// Check a schema (quick, no telemetry)
    Check {
        /// Path to schema file
        #[arg(short, long)]
        file: String,
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
                "Starting Schema Truth Agent on {}",
                addr
            );
            tracing::info!(
                "Agent ID: {}, Version: {}",
                SchemaViolationSignal::AGENT_ID,
                SchemaViolationSignal::AGENT_VERSION
            );

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, router).await?;
        }

        Commands::Validate { file, output } => {
            let content = std::fs::read_to_string(&file)?;
            let schema: serde_json::Value = if file.ends_with(".yaml") || file.ends_with(".yml") {
                serde_yaml::from_str(&content)?
            } else {
                serde_json::from_str(&content)?
            };

            let engine = SchemaValidationEngine::new();
            let input = SchemaValidationEngine::create_input(schema, "cli".to_string())
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let result = engine.validate(&input).await;

            match output.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                _ => {
                    if result.is_valid {
                        println!("Schema is valid");
                        println!("  Rules applied: {}", result.rules_applied.len());
                        println!("  Coverage: {:.1}%", result.coverage * 100.0);
                        println!("  Warnings: {}", result.warnings.len());
                    } else {
                        println!("Schema has violations:");
                        for v in &result.violations {
                            println!("  [{:?}] {}: {}", v.severity, v.code, v.message);
                            if let Some(path) = &v.path {
                                println!("       at: {}", path);
                            }
                        }
                    }
                }
            }

            if !result.is_valid {
                std::process::exit(1);
            }
        }

        Commands::Check { file } => {
            let content = std::fs::read_to_string(&file)?;
            let schema: serde_json::Value = if file.ends_with(".yaml") || file.ends_with(".yml") {
                serde_yaml::from_str(&content)?
            } else {
                serde_json::from_str(&content)?
            };

            let engine = SchemaValidationEngine::new();
            let input = SchemaValidationEngine::create_input(schema, "cli".to_string())
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let result = engine.validate(&input).await;

            println!(
                "{}",
                serde_json::json!({
                    "valid": result.is_valid,
                    "violations": result.violations.len(),
                    "warnings": result.warnings.len(),
                    "coverage": result.coverage,
                    "duration_ms": result.duration_ms
                })
            );

            if !result.is_valid {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
