//! Schema definitions for configuration validation
//!
//! This module provides comprehensive schema definitions for validating
//! configuration structures, including field-level rules, environment-specific
//! constraints, and cross-service compatibility checks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete schema definition for a configuration namespace
///
/// Defines the expected structure, types, and constraints for all
/// configuration values within a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    /// Schema identifier (typically namespace path)
    pub id: String,

    /// Schema version for compatibility tracking
    pub version: String,

    /// Human-readable schema name
    pub name: String,

    /// Schema description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Field definitions
    #[serde(default)]
    pub fields: HashMap<String, FieldRule>,

    /// Environment-specific rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_rules: Vec<EnvironmentRule>,

    /// Compatibility rules with other services/agents
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatibility_rules: Vec<CompatibilityRule>,

    /// Schema metadata
    #[serde(default)]
    pub metadata: SchemaMetadata,
}

impl ConfigSchema {
    /// Create a new schema
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            name: name.into(),
            description: None,
            fields: HashMap::new(),
            environment_rules: Vec::new(),
            compatibility_rules: Vec::new(),
            metadata: SchemaMetadata::default(),
        }
    }

    /// Add a field definition
    pub fn with_field(mut self, key: impl Into<String>, rule: FieldRule) -> Self {
        self.fields.insert(key.into(), rule);
        self
    }

    /// Add an environment rule
    pub fn with_environment_rule(mut self, rule: EnvironmentRule) -> Self {
        self.environment_rules.push(rule);
        self
    }

    /// Add a compatibility rule
    pub fn with_compatibility_rule(mut self, rule: CompatibilityRule) -> Self {
        self.compatibility_rules.push(rule);
        self
    }

    /// Get a field rule by key
    pub fn get_field(&self, key: &str) -> Option<&FieldRule> {
        self.fields.get(key)
    }

    /// Check if schema has deprecated fields
    pub fn has_deprecated_fields(&self) -> bool {
        self.fields.values().any(|f| f.deprecation.is_some())
    }
}

/// Schema metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaMetadata {
    /// When the schema was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// When the schema was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Author of the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Link to documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

/// Definition for a single configuration field
///
/// Specifies the expected type, constraints, and metadata for a
/// configuration key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldRule {
    /// Expected field type
    pub field_type: FieldType,

    /// Whether this field is required
    #[serde(default)]
    pub required: bool,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Default value (if not required)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Validation constraints
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ValidationConstraint>,

    /// Allowed enum values (if applicable)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<serde_json::Value>,

    /// Deprecation information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecation: Option<DeprecationInfo>,

    /// Whether this field contains sensitive data
    #[serde(default)]
    pub sensitive: bool,

    /// Example values
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<serde_json::Value>,

    /// Custom validation rules (by ID)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_rules: Vec<String>,

    /// Nested field definitions (for object types)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub nested_fields: HashMap<String, FieldRule>,

    /// Item rule for array types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub array_item_rule: Option<Box<FieldRule>>,
}

impl FieldRule {
    /// Create a new field rule
    pub fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            required: false,
            description: None,
            default: None,
            constraints: Vec::new(),
            allowed_values: Vec::new(),
            deprecation: None,
            sensitive: false,
            examples: Vec::new(),
            custom_rules: Vec::new(),
            nested_fields: HashMap::new(),
            array_item_rule: None,
        }
    }

    /// Create a required field
    pub fn required(field_type: FieldType) -> Self {
        Self::new(field_type).set_required(true)
    }

    /// Set required flag
    pub fn set_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Add description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a constraint
    pub fn with_constraint(mut self, constraint: ValidationConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Set allowed values
    pub fn with_allowed_values(mut self, values: Vec<serde_json::Value>) -> Self {
        self.allowed_values = values;
        self
    }

    /// Mark as deprecated
    pub fn deprecated(mut self, info: DeprecationInfo) -> Self {
        self.deprecation = Some(info);
        self
    }

    /// Mark as sensitive
    pub fn sensitive(mut self) -> Self {
        self.sensitive = true;
        self
    }

    /// Add nested field for object types
    pub fn with_nested_field(mut self, key: impl Into<String>, rule: FieldRule) -> Self {
        self.nested_fields.insert(key.into(), rule);
        self
    }

    /// Set array item rule
    pub fn with_array_items(mut self, rule: FieldRule) -> Self {
        self.array_item_rule = Some(Box::new(rule));
        self
    }
}

