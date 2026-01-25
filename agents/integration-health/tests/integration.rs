//! Integration tests for Integration Health Agent

use integration_health::contracts::*;
use integration_health::engine::HealthCheckEngine;
use std::collections::HashMap;

fn create_test_adapter(adapter_type: AdapterType, endpoint: &str) -> AdapterConfig {
    AdapterConfig {
        id: format!("test-{:?}", adapter_type).to_lowercase(),
        adapter_type,
        endpoint: endpoint.to_string(),
        auth: None,
        health_path: None,
        properties: HashMap::new(),
    }
}

#[tokio::test]
async fn test_check_http_adapter() {
    let engine = HealthCheckEngine::new();

    // Use a reliable test endpoint
    let adapter = create_test_adapter(AdapterType::Http, "https://httpbin.org/status/200");
    let input = HealthCheckEngine::create_input(vec![adapter], "test".to_string());

    let output = engine.check(&input).await;

    assert_eq!(output.adapters_checked, 1);
    // Note: actual health depends on network availability
}

#[tokio::test]
async fn test_check_unreachable_adapter() {
    let engine = HealthCheckEngine::new();

    // Use an unreachable endpoint
    let adapter = create_test_adapter(AdapterType::Tcp, "192.0.2.1:12345");
    let mut input = HealthCheckEngine::create_input(vec![adapter], "test".to_string());
    input.options.timeout_ms = 100; // Short timeout

    let output = engine.check(&input).await;

    assert_eq!(output.adapters_checked, 1);
    assert_eq!(output.unhealthy_count, 1);
    assert!(!output.is_healthy);
}

#[tokio::test]
async fn test_check_multiple_adapters_parallel() {
    let engine = HealthCheckEngine::new();

    let adapters = vec![
        create_test_adapter(AdapterType::Tcp, "192.0.2.1:12345"),
        create_test_adapter(AdapterType::Tcp, "192.0.2.2:12345"),
    ];

    let mut input = HealthCheckEngine::create_input(adapters, "test".to_string());
    input.options.parallel = true;
    input.options.timeout_ms = 100;

    let output = engine.check(&input).await;

    assert_eq!(output.adapters_checked, 2);
}

#[tokio::test]
async fn test_deterministic_hash() {
    let adapter = create_test_adapter(AdapterType::Redis, "localhost:6379");

    let input1 = HealthCheckEngine::create_input(vec![adapter.clone()], "test".to_string());
    let input2 = HealthCheckEngine::create_input(vec![adapter], "test".to_string());

    let hash1 = HealthCheckEngine::compute_inputs_hash(&input1);
    let hash2 = HealthCheckEngine::compute_inputs_hash(&input2);

    // Same adapters should produce same hash
    assert_eq!(hash1, hash2);
}

#[tokio::test]
async fn test_health_score_calculation() {
    let results = vec![
        AdapterHealthResult::healthy("adapter-1", AdapterType::Http, 50),
        AdapterHealthResult::healthy("adapter-2", AdapterType::Redis, 30),
        AdapterHealthResult::degraded("adapter-3", AdapterType::Postgres, 100, "slow"),
    ];

    let output = IntegrationHealthOutput::healthy(uuid::Uuid::new_v4(), results);

    // 2 healthy + 0.5 * 1 degraded = 2.5 / 3 = 0.833...
    assert!(output.health_score > 0.8);
    assert!(output.is_healthy); // No unhealthy adapters
    assert_eq!(output.healthy_count, 2);
    assert_eq!(output.degraded_count, 1);
}

#[tokio::test]
async fn test_decision_event_creation() {
    let results = vec![
        AdapterHealthResult::healthy("adapter-1", AdapterType::Http, 50),
    ];

    let output = IntegrationHealthOutput::healthy(uuid::Uuid::new_v4(), results);

    let signal = IntegrationHealthSignal::from_health_check(
        "test-hash".to_string(),
        &output,
        "test-execution".to_string(),
    );

    assert_eq!(signal.agent_id, IntegrationHealthSignal::AGENT_ID);
    assert_eq!(signal.signal_type, "integration_health_signal");
    assert!(signal.confidence > 0.0);
    assert!(signal.confidence <= 1.0);
}

#[tokio::test]
async fn test_adapter_preset_creation() {
    let vault = AdapterPreset::hashicorp_vault("http://vault:8200");
    assert_eq!(vault.adapter_type, AdapterType::HashicorpVault);
    assert_eq!(vault.health_path, Some("/v1/sys/health".to_string()));

    let redis = AdapterPreset::redis("localhost", 6379);
    assert_eq!(redis.adapter_type, AdapterType::Redis);
    assert_eq!(redis.endpoint, "localhost:6379");

    let gcp = AdapterPreset::gcp_secret_manager("my-project");
    assert_eq!(gcp.adapter_type, AdapterType::GcpSecretManager);
    assert!(gcp.properties.contains_key("project_id"));
}
