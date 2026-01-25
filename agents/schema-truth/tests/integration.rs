//! Integration tests for Schema Truth Agent

use schema_truth::contracts::*;
use schema_truth::engine::SchemaValidationEngine;
use std::collections::HashMap;

fn create_valid_schema() -> serde_json::Value {
    serde_json::json!({
        "id": "test/config",
        "version": "1.0.0",
        "name": "Test Configuration",
        "description": "A test schema",
        "fields": {
            "database_url": {
                "field_type": "url",
                "required": true,
                "description": "Database connection URL"
            },
            "max_connections": {
                "field_type": "integer",
                "required": false,
                "default": 10,
                "constraints": [
                    {"type": "min", "value": 1, "inclusive": true},
                    {"type": "max", "value": 100, "inclusive": true}
                ]
            },
            "api_key": {
                "field_type": "secret",
                "required": true,
                "secret": true,
                "description": "API authentication key"
            }
        },
        "metadata": {
            "owner": "platform-team",
            "tags": ["database", "config"]
        }
    })
}

#[tokio::test]
async fn test_validate_valid_schema() {
    let engine = SchemaValidationEngine::new();
    let input = engine
        .create_input(create_valid_schema(), "test".to_string())
        .expect("Failed to create input");

    let output = engine.validate(&input).await;

    assert!(output.is_valid);
    assert!(output.violations.is_empty());
    assert!(!output.rules_applied.is_empty());
    assert!(output.coverage > 0.0);
}

#[tokio::test]
async fn test_validate_missing_id() {
    let engine = SchemaValidationEngine::new();
    let schema = serde_json::json!({
        "id": "",
        "version": "1.0.0",
        "name": "Test",
        "fields": {}
    });

    let input = engine
        .create_input(schema, "test".to_string())
        .expect("Failed to create input");

    let output = engine.validate(&input).await;

    assert!(!output.is_valid);
    assert!(output.violations.iter().any(|v| v.code == "SCHEMA_ID_REQUIRED"));
}

#[tokio::test]
async fn test_validate_invalid_version() {
    let engine = SchemaValidationEngine::new();
    let schema = serde_json::json!({
        "id": "test/config",
        "version": "invalid",
        "name": "Test",
        "fields": {}
    });

    let input = engine
        .create_input(schema, "test".to_string())
        .expect("Failed to create input");

    let output = engine.validate(&input).await;

    // Should have warning for non-semver version
    assert!(output.warnings.iter().any(|w| w.code == "VERSION_NOT_SEMVER"));
}

#[tokio::test]
async fn test_validate_invalid_range() {
    let engine = SchemaValidationEngine::new();
    let schema = serde_json::json!({
        "id": "test/config",
        "version": "1.0.0",
        "name": "Test",
        "fields": {
            "count": {
                "field_type": "integer",
                "constraints": [
                    {"type": "range", "min": 100, "max": 10, "inclusive": true}
                ]
            }
        }
    });

    let input = engine
        .create_input(schema, "test".to_string())
        .expect("Failed to create input");

    let output = engine.validate(&input).await;

    assert!(!output.is_valid);
    assert!(output.violations.iter().any(|v| v.code == "INVALID_RANGE"));
}

#[tokio::test]
async fn test_deterministic_hash() {
    let engine = SchemaValidationEngine::new();
    let input1 = engine
        .create_input(create_valid_schema(), "test".to_string())
        .expect("Failed to create input");
    let input2 = engine
        .create_input(create_valid_schema(), "test".to_string())
        .expect("Failed to create input");

    let hash1 = SchemaValidationEngine::compute_inputs_hash(&input1);
    let hash2 = SchemaValidationEngine::compute_inputs_hash(&input2);

    // Same schema should produce same hash
    assert_eq!(hash1, hash2);
}

#[tokio::test]
async fn test_decision_event_creation() {
    let engine = SchemaValidationEngine::new();
    let input = engine
        .create_input(create_valid_schema(), "test".to_string())
        .expect("Failed to create input");

    let inputs_hash = SchemaValidationEngine::compute_inputs_hash(&input);
    let output = engine.validate(&input).await;

    let signal = SchemaViolationSignal::from_validation(
        inputs_hash,
        &output,
        input.request_id.to_string(),
    );

    assert_eq!(signal.agent_id, SchemaViolationSignal::AGENT_ID);
    assert_eq!(signal.signal_type, "schema_violation_signal");
    assert!(signal.confidence > 0.0);
    assert!(signal.confidence <= 1.0);
}
