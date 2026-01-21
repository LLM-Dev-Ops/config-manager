//! CLI command definitions for the Config Validation Agent
//!
//! Provides Clap-based command definitions for validating configurations,
//! inspecting schemas, and checking cross-agent compatibility.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use super::output::{OutputFormat, ValidationOutput};
use super::ExitCode;
use crate::error::ValidationError;

/// Config Validation Agent CLI
///
/// Validate configurations against schemas, inspect configuration structures,
/// and check cross-agent compatibility.
#[derive(Parser, Debug)]
#[command(name = "config-validate")]
#[command(about = "Config Validation Agent - Validate and inspect configurations", long_about = None)]
#[command(version)]
pub struct ValidateCli {
    /// Output verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: ValidateCommands,
}

/// Available validation commands
#[derive(Subcommand, Debug)]
pub enum ValidateCommands {
    /// Validate configuration against schemas
    ///
    /// Validates a configuration file against an optional schema file.
    /// If no schema is provided, performs structural validation only.
    Validate {
        /// Path to the configuration file to validate
        #[arg(short, long)]
        config: PathBuf,

        /// Path to the schema file (optional)
        ///
        /// If not provided, performs structural validation only.
        #[arg(short, long)]
        schema: Option<PathBuf>,

        /// Target environment for validation rules
        #[arg(short, long, default_value = "production")]
        environment: String,

        /// Output format for validation results
        #[arg(long, value_enum, default_value = "table")]
        format: Option<OutputFormat>,

        /// Enable strict validation mode
        ///
        /// In strict mode, warnings are treated as errors.
        #[arg(long)]
        strict: bool,
    },

    /// Inspect configuration schema and structure
    ///
    /// Analyzes a configuration file and displays its structure,
    /// inferred types, and detected patterns.
    Inspect {
        /// Path to the configuration file to inspect
        #[arg(short, long)]
        config: PathBuf,

        /// Output format for inspection results
        #[arg(long, value_enum, default_value = "table")]
        format: Option<OutputFormat>,
    },

    /// Check cross-agent configuration compatibility
    ///
    /// Validates that multiple configuration files are compatible
    /// with each other (e.g., no conflicting values, matching interfaces).
    Compatibility {
        /// Paths to configuration files to check for compatibility
        #[arg(short, long, num_args = 2..)]
        configs: Vec<PathBuf>,

        /// Output format for compatibility results
        #[arg(long, value_enum, default_value = "table")]
        format: Option<OutputFormat>,
    },
}

/// Environment types for validation context
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum ValidationEnvironment {
    /// Base configuration (no environment-specific rules)
    Base,
    /// Development environment
    Dev,
    /// Development environment (alias)
    Development,
    /// Staging environment
    Staging,
    /// Staging environment (alias)
    Stage,
    /// Production environment
    Prod,
    /// Production environment (alias)
    Production,
    /// Edge deployment environment
    Edge,
}

impl std::fmt::Display for ValidationEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationEnvironment::Base => write!(f, "base"),
            ValidationEnvironment::Dev | ValidationEnvironment::Development => {
                write!(f, "development")
            }
            ValidationEnvironment::Staging | ValidationEnvironment::Stage => write!(f, "staging"),
            ValidationEnvironment::Prod | ValidationEnvironment::Production => {
                write!(f, "production")
            }
            ValidationEnvironment::Edge => write!(f, "edge"),
        }
    }
}

impl std::str::FromStr for ValidationEnvironment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "base" => Ok(ValidationEnvironment::Base),
            "dev" | "development" => Ok(ValidationEnvironment::Development),
            "staging" | "stage" => Ok(ValidationEnvironment::Staging),
            "prod" | "production" => Ok(ValidationEnvironment::Production),
            "edge" => Ok(ValidationEnvironment::Edge),
            _ => Err(format!("Unknown environment: {}", s)),
        }
    }
}

