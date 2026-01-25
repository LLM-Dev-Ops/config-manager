//! Schema validation rules
//!
//! Deterministic rules for validating schema structure and content.

use crate::contracts::*;
use crate::engine::SchemaRule;
use regex::Regex;

/// Validates schema structure
pub struct StructureRule;

impl SchemaRule for StructureRule {
    fn id(&self) -> &str {
        "structure"
    }

    fn name(&self) -> &str {
        "Schema Structure Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        // Schema must have an ID
        if schema.id.is_empty() {
            violations.push(
                SchemaViolation::error("SCHEMA_ID_REQUIRED", "Schema must have a non-empty ID"),
            );
        }

        // Schema must have a name
        if schema.name.is_empty() {
            violations.push(
                SchemaViolation::error("SCHEMA_NAME_REQUIRED", "Schema must have a non-empty name"),
            );
        }

        // Schema must have at least one field
        if schema.fields.is_empty() {
            violations.push(
                SchemaViolation::warning(
                    "SCHEMA_NO_FIELDS",
                    "Schema has no field definitions",
                ),
            );
        }

        violations
    }
}

/// Validates field types
pub struct FieldTypeRule;

impl SchemaRule for FieldTypeRule {
    fn id(&self) -> &str {
        "field_type"
    }

    fn name(&self) -> &str {
        "Field Type Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        for (field_name, field_def) in &schema.fields {
            // Object types should have nested schema
            if field_def.field_type == FieldType::Object && field_def.nested_schema.is_none() {
                violations.push(
                    SchemaViolation::warning(
                        "OBJECT_NO_SCHEMA",
                        format!("Object field '{}' has no nested schema", field_name),
                    )
                    .with_path(field_name.clone()),
                );
            }

            // Secret fields should be marked as secret
            if field_def.field_type == FieldType::Secret && !field_def.secret {
                violations.push(
                    SchemaViolation::error(
                        "SECRET_NOT_MARKED",
                        format!("Field '{}' has Secret type but secret=false", field_name),
                    )
                    .with_path(field_name.clone()),
                );
            }

            // Array types should have constraints
            if field_def.field_type == FieldType::Array && field_def.constraints.is_empty() {
                violations.push(
                    SchemaViolation::warning(
                        "ARRAY_NO_CONSTRAINTS",
                        format!("Array field '{}' has no constraints", field_name),
                    )
                    .with_path(field_name.clone()),
                );
            }
        }

        violations
    }
}

/// Validates field constraints
pub struct ConstraintRule;

impl SchemaRule for ConstraintRule {
    fn id(&self) -> &str {
        "constraint"
    }