/// Field type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// String value
    String,
    /// Integer value
    Integer,
    /// Floating-point value
    Float,
    /// Boolean value
    Boolean,
    /// Array of values
    Array,
    /// Nested object
    Object,
    /// Secret/encrypted value
    Secret,
    /// Any type (no type checking)
    Any,
    /// Duration (e.g., "30s", "5m")
    Duration,
    /// URL string
    Url,
    /// Email address
    Email,
    /// IP address
    IpAddress,
    /// File path
    FilePath,
    /// Regex pattern
    Regex,
    /// JSON string
    Json,
    /// Timestamp/datetime
    Timestamp,
}

impl FieldType {
    /// Get the type name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Integer => "integer",
            FieldType::Float => "float",
            FieldType::Boolean => "boolean",
            FieldType::Array => "array",
            FieldType::Object => "object",
            FieldType::Secret => "secret",
            FieldType::Any => "any",
            FieldType::Duration => "duration",
            FieldType::Url => "url",
            FieldType::Email => "email",
            FieldType::IpAddress => "ip_address",
            FieldType::FilePath => "file_path",
            FieldType::Regex => "regex",
            FieldType::Json => "json",
            FieldType::Timestamp => "timestamp",
        }
    }
}

/// Validation constraint for field values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationConstraint {
    /// Minimum value (for numbers)
    Min { value: f64, inclusive: bool },
    /// Maximum value (for numbers)
    Max { value: f64, inclusive: bool },
    /// Value range (for numbers)
    Range { min: f64, max: f64, inclusive: bool },

    /// Minimum length (for strings/arrays)
    MinLength { length: usize },
    /// Maximum length (for strings/arrays)
    MaxLength { length: usize },
    /// Exact length (for strings/arrays)
    Length { length: usize },

    /// Regex pattern match (for strings)
    Pattern { regex: String, description: Option<String> },
    /// Starts with prefix (for strings)
    StartsWith { prefix: String },
    /// Ends with suffix (for strings)
    EndsWith { suffix: String },
    /// Contains substring (for strings)
    Contains { substring: String },

    /// Not empty (for strings/arrays/objects)
    NotEmpty,
    /// Unique items (for arrays)
    UniqueItems,

    /// Custom validation expression
    Custom { expression: String, message: String },

    /// Conditional constraint (if condition met, apply nested constraint)
    Conditional {
        condition: String,
        then_constraint: Box<ValidationConstraint>,
        else_constraint: Option<Box<ValidationConstraint>>,
    },

    /// One of multiple possible types
    OneOf { types: Vec<FieldType> },

    /// Must reference an existing config key
    Reference { namespace: Option<String>, key_pattern: String },
}

impl ValidationConstraint {
    /// Create a min constraint
    pub fn min(value: f64) -> Self {
        Self::Min { value, inclusive: true }
    }

    /// Create a max constraint
    pub fn max(value: f64) -> Self {
        Self::Max { value, inclusive: true }
    }

    /// Create a range constraint
    pub fn range(min: f64, max: f64) -> Self {
        Self::Range { min, max, inclusive: true }
    }

    /// Create a min length constraint
    pub fn min_length(length: usize) -> Self {
        Self::MinLength { length }
    }

    /// Create a max length constraint
    pub fn max_length(length: usize) -> Self {
        Self::MaxLength { length }
    }

    /// Create a pattern constraint
    pub fn pattern(regex: impl Into<String>) -> Self {
        Self::Pattern { regex: regex.into(), description: None }
    }

    /// Get a human-readable description of this constraint
    pub fn description(&self) -> String {
        match self {
            Self::Min { value, inclusive } => {
                format!("must be {} {}", if *inclusive { ">=" } else { ">" }, value)
            }
            Self::Max { value, inclusive } => {
                format!("must be {} {}", if *inclusive { "<=" } else { "<" }, value)
            }
            Self::Range { min, max, inclusive } => {
                let op = if *inclusive { "..=" } else { ".." };
                format!("must be in range {}{}{}", min, op, max)
            }
            Self::MinLength { length } => format!("minimum length: {}", length),
            Self::MaxLength { length } => format!("maximum length: {}", length),
            Self::Length { length } => format!("exact length: {}", length),
            Self::Pattern { description, regex } => {
                description.clone().unwrap_or_else(|| format!("must match pattern: {}", regex))
            }
            Self::StartsWith { prefix } => format!("must start with: {}", prefix),
            Self::EndsWith { suffix } => format!("must end with: {}", suffix),
            Self::Contains { substring } => format!("must contain: {}", substring),
            Self::NotEmpty => "must not be empty".to_string(),
            Self::UniqueItems => "items must be unique".to_string(),
            Self::Custom { message, .. } => message.clone(),
            Self::Conditional { .. } => "conditional constraint".to_string(),
            Self::OneOf { types } => {
                format!("must be one of: {:?}", types.iter().map(|t| t.as_str()).collect::<Vec<_>>())
            }
            Self::Reference { key_pattern, .. } => format!("must reference: {}", key_pattern),
        }
    }
}

