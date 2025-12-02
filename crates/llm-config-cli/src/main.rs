//! LLM Config Manager CLI

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use llm_config_core::{ConfigManager, ConfigValue, Environment};
use llm_config_core::benchmarks;
use llm_config_crypto::{Algorithm, SecretKey};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "llm-config")]
#[command(about = "LLM Configuration Manager - Secure configuration and secrets management", long_about = None)]
#[command(version)]
struct Cli {
    /// Storage directory path
    #[arg(short, long, default_value = ".llm-config")]
    storage: PathBuf,

    /// Encryption key (base64 encoded)
    #[arg(short = 'k', long, env = "LLM_CONFIG_KEY")]
    encryption_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get a configuration value
    Get {
        /// Namespace (e.g., "org/project/service")
        namespace: String,

        /// Configuration key
        key: String,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,

        /// Apply environment overrides
        #[arg(short = 'o', long)]
        with_overrides: bool,
    },

    /// Set a configuration value
    Set {
        /// Namespace (e.g., "org/project/service")
        namespace: String,

        /// Configuration key
        key: String,

        /// Configuration value
        value: String,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,

        /// User performing the operation
        #[arg(short, long, default_value = "cli-user")]
        user: String,

        /// Store as a secret (encrypted)
        #[arg(short, long)]
        secret: bool,
    },

