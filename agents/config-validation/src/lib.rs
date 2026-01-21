//! Config Validation Agent
//!
//! An agent for validating configuration files with comprehensive telemetry
//! and DecisionEvent emission to ruvector-service.
//!
//! ## Features
//!
//! - **CLI Support**: Full command-line interface for validation operations
//! - **Schema Validation**: Validate configurations against JSON Schema-like definitions
//! - **Structure Inspection**: Analyze and infer configuration structure and types
//! - **Compatibility Checking**: Validate compatibility between multiple configurations
//! - **Security Analysis**: Detect potential security issues in configurations
//! - **Telemetry**: Prometheus metrics and DecisionEvent emission
//! - **Async Operations**: Non-blocking validation and event emission
//! - **Retry Logic**: Exponential backoff for ruvector-service communication
//! - **No Direct SQL**: All persistence through HTTP APIs
//! - **Contract-Driven**: Well-defined input/output schemas and validation rules
//!
//! ## Architecture
//!
//! The agent follows a read-only, contract-driven design:
//!
//! 1. **CLI** (`cli/`): Command-line interface for validation, inspection, and
//!    compatibility checking with machine-readable output.
//!
//! 2. **Contracts** (`contracts/`): Define input/output schemas, validation rules,
//!    and DecisionEvent structures for ruvector-service integration.
//!
//! 3. **Telemetry** (`telemetry/`): Handles DecisionEvent emission and metrics.
//!
//! 4. **Client** (`client/`): HTTP client for ruvector-service communication.
//!
//! 5. **Validation** (`validation/`): Core validation engine with schema support.
//!
//! 6. **Schema** (`schema/`): Schema inference and type detection.
//!
//! 7. **Compatibility** (`compatibility/`): Cross-configuration compatibility checking.
//!
//! ## CLI Usage
//!
//! ```bash
//! # Validate a configuration file
//! config-validate validate --config app.yaml --schema schema.json --environment production
//!
//! # Inspect configuration structure
//! config-validate inspect --config app.yaml --format json
//!
//! # Check compatibility between configurations
//! config-validate compatibility --configs config1.yaml config2.yaml
//! ```
//!
//! ## Example
//!
//! ```rust,no_run
//! use config_validation::{
//!     telemetry::{DecisionEventEmitter, EmitterConfig, ValidationOutput},
//!     client::RuvectorClient,
//!     contracts::{ValidationInput, ConfigValueRef, EnvironmentRef},
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create validation input using contracts
//!     let input = ValidationInput::new(
//!         "app/database",
//!         "connection_string",
//!         ConfigValueRef::String("postgres://localhost/db".to_string()),
//!         EnvironmentRef::Development,
//!         "test-user",
//!     );
//!
//!     // Create emitter
//!     let config = EmitterConfig::default();
//!     let emitter = DecisionEventEmitter::new(config);
//!
//!     // Emit validation result
//!     let output = ValidationOutput {
//!         valid: true,
//!         findings: vec![],
//!         schema_version: "1.0.0".to_string(),
//!         rules_applied: vec!["required-fields".to_string()],
//!         coverage: 0.95,
//!     };
//!
//!     let rule_results = std::collections::HashMap::new();
//!     emitter
//!         .emit_validation_result(
//!             "config content",
//!             output,
//!             &["required-fields".to_string()],
//!             &rule_results,
//!             "exec-ref-123",
//!         )
//!         .await
//!         .unwrap();
//! }
//! ```

// Core modules
pub mod cli;
pub mod client;
pub mod compatibility;
pub mod error;
pub mod handler;
pub mod schema;
pub mod telemetry;
pub mod validation;

// Contracts module - located at ../contracts relative to src/
#[path = "../contracts/mod.rs"]
pub mod contracts;

// Re-export commonly used types from telemetry
pub use client::RuvectorClient;
pub use telemetry::{
    DecisionEventEmitter, EmitterConfig, TelemetryConfig, TelemetryError,
    ValidationMetrics, ValidationMetricsRegistry,
};

// Re-export handler types for edge function deployment
pub use handler::{
    create_router, handle_request, ApiError, ApiResponse, EdgeFunctionConfig,
    EdgeFunctionError, ErrorInfo, HandlerState, HealthResponse, HealthStatus,
    InspectionRequest, InspectionResult, MiddlewareState, ResponseMetadata,
    ValidationError, ValidationOptions, ValidationRequest, ValidationResult,
    ValidationStats, ValidationWarning,
};

// Re-export contract types for external use
pub use contracts::{
    // Core input/output types
    ValidationInput, ValidationOutput, ValidationIssue, IssueSeverity,
    ConfigValueRef, EnvironmentRef, RuleRef,
    // Schema types
    ConfigSchema, FieldRule, FieldType, ValidationConstraint,
    EnvironmentRule, CompatibilityRule, DeprecationInfo, SchemaDefinition,
    // Decision event types
    DecisionEvent, ValidationOutputs,
    // Validation rule trait
    ValidationRule,
};

// Re-export additional types from contract submodules (schemas and decision_event are pub)
pub use contracts::schemas::{EnvironmentRuleType, CompatibilityRequirement};
pub use contracts::decision_event::{DecisionType, PerformanceMetrics, IssueSummary};

// Re-export CLI types for command-line usage
pub use cli::{ExitCode, OutputFormat, ValidateCli, ValidateCommands};
pub use cli::output::ValidationOutput as CliValidationOutput;

// Re-export validation engine types
pub use validation::{
    ValidationContext, ValidationFinding, ValidationResult as CliValidationResult,
    ValidationSeverity, Validator,
};

// Re-export schema inference types
pub use schema::{InferredSchema, SchemaInference, TypeInfo, TypeName};

// Re-export compatibility checking types
pub use compatibility::{
    CompatibilityChecker, CompatibilityResult, Conflict, ConflictSeverity,
};

// Re-export error types
pub use error::ValidationError as CliValidationError;

/// Agent version (from Cargo.toml)
pub const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Agent identifier
pub const AGENT_ID: &str = "config-validation-agent";

/// Run the CLI application
///
/// This is the main entry point for the CLI binary.
///
/// # Example
///
/// ```rust,no_run
/// use clap::Parser;
/// use config_validation::{ValidateCli, run_cli};
///
/// fn main() {
///     let cli = ValidateCli::parse();
///     let exit_code = run_cli(cli);
///     std::process::exit(exit_code.into());
/// }
/// ```
pub fn run_cli(cli: ValidateCli) -> ExitCode {
    match cli::run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            if e.is_user_error() {
                ExitCode::InvalidInput
            } else {
                ExitCode::InternalError
            }
        }
    }
}
