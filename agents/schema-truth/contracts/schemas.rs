//! Schema definition types for schema truth validation
//!
//! Defines the canonical schema structure that all configurations must follow.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Schema definition - the source of truth for configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    /// Schema identifier (namespace path)
    pub id: String,

    /// Schema version (semver)
    pub version: String,

    /// Human-readable name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Field definitions
    pub fields: HashMap<String, FieldDefinition>,

    /// Schema metadata
    #[serde(default)]
    pub metadata: SchemaMetadata,

    /// Environment-specific rules
    #[serde(default)]
    pub environment_rules: Vec<EnvironmentRule>,

    /// Compatibility constraints
    #[serde(default)]
    pub compatibility: Vec<CompatibilityConstraint>,
}

/// Field definition within a schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Field type
    pub field_type: FieldType,

    /// Whether field is required
    #[serde(default)]
    pub required: bool,

    /// Default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Validation constraints
    #[serde(default)]
    pub constraints: Vec<FieldConstraint>,

    /// Deprecation info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<DeprecationInfo>,

    /// Whether this is a secret field
    #[serde(default)]
    pub secret: bool,

    /// Nested schema for object types
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_schema: Option<Box<SchemaDefinition>>,
}

/// Supported field types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
    Secret,
    Duration,
    Url,
    Email,
    IpAddress,
    FilePath,
    Regex,
    Json,
    Timestamp,
    Any,
}

/// Field validation constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldConstraint {
    /// Minimum value (numeric)
    Min { value: f64, inclusive: bool },

    /// Maximum value (numeric)
    Max { value: f64, inclusive: bool },

    /// Value range
    Range { min: f64, max: f64, inclusive: bool },

    /// Minimum length (string/array)
    MinLength { length: usize },

    /// Maximum length (string/array)
    MaxLength { length: usize },

    /// Exact length
    Length { length: usize },

    /// Regex pattern
    Pattern { regex: String, description: Option<String> },

    /// String prefix
    StartsWith { prefix: String },

    /// String suffix
    EndsWith { suffix: String },

    /// String contains
    Contains { substring: String },

    /// Non-empty
    NotEmpty,

    /// Unique items (array)
    UniqueItems,

    /// Allowed values
    Enum { values: Vec<serde_json::Value> },

    /// Custom validation expression
    Custom { expression: String, message: String },
}

/// Schema metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaMetadata {
    /// Schema owner/maintainer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Last modified timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    /// Additional properties
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Environment-specific rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "rule_type", rename_all = "snake_case")]
pub enum EnvironmentRule {
    /// Field required in specific environments
    RequiredIn {
        field: String,
        environments: Vec<String>,
    },

    /// Field forbidden in specific environments
    ForbiddenIn {
        field: String,
        environments: Vec<String>,
    },

    /// Different constraints per environment
    ConstraintOverride {
        field: String,
        environment: String,
        constraints: Vec<FieldConstraint>,
    },

    /// Value must differ between environments
    MustDiffer {
        field: String,
        environments: Vec<String>,
    },

    /// Must be encrypted in specific environments
    MustEncrypt {
        field: String,
        environments: Vec<String>,
    },
}

/// Cross-service/schema compatibility constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "constraint_type", rename_all = "snake_case")]
pub enum CompatibilityConstraint {
    /// Requires another schema
    RequiresSchema { schema_id: String, min_version: Option<String> },

    /// Field must match format in target
    FieldFormat { field: String, target_schema: String, target_field: String },

    /// Protocol version compatibility
    ProtocolVersion { min: String, max: Option<String> },

    /// Custom compatibility check
    Custom { expression: String, message: String },
}

/// Deprecation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    /// Version when deprecated
    pub since_version: String,

    /// Reason for deprecation
    pub reason: String,

    /// Replacement field/approach
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,

    /// Version when will be removed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal_version: Option<String>,

    /// Migration guide URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_guide: Option<String>,
}