    /// List configurations in a namespace
    List {
        /// Namespace (e.g., "org/project/service")
        namespace: String,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,

        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Delete a configuration
    Delete {
        /// Namespace
        namespace: String,

        /// Configuration key
        key: String,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,

        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Show version history
    History {
        /// Namespace
        namespace: String,

        /// Configuration key
        key: String,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,
    },

    /// Rollback to a specific version
    Rollback {
        /// Namespace
        namespace: String,

        /// Configuration key
        key: String,

        /// Target version number
        version: u64,

        /// Environment
        #[arg(short, long, value_enum, default_value = "development")]
        env: Env,
    },

    /// Export all configurations
    Export {
        /// Export directory path
        path: PathBuf,
    },

    /// Generate a new encryption key
    Keygen,

    /// Run benchmarks
    Run {
        /// Run all benchmarks
        #[arg(short, long)]
        all: bool,

        /// Run benchmarks for a specific category (config, cache, crypto, secrets, storage)
        #[arg(short, long)]
        category: Option<String>,

        /// Run a specific benchmark by target ID
        #[arg(short, long)]
        target: Option<String>,

        /// List available benchmarks
        #[arg(short, long)]
        list: bool,

        /// Output directory for benchmark results
        #[arg(short, long, default_value = ".")]
        output: PathBuf,

        /// Output format (json, table)
        #[arg(short, long, value_enum, default_value = "table")]
        format: BenchOutputFormat,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum BenchOutputFormat {
    Table,
    Json,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Env {
    Base,
    Dev,
    Development,
    Staging,
    Stage,
    Prod,
    Production,
    Edge,
}

impl From<Env> for Environment {
    fn from(env: Env) -> Self {
        match env {
            Env::Base => Environment::Base,
            Env::Dev | Env::Development => Environment::Development,
            Env::Staging | Env::Stage => Environment::Staging,
            Env::Prod | Env::Production => Environment::Production,
            Env::Edge => Environment::Edge,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Yaml,
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    // Create manager
    let mut manager = ConfigManager::new(&cli.storage)?;

    // Set encryption key if provided
    let has_key = cli.encryption_key.is_some();
    if let Some(key_str) = cli.encryption_key {
        let key = SecretKey::from_base64(Algorithm::Aes256Gcm, &key_str)?;
        manager = manager.with_encryption_key(key);
    }

    match cli.command {
        Commands::Get {
            namespace,
            key,
            env,
            with_overrides,
        } => {
            let env: Environment = env.into();

            if with_overrides {
                if let Some(value) = manager.get_with_overrides(&namespace, &key, env)? {
                    println!("{}", format_value(&value));
                } else {
                    println!("{}", "Configuration not found".yellow());
                }
            } else {
                if let Some(entry) = manager.get(&namespace, &key, env)? {
                    println!("{}", "Configuration:".green().bold());
                    println!("  Namespace: {}", entry.namespace);
                    println!("  Key: {}", entry.key);
                    println!("  Environment: {}", entry.environment);
                    println!("  Value: {}", format_value(&entry.value));
                    println!("  Version: {}", entry.version);
                    println!("  Updated: {}", entry.metadata.updated_at);
                    println!("  Updated by: {}", entry.metadata.updated_by);
                } else {
                    println!("{}", "Configuration not found".yellow());
                }
            }
        }

        Commands::Set {
            namespace,
            key,
            value,
            env,
            user,
            secret,
        } => {
            let env: Environment = env.into();

            let entry = if secret {
                if !has_key {
                    anyhow::bail!("Encryption key required for secrets. Set --encryption-key or LLM_CONFIG_KEY environment variable.");
                }
                manager.set_secret(&namespace, &key, value.as_bytes(), env, &user)?
            } else {
                let config_value = parse_value(&value)?;
                manager.set(&namespace, &key, config_value, env, &user)?
            };

            println!("{}", "Configuration saved successfully!".green().bold());
            println!("  Version: {}", entry.version);
            println!("  ID: {}", entry.id);
        }

        Commands::List {
            namespace,
            env,
            format,
        } => {
            let env: Environment = env.into();
            let entries = manager.list(&namespace, env)?;

            if entries.is_empty() {
                println!("{}", "No configurations found".yellow());
                return Ok(());
            }

            match format {
                OutputFormat::Table => {
                    println!("{}", format!("Configurations in {} ({})", namespace, env).green().bold());
                    println!();
                    for entry in entries {
                        println!("  {} {} = {}", "•".blue(), entry.key.bold(), format_value(&entry.value));
                        println!("    Version: {} | Updated: {} by {}",
                            entry.version,
                            entry.metadata.updated_at.format("%Y-%m-%d %H:%M:%S"),
                            entry.metadata.updated_by
                        );
                        println!();
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&entries)?);
                }
                OutputFormat::Yaml => {
                    println!("{}", serde_yaml::to_string(&entries)?);
                }
            }
        }

        Commands::Delete {
            namespace,
            key,
            env,
            yes,
        } => {
            let env: Environment = env.into();

            if !yes {
                print!("Delete configuration {}:{} in {} environment? [y/N] ", namespace, key, env);
                use std::io::{self, Write};
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            let deleted = manager.delete(&namespace, &key, env)?;

            if deleted {
                println!("{}", "Configuration deleted successfully!".green().bold());
            } else {
                println!("{}", "Configuration not found".yellow());
            }
        }

        Commands::History {
            namespace,
            key,
            env,
        } => {
            let env: Environment = env.into();
            let history = manager.get_history(&namespace, &key, env)?;

            if history.is_empty() {
                println!("{}", "No version history found".yellow());
                return Ok(());
            }

            println!("{}", format!("Version history for {}:{}", namespace, key).green().bold());
            println!();

            for version in history {
                println!("  {} Version {}", "→".blue(), version.version.to_string().bold());
                println!("    Value: {}", format_value(&version.value));
                println!("    Created: {} by {}", version.created_at.format("%Y-%m-%d %H:%M:%S"), version.created_by);
                if let Some(desc) = version.change_description {
                    println!("    Note: {}", desc);
                }
                println!();
            }
        }

        Commands::Rollback {
            namespace,
            key,
            version,
            env,
        } => {
            let env: Environment = env.into();

            if let Some(entry) = manager.rollback(&namespace, &key, env, version)? {
                println!("{}", "Rollback successful!".green().bold());
                println!("  New version: {}", entry.version);
                println!("  Value: {}", format_value(&entry.value));
            } else {
                println!("{}", format!("Version {} not found", version).yellow());
            }
        }

        Commands::Export { path } => {
            let count = manager.export_all(&path)?;
            println!("{}", format!("Exported {} configurations to {}", count, path.display()).green().bold());
        }

        Commands::Keygen => {
            let key = SecretKey::generate(Algorithm::Aes256Gcm)?;
            println!("{}", "Generated encryption key:".green().bold());
            println!();
            println!("{}", key.to_base64());
            println!();
            println!("Set this key using:");
            println!("  {} export LLM_CONFIG_KEY=\"{}\"", "•".blue(), key.to_base64());
            println!("  {} llm-config --encryption-key <key> ...", "•".blue());
        }

        Commands::Run {
            all,
            category,
            target,
            list,
            output,
            format,
        } => {
            if list {
                // List available benchmarks
                println!("{}", "Available Benchmarks:".green().bold());
                println!();
                let bench_list = benchmarks::list_benchmarks();
                for info in bench_list {
                    println!("  {} [{}] {}", "•".blue(), info.category.yellow(), info.id.bold());
                    println!("    {}", info.description);
                }
                return Ok(());
            }

            let results = if let Some(target_id) = target {
                // Run specific benchmark
                println!("{}", format!("Running benchmark: {}", target_id).green().bold());
                match benchmarks::run_benchmark(&target_id) {
                    Some(result) => vec![result],
                    None => {
                        println!("{}", format!("Benchmark '{}' not found", target_id).red());
                        return Ok(());
                    }
                }
            } else if let Some(cat) = category {
                // Run benchmarks by category
                println!("{}", format!("Running {} benchmarks...", cat).green().bold());
                benchmarks::run_benchmarks_by_category(&cat)
            } else if all {
                // Run all benchmarks
                println!("{}", "Running all benchmarks...".green().bold());
                benchmarks::run_all_benchmarks()
            } else {
                // Default: run all
                println!("{}", "Running all benchmarks...".green().bold());
                benchmarks::run_all_benchmarks()
            };

            if results.is_empty() {
                println!("{}", "No benchmarks were run.".yellow());
                return Ok(());
            }

            // Display results
            match format {
                BenchOutputFormat::Table => {
                    println!();
                    println!("{}", "Benchmark Results:".green().bold());
                    println!();
                    println!("{:<25} {:>15} {:>15}", "Target".bold(), "Duration (ms)".bold(), "Ops/sec".bold());
                    println!("{}", "-".repeat(60));

                    for result in &results {
                        let duration_ms = result.metrics
                            .get("duration_ms")
                            .and_then(|v| v.as_f64())
                            .map(|v| format!("{:.3}", v))
                            .unwrap_or_else(|| "-".to_string());

                        let ops_per_sec = result.metrics
                            .get("throughput_ops_per_sec")
                            .and_then(|v| v.as_f64())
                            .map(|v| format!("{:.2}", v))
                            .unwrap_or_else(|| "-".to_string());

                        println!("{:<25} {:>15} {:>15}", result.target_id, duration_ms, ops_per_sec);
                    }
                    println!();
                }
                BenchOutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&results)?);
                }
            }

            // Save results to output directory
            if let Err(e) = benchmarks::io::write_benchmark_run(&output, &results) {
                eprintln!("{} Failed to write results: {}", "Warning:".yellow(), e);
            } else {
                println!("{} Results saved to {}", "✓".green(), output.join("benchmarks/output").display());
            }

            // Update summary
            if let Err(e) = benchmarks::markdown::update_summary(&output) {
                eprintln!("{} Failed to update summary: {}", "Warning:".yellow(), e);
            }
        }
    }

    Ok(())
}

fn parse_value(s: &str) -> anyhow::Result<ConfigValue> {
    // Try to parse as JSON first
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(s) {
        return Ok(match json_value {
            serde_json::Value::String(s) => ConfigValue::String(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ConfigValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    ConfigValue::Float(f)
                } else {
                    ConfigValue::String(s.to_string())
                }
            }
            serde_json::Value::Bool(b) => ConfigValue::Boolean(b),
            serde_json::Value::Array(arr) => {
                let values: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| parse_value(&v.to_string()))
                    .collect();
                ConfigValue::Array(values?)
            }
            serde_json::Value::Object(_) => {
                // For objects, store as JSON string
                ConfigValue::String(s.to_string())
            }
            serde_json::Value::Null => ConfigValue::String(String::new()),
        });
    }

    // Otherwise treat as string
    Ok(ConfigValue::String(s.to_string()))
}

fn format_value(value: &ConfigValue) -> String {
    match value {
        ConfigValue::String(s) => s.clone(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Float(f) => f.to_string(),
        ConfigValue::Boolean(b) => b.to_string(),
        ConfigValue::Array(arr) => format!("[{}]", arr.iter().map(format_value).collect::<Vec<_>>().join(", ")),
        ConfigValue::Object(_) => "<object>".to_string(),
        ConfigValue::Secret(_) => "<encrypted>".yellow().to_string(),
    }
}