/// Information about deprecated fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    /// When the field was deprecated
    pub since_version: String,

    /// Reason for deprecation
    pub reason: String,

    /// Suggested replacement (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,

    /// When the field will be removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal_version: Option<String>,

    /// Link to migration guide
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_guide_url: Option<String>,
}

impl DeprecationInfo {
    /// Create new deprecation info
    pub fn new(since_version: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            since_version: since_version.into(),
            reason: reason.into(),
            replacement: None,
            removal_version: None,
            migration_guide_url: None,
        }
    }

    /// Set replacement field
    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }

    /// Set removal version
    pub fn will_be_removed_in(mut self, version: impl Into<String>) -> Self {
        self.removal_version = Some(version.into());
        self
    }
}

/// Environment-specific validation rules
///
/// Defines rules that apply only in certain environments or have
/// different constraints per environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentRule {
    /// Rule identifier
    pub id: String,

    /// Environments this rule applies to
    pub environments: Vec<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Fields affected by this rule
    pub affected_fields: Vec<String>,

    /// The rule type
    pub rule_type: EnvironmentRuleType,

    /// Whether this is a blocking rule
    #[serde(default = "default_true")]
    pub blocking: bool,
}

fn default_true() -> bool {
    true
}

/// Types of environment-specific rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EnvironmentRuleType {
    /// Field is required in these environments
    RequiredIn,

    /// Field is forbidden in these environments
    ForbiddenIn,

    /// Different constraints per environment
    ConstraintOverride {
        constraints: Vec<ValidationConstraint>,
    },

    /// Field must have different values per environment
    MustDiffer { from_environments: Vec<String> },

    /// Field must be encrypted in these environments
    MustEncrypt,

    /// Field must have a minimum rotation frequency
    RotationRequired { max_age_days: u32 },

    /// Custom environment-specific validation
    Custom { expression: String, message: String },
}

impl EnvironmentRule {
    /// Create a new required-in rule
    pub fn required_in(id: impl Into<String>, environments: Vec<String>, fields: Vec<String>) -> Self {
        Self {
            id: id.into(),
            environments,
            description: None,
            affected_fields: fields,
            rule_type: EnvironmentRuleType::RequiredIn,
            blocking: true,
        }
    }

    /// Create a new forbidden-in rule
    pub fn forbidden_in(id: impl Into<String>, environments: Vec<String>, fields: Vec<String>) -> Self {
        Self {
            id: id.into(),
            environments,
            description: None,
            affected_fields: fields,
            rule_type: EnvironmentRuleType::ForbiddenIn,
            blocking: true,
        }
    }

    /// Create a must-encrypt rule
    pub fn must_encrypt(id: impl Into<String>, environments: Vec<String>, fields: Vec<String>) -> Self {
        Self {
            id: id.into(),
            environments,
            description: None,
            affected_fields: fields,
            rule_type: EnvironmentRuleType::MustEncrypt,
            blocking: true,
        }
    }

    /// Add description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set as non-blocking (warning only)
    pub fn as_warning(mut self) -> Self {
        self.blocking = false;
        self
    }
}

/// Compatibility rules for cross-service/agent validation
///
/// Defines rules for ensuring configuration compatibility between
/// different services or agents in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityRule {
    /// Rule identifier
    pub id: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Service/agent this rule ensures compatibility with
    pub target_service: String,

    /// Target service version constraint (semver)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,

    /// The compatibility requirement
    pub requirement: CompatibilityRequirement,

    /// Whether this is a blocking rule
    #[serde(default = "default_true")]
    pub blocking: bool,

    /// Link to compatibility documentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