    fn name(&self) -> &str {
        "Constraint Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        for (field_name, field_def) in &schema.fields {
            for constraint in &field_def.constraints {
                match constraint {
                    FieldConstraint::Pattern { regex, .. } => {
                        // Validate regex is compilable
                        if Regex::new(regex).is_err() {
                            violations.push(
                                SchemaViolation::error(
                                    "INVALID_REGEX",
                                    format!("Field '{}' has invalid regex pattern", field_name),
                                )
                                .with_path(field_name.clone()),
                            );
                        }
                    }
                    FieldConstraint::Range { min, max, .. } => {
                        if min > max {
                            violations.push(
                                SchemaViolation::error(
                                    "INVALID_RANGE",
                                    format!("Field '{}' has min > max in range", field_name),
                                )
                                .with_path(field_name.clone())
                                .with_expected_actual(
                                    format!("min <= max"),
                                    format!("min={}, max={}", min, max),
                                ),
                            );
                        }
                    }
                    FieldConstraint::MinLength { length } | FieldConstraint::MaxLength { length } => {
                        // Length constraints only valid for string/array
                        match field_def.field_type {
                            FieldType::String | FieldType::Array => {}
                            _ => {
                                violations.push(
                                    SchemaViolation::error(
                                        "INVALID_CONSTRAINT_TYPE",
                                        format!(
                                            "Field '{}' has length constraint but is not string/array",
                                            field_name
                                        ),
                                    )
                                    .with_path(field_name.clone()),
                                );
                            }
                        }
                        let _ = length; // suppress unused warning
                    }
                    FieldConstraint::Enum { values } => {
                        if values.is_empty() {
                            violations.push(
                                SchemaViolation::error(
                                    "EMPTY_ENUM",
                                    format!("Field '{}' has empty enum values", field_name),
                                )
                                .with_path(field_name.clone()),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        violations
    }
}

/// Validates required fields
pub struct RequiredFieldRule;

impl SchemaRule for RequiredFieldRule {
    fn id(&self) -> &str {
        "required_field"
    }

    fn name(&self) -> &str {
        "Required Field Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        for (field_name, field_def) in &schema.fields {
            // Required fields should not have defaults (they're always provided)
            if field_def.required && field_def.default.is_some() {
                violations.push(
                    SchemaViolation::warning(
                        "REQUIRED_WITH_DEFAULT",
                        format!("Required field '{}' has a default value (redundant)", field_name),
                    )
                    .with_path(field_name.clone()),
                );
            }
        }

        // Check environment rules for required fields
        for rule in &schema.environment_rules {
            if let EnvironmentRule::RequiredIn { field, .. } = rule {
                if !schema.fields.contains_key(field) {
                    violations.push(
                        SchemaViolation::error(
                            "REQUIRED_FIELD_MISSING",
                            format!("Environment rule references undefined field '{}'", field),
                        )
                        .with_path(format!("environment_rules.{}", field)),
                    );
                }
            }
        }

        violations
    }
}

/// Validates deprecation info
pub struct DeprecationRule;

impl SchemaRule for DeprecationRule {
    fn id(&self) -> &str {
        "deprecation"
    }

    fn name(&self) -> &str {
        "Deprecation Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        for (field_name, field_def) in &schema.fields {
            if let Some(deprecation) = &field_def.deprecated {
                // Deprecated fields should have a reason
                if deprecation.reason.is_empty() {
                    violations.push(
                        SchemaViolation::warning(
                            "DEPRECATION_NO_REASON",
                            format!("Deprecated field '{}' has no reason", field_name),
                        )
                        .with_path(field_name.clone()),
                    );
                }

                // Should have replacement suggestion
                if deprecation.replacement.is_none() && deprecation.migration_guide.is_none() {
                    violations.push(
                        SchemaViolation::warning(
                            "DEPRECATION_NO_MIGRATION",
                            format!(
                                "Deprecated field '{}' has no replacement or migration guide",
                                field_name
                            ),
                        )
                        .with_path(field_name.clone()),
                    );
                }
            }
        }

        violations
    }
}

/// Validates naming conventions
pub struct NamingConventionRule;

impl SchemaRule for NamingConventionRule {
    fn id(&self) -> &str {
        "naming_convention"
    }

    fn name(&self) -> &str {
        "Naming Convention Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();
        let snake_case = Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();

        for field_name in schema.fields.keys() {
            if !snake_case.is_match(field_name) {
                violations.push(
                    SchemaViolation::warning(
                        "NAMING_NOT_SNAKE_CASE",
                        format!("Field '{}' should use snake_case", field_name),
                    )
                    .with_path(field_name.clone()),
                );
            }
        }

        // Schema ID should follow namespace pattern
        if !schema.id.contains('/') && !schema.id.contains('.') {
            violations.push(
                SchemaViolation::warning(
                    "SCHEMA_ID_NO_NAMESPACE",
                    "Schema ID should include namespace (e.g., 'app/config')",
                ),
            );
        }

        violations
    }
}

/// Validates version format
pub struct VersionRule;

impl SchemaRule for VersionRule {
    fn id(&self) -> &str {
        "version"
    }

    fn name(&self) -> &str {
        "Version Validation"
    }

    fn applies_to(&self, _schema: &SchemaDefinition) -> bool {
        true
    }

    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        _parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();
        let semver = Regex::new(r"^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?(\+[a-zA-Z0-9.]+)?$").unwrap();

        if schema.version.is_empty() {
            violations.push(
                SchemaViolation::error("VERSION_REQUIRED", "Schema must have a version"),
            );
        } else if !semver.is_match(&schema.version) {
            violations.push(
                SchemaViolation::warning(
                    "VERSION_NOT_SEMVER",
                    format!("Schema version '{}' does not follow semver", schema.version),
                )
                .with_expected_actual("X.Y.Z[-prerelease][+build]", &schema.version),
            );
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_schema() -> SchemaDefinition {
        SchemaDefinition {
            id: "test/config".to_string(),
            version: "1.0.0".to_string(),
            name: "Test Config".to_string(),
            description: None,
            fields: HashMap::new(),
            metadata: SchemaMetadata::default(),
            environment_rules: Vec::new(),
            compatibility: Vec::new(),
        }
    }

    #[test]
    fn test_structure_rule_empty_id() {
        let rule = StructureRule;
        let mut schema = create_test_schema();
        schema.id = String::new();

        let violations = rule.evaluate(&schema, None);
        assert!(violations.iter().any(|v| v.code == "SCHEMA_ID_REQUIRED"));
    }

    #[test]
    fn test_version_rule_invalid_semver() {
        let rule = VersionRule;
        let mut schema = create_test_schema();
        schema.version = "invalid".to_string();

        let violations = rule.evaluate(&schema, None);
        assert!(violations.iter().any(|v| v.code == "VERSION_NOT_SEMVER"));
    }

    #[test]
    fn test_constraint_rule_invalid_range() {
        let rule = ConstraintRule;
        let mut schema = create_test_schema();
        schema.fields.insert(
            "count".to_string(),
            FieldDefinition {
                field_type: FieldType::Integer,
                required: false,
                default: None,
                description: None,
                constraints: vec![FieldConstraint::Range {
                    min: 100.0,
                    max: 10.0,
                    inclusive: true,
                }],
                deprecated: None,
                secret: false,
                nested_schema: None,
            },
        );

        let violations = rule.evaluate(&schema, None);
        assert!(violations.iter().any(|v| v.code == "INVALID_RANGE"));
    }
}