/// Execute the validate command
pub fn execute_validate(
    config: PathBuf,
    schema: Option<PathBuf>,
    environment: String,
    format: Option<OutputFormat>,
    strict: bool,
) -> Result<ExitCode, ValidationError> {
    use crate::validation::{ValidationContext, ValidationSeverity, Validator};

    // Parse environment
    let env: ValidationEnvironment = environment
        .parse()
        .map_err(|e: String| ValidationError::InvalidInput(e))?;

    // Create validation context
    let context = ValidationContext::new()
        .with_environment(&env.to_string())
        .with_strict_mode(strict);

    // Load configuration
    let config_content = std::fs::read_to_string(&config).map_err(|e| {
        ValidationError::FileError(format!(
            "Failed to read config file '{}': {}",
            config.display(),
            e
        ))
    })?;

    // Parse configuration based on extension
    let config_value = parse_config_file(&config, &config_content)?;

    // Create validator
    let mut validator = Validator::new(context);

    // Load schema if provided
    if let Some(schema_path) = &schema {
        let schema_content = std::fs::read_to_string(schema_path).map_err(|e| {
            ValidationError::FileError(format!(
                "Failed to read schema file '{}': {}",
                schema_path.display(),
                e
            ))
        })?;
        validator.load_schema(&schema_content)?;
    }

    // Perform validation
    let result = validator.validate(&config_value)?;

    // Format and output results
    let output_format = format.unwrap_or(OutputFormat::Table);
    let output = ValidationOutput::from_result(&result);
    output.render(output_format)?;

    // Determine exit code
    let has_errors = result
        .findings
        .iter()
        .any(|f| f.severity == ValidationSeverity::Error);
    let has_warnings = result
        .findings
        .iter()
        .any(|f| f.severity == ValidationSeverity::Warning);

    Ok(ExitCode::from_validation_result(has_errors, has_warnings))
}

/// Execute the inspect command
pub fn execute_inspect(
    config: PathBuf,
    format: Option<OutputFormat>,
) -> Result<ExitCode, ValidationError> {
    use crate::schema::{SchemaInference, TypeInfo};

    // Load configuration
    let config_content = std::fs::read_to_string(&config).map_err(|e| {
        ValidationError::FileError(format!(
            "Failed to read config file '{}': {}",
            config.display(),
            e
        ))
    })?;

    // Parse configuration
    let config_value = parse_config_file(&config, &config_content)?;

    // Infer schema
    let inference = SchemaInference::new();
    let inferred_schema = inference.infer(&config_value)?;

    // Build inspection output
    let output_format = format.unwrap_or(OutputFormat::Table);

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&inferred_schema)
                .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&inferred_schema)
                .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
            println!("{}", yaml);
        }
        OutputFormat::Table => {
            print_schema_table(&inferred_schema, &config);
        }
    }

    Ok(ExitCode::Success)
}

/// Execute the compatibility command
pub fn execute_compatibility(
    configs: Vec<PathBuf>,
    format: Option<OutputFormat>,
) -> Result<ExitCode, ValidationError> {
    use crate::compatibility::{CompatibilityChecker, CompatibilityResult};

    if configs.len() < 2 {
        return Err(ValidationError::InvalidInput(
            "At least 2 configuration files are required for compatibility check".to_string(),
        ));
    }

    // Load all configurations
    let mut config_values = Vec::new();
    for config_path in &configs {
        let content = std::fs::read_to_string(config_path).map_err(|e| {
            ValidationError::FileError(format!(
                "Failed to read config file '{}': {}",
                config_path.display(),
                e
            ))
        })?;
        let value = parse_config_file(config_path, &content)?;
        config_values.push((config_path.clone(), value));
    }

    // Check compatibility
    let checker = CompatibilityChecker::new();
    let result = checker.check(&config_values)?;

    // Format and output results
    let output_format = format.unwrap_or(OutputFormat::Table);

    match output_format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&result)
                .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
            println!("{}", yaml);
        }
        OutputFormat::Table => {
            print_compatibility_table(&result);
        }
    }

    // Determine exit code
    let has_errors = !result.is_compatible;
    let has_warnings = !result.warnings.is_empty();

    Ok(ExitCode::from_validation_result(has_errors, has_warnings))
}

/// Parse a configuration file based on its extension
fn parse_config_file(
    path: &PathBuf,
    content: &str,
) -> Result<serde_json::Value, ValidationError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "json" => serde_json::from_str(content)
            .map_err(|e| ValidationError::ParseError(format!("Invalid JSON: {}", e))),
        "yaml" | "yml" => serde_yaml::from_str(content)
            .map_err(|e| ValidationError::ParseError(format!("Invalid YAML: {}", e))),
        "toml" => {
            let toml_value: toml::Value = toml::from_str(content)
                .map_err(|e| ValidationError::ParseError(format!("Invalid TOML: {}", e)))?;
            // Convert TOML to JSON Value for uniform processing
            let json_str = serde_json::to_string(&toml_value)
                .map_err(|e| ValidationError::SerializationError(e.to_string()))?;
            serde_json::from_str(&json_str)
                .map_err(|e| ValidationError::ParseError(format!("Conversion error: {}", e)))
        }
        _ => Err(ValidationError::InvalidInput(format!(
            "Unsupported file format: {}. Supported formats: json, yaml, yml, toml",
            extension
        ))),
    }
}