/// Types of compatibility requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompatibilityRequirement {
    /// Field must exist for compatibility
    RequiresField { field: String },

    /// Field must have specific format for target service
    RequiresFormat { field: String, format: String },

    /// Field value must be in allowed set for target service
    AllowedValues { field: String, values: Vec<serde_json::Value> },

    /// Field must match target service's schema
    SchemaMatch { schema_ref: String },

    /// Custom compatibility check
    Custom { expression: String, message: String },

    /// Version range compatibility
    VersionRange { field: String, min_version: Option<String>, max_version: Option<String> },

    /// Protocol compatibility
    ProtocolVersion { field: String, protocol: String, min_version: String },
}

impl CompatibilityRule {
    /// Create a new requires-field rule
    pub fn requires_field(
        id: impl Into<String>,
        target_service: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: None,
            target_service: target_service.into(),
            target_version: None,
            requirement: CompatibilityRequirement::RequiresField { field: field.into() },
            blocking: true,
            documentation_url: None,
        }
    }

    /// Create a new requires-format rule
    pub fn requires_format(
        id: impl Into<String>,
        target_service: impl Into<String>,
        field: impl Into<String>,
        format: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            description: None,
            target_service: target_service.into(),
            target_version: None,
            requirement: CompatibilityRequirement::RequiresFormat {
                field: field.into(),
                format: format.into(),
            },
            blocking: true,
            documentation_url: None,
        }
    }

    /// Add description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set target version constraint
    pub fn for_version(mut self, version: impl Into<String>) -> Self {
        self.target_version = Some(version.into());
        self
    }

    /// Set as non-blocking (warning only)
    pub fn as_warning(mut self) -> Self {
        self.blocking = false;
        self
    }
}

/// Complete schema definition document
///
/// A versioned document containing a schema definition with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    /// Schema document format version
    pub schema_format_version: String,

    /// The schema definition
    pub schema: ConfigSchema,

    /// Document checksum for integrity verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,

    /// Signature for authenticity (base64)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl SchemaDefinition {
    /// Create a new schema definition
    pub fn new(schema: ConfigSchema) -> Self {
        Self {
            schema_format_version: "1.0".to_string(),
            schema,
            checksum: None,
            signature: None,
        }
    }

    /// Current schema format version
    pub const CURRENT_FORMAT_VERSION: &'static str = "1.0";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_creation() {
        let schema = ConfigSchema::new("app/database", "Database Config", "1.0.0")
            .with_field("host", FieldRule::required(FieldType::String))
            .with_field("port", FieldRule::new(FieldType::Integer)
                .with_constraint(ValidationConstraint::range(1.0, 65535.0)));

        assert_eq!(schema.fields.len(), 2);
        assert!(schema.fields.get("host").unwrap().required);
    }

    #[test]
    fn test_field_rule_builder() {
        let rule = FieldRule::required(FieldType::String)
            .with_description("Database connection string")
            .with_constraint(ValidationConstraint::min_length(10))
            .with_constraint(ValidationConstraint::pattern(r"^postgres://"))
            .sensitive();

        assert!(rule.required);
        assert!(rule.sensitive);
        assert_eq!(rule.constraints.len(), 2);
    }

    #[test]
    fn test_environment_rule_creation() {
        let rule = EnvironmentRule::must_encrypt(
            "prod-secrets",
            vec!["production".to_string()],
            vec!["api_key".to_string(), "db_password".to_string()],
        ).with_description("Secrets must be encrypted in production");

        assert!(rule.blocking);
        assert_eq!(rule.affected_fields.len(), 2);
    }

    #[test]
    fn test_compatibility_rule_creation() {
        let rule = CompatibilityRule::requires_field(
            "metrics-service-compat",
            "metrics-service",
            "telemetry.enabled",
        )
        .for_version(">=2.0.0")
        .with_description("Metrics service v2+ requires telemetry.enabled field");

        assert_eq!(rule.target_service, "metrics-service");
        assert_eq!(rule.target_version, Some(">=2.0.0".to_string()));
    }

    #[test]
    fn test_deprecation_info() {
        let deprecation = DeprecationInfo::new("2.0.0", "Use connection_url instead")
            .with_replacement("connection_url")
            .will_be_removed_in("3.0.0");

        assert_eq!(deprecation.since_version, "2.0.0");
        assert_eq!(deprecation.replacement, Some("connection_url".to_string()));
    }

    #[test]
    fn test_constraint_description() {
        let constraint = ValidationConstraint::range(1.0, 100.0);
        assert!(constraint.description().contains("1"));
        assert!(constraint.description().contains("100"));

        let pattern = ValidationConstraint::pattern(r"^\d+$");
        assert!(pattern.description().contains("pattern"));
    }
}
