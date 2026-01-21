//! Integration tests for Config Validation Agent telemetry
//!
//! Tests the telemetry module including:
//! - DecisionEvent creation and emission
//! - Prometheus metrics collection
//! - Input hash calculation
//! - Confidence score computation

use config_validation::contracts::{
    ConfigValueRef, DecisionEvent, DecisionType, EnvironmentRef, IssueSeverity,
    ValidationInput, ValidationIssue, ValidationOutput, ValidationOutputs,
};
use config_validation::telemetry::{
    emitter::{build_constraints_list, calculate_inputs_hash, hash_validation_components},
    metrics::ValidationMetricsRegistry,
    TelemetryConfig,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Helper to create a test validation input
fn create_test_input() -> ValidationInput {
    ValidationInput::new(
        "app/database",
        "connection_string",
        ConfigValueRef::String("postgres://localhost/db".to_string()),
        EnvironmentRef::Development,
        "test-user",
    )
}

/// Helper to create a test validation output
fn create_test_output(valid: bool, rules: Vec<String>) -> ValidationOutput {
    if valid {
        ValidationOutput::success(Uuid::new_v4(), rules)
            .with_coverage(0.95)
            .with_duration(42)
    } else {
        ValidationOutput::failure(
            Uuid::new_v4(),
            vec![ValidationIssue::error("E001", "Test error")],
        )
        .with_coverage(0.8)
        .with_duration(100)
    }
}

#[test]
fn test_full_decision_event_flow() {
    // Create validation output
    let output = create_test_output(
        true,
        vec![
            "required-fields".to_string(),
            "type-check".to_string(),
            "range-validation".to_string(),
        ],
    )
    .with_warning(ValidationIssue::warning("WARN001", "Missing optional field"));

    // Calculate inputs hash
    let config_content = r#"{"name": "test", "value": 42}"#;
    let inputs_hash = hash_validation_components(
        config_content,
        "2.0.0",
        &["required-fields".to_string(), "type-check".to_string()],
    );

    // Create decision event
    let event = DecisionEvent::from_validation(
        inputs_hash.clone(),
        &output,
        "exec-ref-integration-test".to_string(),
    )
    .with_metadata("environment", serde_json::json!("test"))
    .with_correlation_id("trace_id", "trace-123");

    // Verify event structure
    assert_eq!(event.agent_id, DecisionEvent::AGENT_ID);
    assert_eq!(event.agent_version, DecisionEvent::AGENT_VERSION);
    assert_eq!(event.inputs_hash, inputs_hash);
    assert!(event.outputs.is_valid);
    assert_eq!(event.outputs.warning_count, 1);

    // Verify confidence calculation
    assert!(event.confidence >= 0.0 && event.confidence <= 1.0);

    // Verify serialization roundtrip
    let json = serde_json::to_string_pretty(&event).unwrap();
    let deserialized: DecisionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event.event_id, deserialized.event_id);
    assert_eq!(event.agent_id, deserialized.agent_id);
}

#[test]
fn test_metrics_integration() {
    let registry = ValidationMetricsRegistry::new().unwrap();

    // Simulate a validation workflow
    let metrics = registry.validation();

    // Record validation request
    metrics.record_request("production", "app-config", true);

    // Record duration
    metrics.observe_duration("production", "2.0.0", 0.045);

    // Record findings
    metrics.record_finding("info", "INFO001", "production");
    metrics.record_finding("warning", "WARN001", "production");

    // Record findings by severity enum
    metrics.record_finding_severity(IssueSeverity::Error, "ERR001", "staging");

    // Set confidence
    metrics.set_confidence("production", "app-config", 0.86);

    // Record schema version usage
    metrics.record_schema_version("2.0.0");

    // Record rule evaluations
    metrics.record_rule_evaluation("required-fields", true);
    metrics.record_rule_evaluation("type-check", true);
    metrics.record_rule_evaluation("range-validation", true);

    // Record event emission metrics
    metrics.record_event_emitted();
    metrics.record_event_emitted();
    metrics.record_event_failed();
    metrics.set_queue_depth(5);

    // Verify metrics can be gathered
    let families = registry.gather();
    assert!(!families.is_empty());

    // Verify text encoding
    let text = registry.encode_text().unwrap();
    assert!(text.contains("config_validation_validation_requests_total"));
    assert!(text.contains("config_validation_validation_duration_seconds"));
    assert!(text.contains("config_validation_validation_findings_total"));
    assert!(text.contains("config_validation_validation_confidence"));
    assert!(text.contains("config_validation_events_emitted_total"));
}