/// Print schema inspection results in table format
fn print_schema_table(schema: &crate::schema::InferredSchema, config_path: &PathBuf) {
    use colored::Colorize;

    println!(
        "{}",
        format!("Configuration Schema: {}", config_path.display())
            .green()
            .bold()
    );
    println!();

    println!("{}", "Structure:".cyan().bold());
    print_type_tree(&schema.root, "", true);
    println!();

    if !schema.patterns.is_empty() {
        println!("{}", "Detected Patterns:".cyan().bold());
        for pattern in &schema.patterns {
            println!("  {} {}", "-".blue(), pattern);
        }
        println!();
    }

    if !schema.constraints.is_empty() {
        println!("{}", "Inferred Constraints:".cyan().bold());
        for constraint in &schema.constraints {
            println!("  {} {}", "-".blue(), constraint);
        }
        println!();
    }

    println!("{}", "Statistics:".cyan().bold());
    println!("  Total fields: {}", schema.field_count);
    println!("  Depth: {}", schema.max_depth);
    println!("  Arrays: {}", schema.array_count);
    println!("  Objects: {}", schema.object_count);
}

/// Recursively print type tree
fn print_type_tree(type_info: &crate::schema::TypeInfo, prefix: &str, is_last: bool) {
    use colored::Colorize;

    let connector = if is_last { "└── " } else { "├── " };
    let new_prefix = if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };

    let type_str = match &type_info.type_name {
        crate::schema::TypeName::String => "string".yellow(),
        crate::schema::TypeName::Integer => "integer".cyan(),
        crate::schema::TypeName::Float => "float".cyan(),
        crate::schema::TypeName::Boolean => "boolean".magenta(),
        crate::schema::TypeName::Array => "array".blue(),
        crate::schema::TypeName::Object => "object".green(),
        crate::schema::TypeName::Null => "null".white(),
        crate::schema::TypeName::Mixed(types) => format!("mixed({})", types.join("|")).red(),
    };

    let required_str = if type_info.required {
        "*".red().to_string()
    } else {
        "".to_string()
    };

    println!(
        "{}{}{}{}: {}",
        prefix,
        connector,
        type_info.name.bold(),
        required_str,
        type_str
    );

    let children: Vec<_> = type_info.children.iter().collect();
    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        print_type_tree(child, &new_prefix, is_last_child);
    }
}

/// Print compatibility results in table format
fn print_compatibility_table(result: &crate::compatibility::CompatibilityResult) {
    use colored::Colorize;

    let status = if result.is_compatible {
        "COMPATIBLE".green().bold()
    } else {
        "INCOMPATIBLE".red().bold()
    };

    println!("{}", "Compatibility Check Results".cyan().bold());
    println!();
    println!("Status: {}", status);
    println!();

    if !result.conflicts.is_empty() {
        println!("{}", "Conflicts:".red().bold());
        for conflict in &result.conflicts {
            println!(
                "  {} {} at '{}'",
                "x".red(),
                conflict.description,
                conflict.path
            );
            println!(
                "    {} {}",
                "File 1:".dimmed(),
                conflict.value1.to_string().yellow()
            );
            println!(
                "    {} {}",
                "File 2:".dimmed(),
                conflict.value2.to_string().yellow()
            );
        }
        println!();
    }

    if !result.warnings.is_empty() {
        println!("{}", "Warnings:".yellow().bold());
        for warning in &result.warnings {
            println!("  {} {}", "!".yellow(), warning);
        }
        println!();
    }

    if !result.suggestions.is_empty() {
        println!("{}", "Suggestions:".blue().bold());
        for suggestion in &result.suggestions {
            println!("  {} {}", "->".blue(), suggestion);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_environment_parsing() {
        assert_eq!(
            "dev".parse::<ValidationEnvironment>().unwrap(),
            ValidationEnvironment::Development
        );
        assert_eq!(
            "production".parse::<ValidationEnvironment>().unwrap(),
            ValidationEnvironment::Production
        );
        assert!("invalid".parse::<ValidationEnvironment>().is_err());
    }

    #[test]
    fn test_validation_environment_display() {
        assert_eq!(ValidationEnvironment::Development.to_string(), "development");
        assert_eq!(ValidationEnvironment::Production.to_string(), "production");
        assert_eq!(ValidationEnvironment::Dev.to_string(), "development");
    }

    #[test]
    fn test_parse_config_json() {
        let content = r#"{"key": "value", "number": 42}"#;
        let path = PathBuf::from("test.json");
        let result = parse_config_file(&path, content);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["key"], "value");
        assert_eq!(value["number"], 42);
    }

    #[test]
    fn test_parse_config_yaml() {
        let content = "key: value\nnumber: 42";
        let path = PathBuf::from("test.yaml");
        let result = parse_config_file(&path, content);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["key"], "value");
        assert_eq!(value["number"], 42);
    }

    #[test]
    fn test_parse_config_unsupported() {
        let content = "some content";
        let path = PathBuf::from("test.txt");
        let result = parse_config_file(&path, content);
        assert!(result.is_err());
    }
}
