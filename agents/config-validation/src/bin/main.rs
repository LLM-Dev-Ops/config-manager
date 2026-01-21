//! Config Validation Agent CLI
//!
//! Command-line interface for the Config Validation Agent.
//!
//! # Usage
//!
//! ```bash
//! # Validate a configuration file against a schema
//! config-validate validate --config app.yaml --schema schema.json --environment production
//!
//! # Inspect configuration structure
//! config-validate inspect --config app.yaml --format json
//!
//! # Check compatibility between multiple configurations
//! config-validate compatibility --configs config1.yaml config2.yaml
//! ```
//!
//! # Exit Codes
//!
//! - 0: Success - validation passed
//! - 1: Validation failed with errors
//! - 2: Validation passed with warnings
//! - 3: Invalid input or arguments
//! - 4: File not found or inaccessible
//! - 5: Schema-related errors
//! - 10: Internal error

use clap::Parser;
use config_validation::{run_cli, ValidateCli};

fn main() {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    // Parse CLI arguments
    let cli = ValidateCli::parse();

    // Run the CLI and exit with appropriate code
    let exit_code = run_cli(cli);
    std::process::exit(exit_code.into());
}