#[test]
fn test_validation_timer_integration() {
    let registry = ValidationMetricsRegistry::new().unwrap();
    let metrics = registry.validation();

    // Use timer for automatic duration recording
    {
        let timer = metrics.start_timer("staging", "1.5.0");
        // Simulate validation work
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(timer.elapsed_secs() >= 0.05);
    }

    // Timer should have recorded duration and decremented active count
    let text = registry.encode_text().unwrap();
    assert!(text.contains("config_validation_validation_duration_seconds"));
}

#[test]
fn test_inputs_hash_determinism() {
    let config = r#"{"key": "value"}"#;
    let schema = "1.0.0";
    let rules = vec!["rule1".to_string(), "rule2".to_string()];

    let hash1 = hash_validation_components(config, schema, &rules);
    let hash2 = hash_validation_components(config, schema, &rules);

    // Same inputs should produce same hash
    assert_eq!(hash1, hash2);

    // Different inputs should produce different hash
    let hash3 = hash_validation_components(config, "2.0.0", &rules);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_calculate_inputs_hash_from_validation_input() {
    let input1 = create_test_input();
    let input2 = create_test_input();

    let hash1 = calculate_inputs_hash(&input1);
    let hash2 = calculate_inputs_hash(&input2);

    // Hash should be consistent length (SHA-256 = 64 hex chars)
    assert_eq!(hash1.len(), 64);
    assert_eq!(hash2.len(), 64);

    // Different namespace should produce different hash
    let mut input3 = create_test_input();
    input3.namespace = "different/namespace".to_string();
    let hash3 = calculate_inputs_hash(&input3);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_confidence_calculation() {
    // Perfect validation
    let perfect = ValidationOutput::success(Uuid::new_v4(), vec![
        "rule1".to_string(),
        "rule2".to_string(),
        "rule3".to_string(),
    ])
    .with_coverage(1.0);
    assert!(perfect.confidence() >= 0.9);

    // Low coverage validation
    let low_coverage = ValidationOutput::success(Uuid::new_v4(), vec!["rule1".to_string()])
        .with_coverage(0.5);
    // Low rules count reduces confidence further
    assert!(low_coverage.confidence() < 0.5);

    // Validation with warnings
    let with_warning = ValidationOutput::success(
        Uuid::new_v4(),
        vec!["rule1".to_string(), "rule2".to_string(), "rule3".to_string()],
    )
    .with_coverage(1.0)
    .with_warning(ValidationIssue::warning("WARN001", "Test warning"));
    // Warning reduces confidence
    assert!(with_warning.confidence() < perfect.confidence());
}

#[test]
fn test_build_constraints_list() {
    let constraints = build_constraints_list(
        Some("2.0.0"),
        &["type_check".to_string(), "bounds_check".to_string()],
        &["required_field".to_string()],
    );

    assert!(constraints.contains(&"schema:2.0.0".to_string()));
    assert!(constraints.contains(&"rule:type_check".to_string()));
    assert!(constraints.contains(&"rule:bounds_check".to_string()));
    assert!(constraints.contains(&"required_field".to_string()));

    // Without schema
    let constraints_no_schema = build_constraints_list(
        None,
        &["type_check".to_string()],
        &[],
    );
    assert!(!constraints_no_schema.iter().any(|c| c.starts_with("schema:")));
}

#[test]
fn test_decision_event_types() {
    let output = ValidationOutputs::success(vec!["rule1".to_string()], 0.95);

    // Config validation result
    let config_event = DecisionEvent::new(
        DecisionType::ConfigValidationResult,
        "hash1".to_string(),
        output.clone(),
        0.9,
        "ref1".to_string(),
    );
    assert_eq!(config_event.decision_type.as_str(), "config_validation_result");

    // Schema validation result
    let schema_event = DecisionEvent::new(
        DecisionType::SchemaValidationResult,
        "hash2".to_string(),
        output.clone(),
        0.85,
        "ref2".to_string(),
    );
    assert_eq!(schema_event.decision_type.as_str(), "schema_validation_result");

    // Compatibility check result
    let compat_event = DecisionEvent::new(
        DecisionType::CompatibilityCheckResult,
        "hash3".to_string(),
        output,
        0.8,
        "ref3".to_string(),
    );
    assert_eq!(compat_event.decision_type.as_str(), "compatibility_check_result");
}

#[test]
fn test_telemetry_config() {
    let config = TelemetryConfig::default();
    assert!(config.emit_decisions);
    assert!(config.enable_metrics);
    assert_eq!(config.max_queue_size, 1000);
    assert_eq!(config.timeout_ms, 5000);
    assert!(!config.enable_batching);

    // Test builder
    let custom_config = TelemetryConfig::builder()
        .endpoint("http://custom:9090")
        .emit_decisions(true)
        .enable_metrics(true)
        .max_queue_size(500)
        .timeout_ms(10000)
        .with_batching(50, 2000)
        .build();

    assert_eq!(custom_config.ruvector_endpoint, "http://custom:9090");
    assert_eq!(custom_config.max_queue_size, 500);
    assert!(custom_config.enable_batching);
    assert_eq!(custom_config.batch_size, 50);
}

#[test]
fn test_validation_outputs_analytics() {
    let output = ValidationOutput::failure(
        Uuid::new_v4(),
        vec![
            ValidationIssue::error("E001", "Error 1").with_path("/config/field1"),
            ValidationIssue::error("E002", "Error 2").with_path("/config/field2"),
        ],
    )
    .with_warning(ValidationIssue::warning("W001", "Warning 1"));

    let outputs = ValidationOutputs::from_output(&output);

    assert!(!outputs.is_valid);
    assert_eq!(outputs.error_count, 2);
    assert_eq!(outputs.warning_count, 1);
    assert!(outputs.error_codes.contains(&"E001".to_string()));
    assert!(outputs.error_codes.contains(&"E002".to_string()));
}

#[test]
fn test_event_metadata_and_correlation() {
    let output = ValidationOutputs::success(vec!["rule1".to_string()], 0.95);

    let event = DecisionEvent::new(
        DecisionType::ConfigValidationResult,
        "hash".to_string(),
        output,
        0.9,
        "exec-ref".to_string(),
    )
    .with_metadata("environment", serde_json::json!("production"))
    .with_metadata("namespace", serde_json::json!("app/config"))
    .with_correlation_id("trace_id", "trace-abc-123")
    .with_correlation_id("span_id", "span-xyz-456");

    assert_eq!(
        event.metadata.get("environment"),
        Some(&serde_json::json!("production"))
    );
    assert_eq!(
        event.correlation_ids.get("trace_id"),
        Some(&"trace-abc-123".to_string())
    );
    assert_eq!(
        event.correlation_ids.get("span_id"),
        Some(&"span-xyz-456".to_string())
    );
}

#[test]
fn test_decision_event_summary() {
    let output = ValidationOutputs::failure(vec![
        config_validation::contracts::IssueSummary::new("E001", IssueSeverity::Error),
    ]);

    let event = DecisionEvent::new(
        DecisionType::ConfigValidationResult,
        "hash".to_string(),
        output,
        0.5,
        "ref".to_string(),
    );

    let summary = event.summary();
    assert!(summary.contains("config-validation-agent"));
    assert!(summary.contains("valid=false"));
    assert!(summary.contains("errors=1"));
}
