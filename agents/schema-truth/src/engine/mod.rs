//! Schema validation engine
//!
//! Deterministic validation of schema definitions.

mod rules;

pub use rules::*;

use crate::contracts::*;
use sha2::{Digest, Sha256};
use std::time::Instant;
use uuid::Uuid;

/// Performance budget constants
pub const MAX_LATENCY_MS: u64 = 1500;
pub const MAX_TOKENS: usize = 800;

/// Schema validation engine
pub struct SchemaValidationEngine {
    rules: Vec<Box<dyn SchemaRule>>,
}

impl Default for SchemaValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaValidationEngine {
    /// Create new engine with default rules
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(StructureRule),
                Box::new(FieldTypeRule),
                Box::new(ConstraintRule),
                Box::new(RequiredFieldRule),
                Box::new(DeprecationRule),
                Box::new(NamingConventionRule),
                Box::new(VersionRule),
            ],
        }
    }

    /// Validate a schema definition
    pub async fn validate(&self, input: &SchemaValidationInput) -> SchemaValidationOutput {
        let start = Instant::now();
        let request_id = input.request_id;

        let mut violations = Vec::new();
        let mut warnings = Vec::new();
        let mut rules_applied = Vec::new();
        let mut constraints_checked = Vec::new();

        // Apply each rule
        for rule in &self.rules {
            if !rule.applies_to(&input.schema) {
                continue;
            }

            rules_applied.push(rule.id().to_string());

            let findings = rule.evaluate(&input.schema, input.parent_schema.as_ref());

            for finding in findings {
                constraints_checked.push(format!("{}:{}", rule.id(), finding.code));

                match finding.severity {
                    ViolationSeverity::Error | ViolationSeverity::Critical => {
                        violations.push(finding);
                    }
                    ViolationSeverity::Warning | ViolationSeverity::Info => {
                        warnings.push(finding);
                    }
                }
            }

            // Check latency budget
            if start.elapsed().as_millis() as u64 > MAX_LATENCY_MS {
                tracing::warn!("Validation exceeded latency budget, stopping early");
                break;
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let coverage = if self.rules.is_empty() {
            0.0
        } else {
            rules_applied.len() as f64 / self.rules.len() as f64
        };

        SchemaValidationOutput {
            request_id,
            is_valid: violations.is_empty(),
            violations,
            warnings,
            rules_applied,
            constraints_checked,
            coverage,
            completed_at: chrono::Utc::now(),
            duration_ms,
        }
    }

    /// Compute deterministic hash of inputs
    pub fn compute_inputs_hash(input: &SchemaValidationInput) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.schema.id.as_bytes());
        hasher.update(input.schema.version.as_bytes());
        if let Ok(json) = serde_json::to_string(&input.schema.fields) {
            hasher.update(json.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Create validation input from schema JSON
    pub fn create_input(
        schema_json: serde_json::Value,
        requested_by: String,
    ) -> Result<SchemaValidationInput, String> {
        let schema: SchemaDefinition = serde_json::from_value(schema_json)
            .map_err(|e| format!("Invalid schema JSON: {}", e))?;

        Ok(SchemaValidationInput {
            request_id: Uuid::new_v4(),
            schema,
            parent_schema: None,
            context: std::collections::HashMap::new(),
            requested_at: chrono::Utc::now(),
            requested_by,
        })
    }
}

/// Trait for schema validation rules
pub trait SchemaRule: Send + Sync {
    /// Rule identifier
    fn id(&self) -> &str;

    /// Rule name
    fn name(&self) -> &str;

    /// Check if rule applies to this schema
    fn applies_to(&self, schema: &SchemaDefinition) -> bool;

    /// Evaluate the schema and return violations
    fn evaluate(
        &self,
        schema: &SchemaDefinition,
        parent: Option<&SchemaDefinition>,
    ) -> Vec<SchemaViolation>;
}
