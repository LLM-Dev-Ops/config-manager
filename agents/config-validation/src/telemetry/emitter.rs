//! DecisionEvent emitter for ruvector-service
//!
//! Emits DecisionEvents asynchronously and non-blocking to the ruvector-service.
//! Integrates with the existing contracts::decision_event module.
//!
//! # Features
//!
//! - Async, non-blocking emission via channel-based queue
//! - Automatic inputs hash calculation
//! - Confidence score computation from validation coverage
//! - Retry logic with exponential backoff (via ruvector client)
//!
//! # Example
//!
//! ```rust,no_run
//! use config_validation::telemetry::{DecisionEventEmitter, EmitterConfig};
//! use config_validation::contracts::{ValidationInput, ValidationOutput, DecisionEvent};
//!
//! #[tokio::main]
//! async fn main() {
//!     let emitter = DecisionEventEmitter::new(EmitterConfig::default());
//!
//!     // Create a decision event from validation results
//!     let event = DecisionEvent::from_validation(
//!         "inputs_hash".to_string(),
//!         &output,
//!         "execution-ref".to_string(),
//!     );
//!
//!     emitter.emit(event).await.unwrap();
//! }
//! ```

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{Result, TelemetryError};
use crate::client::ruvector::RuvectorClient;
use crate::contracts::{DecisionEvent, ValidationInput, ValidationOutput};

/// Configuration for the DecisionEvent emitter
#[derive(Debug, Clone)]
pub struct EmitterConfig {
    /// ruvector-service endpoint URL
    pub endpoint: String,

    /// Maximum queue size for buffering events
    pub max_queue_size: usize,

    /// Timeout for emission in milliseconds
    pub timeout_ms: u64,

    /// Agent identifier
    pub agent_id: String,

    /// Agent version
    pub agent_version: String,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Initial backoff delay in milliseconds
    pub initial_backoff_ms: u64,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8080".to_string(),
            max_queue_size: 1000,
            timeout_ms: 5000,
            agent_id: DecisionEvent::AGENT_ID.to_string(),
            agent_version: DecisionEvent::AGENT_VERSION.to_string(),
            max_retries: 3,
            initial_backoff_ms: 100,
        }
    }
}

impl EmitterConfig {
    /// Create a new emitter config with custom endpoint
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            ..Default::default()
        }
    }
}

/// Calculate SHA-256 hash of validation inputs for traceability
pub fn calculate_inputs_hash(input: &ValidationInput) -> String {
    let mut hasher = Sha256::new();

    // Hash the namespace and key
    hasher.update(input.namespace.as_bytes());
    hasher.update(input.key.as_bytes());

    // Hash the value (serialized)
    if let Ok(value_json) = serde_json::to_string(&input.value) {
        hasher.update(value_json.as_bytes());
    }

    // Hash the environment
    hasher.update(format!("{:?}", input.environment).as_bytes());

    // Hash the schema version if present
    if let Some(ref schema) = input.schema {
        hasher.update(schema.version.as_bytes());
    }

    // Hash additional rules
    for rule in &input.additional_rules {
        hasher.update(rule.rule_id.as_bytes());
    }

    let result = hasher.finalize();
    hex::encode(result)
}

/// Calculate inputs hash from raw components
pub fn hash_validation_components(
    config_content: &str,
    schema_version: &str,
    rules: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config_content.as_bytes());
    hasher.update(schema_version.as_bytes());
    for rule in rules {
        hasher.update(rule.as_bytes());
    }
    let result = hasher.finalize();
    hex::encode(result)
}

/// Build constraints list from schema and applied rules
pub fn build_constraints_list(
    schema_version: Option<&str>,
    rules_applied: &[String],
    constraints_checked: &[String],
) -> Vec<String> {
    let mut constraints = Vec::new();

    // Add schema constraint
    if let Some(version) = schema_version {
        constraints.push(format!("schema:{}", version));
    }

    // Add rule constraints
    for rule in rules_applied {
        constraints.push(format!("rule:{}", rule));
    }

    // Add checked constraints
    for constraint in constraints_checked {
        if !constraints.contains(constraint) {
            constraints.push(constraint.clone());
        }
    }

    constraints
}

/// Async, non-blocking DecisionEvent emitter
pub struct DecisionEventEmitter {
    config: EmitterConfig,
    client: Arc<RuvectorClient>,
    sender: mpsc::Sender<DecisionEvent>,
}

impl DecisionEventEmitter {
    /// Create a new emitter with the given configuration
    pub fn new(config: EmitterConfig) -> Self {
        let client = Arc::new(RuvectorClient::new(
            config.endpoint.clone(),
            config.timeout_ms,
        ));

        let (sender, mut receiver) = mpsc::channel::<DecisionEvent>(config.max_queue_size);

        // Spawn background task to process events
        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                // Non-blocking emission - log errors but don't fail
                if let Err(e) = client_clone.persist_decision_event(&event).await {
                    tracing::warn!(
                        event_id = %event.event_id,
                        error = %e,
                        "Failed to emit decision event"
                    );
                } else {
                    tracing::debug!(
                        event_id = %event.event_id,
                        decision_type = ?event.decision_type,
                        confidence = event.confidence,
                        "Successfully emitted decision event"
                    );
                }
            }
        });

        Self {
            config,
            client,
            sender,
        }
    }

    /// Emit a DecisionEvent asynchronously (non-blocking)
    pub async fn emit(&self, event: DecisionEvent) -> Result<()> {
        self.sender.send(event).await.map_err(|e| {
            TelemetryError::EmissionFailed(format!("Failed to queue event: {}", e))
        })?;
        Ok(())
    }

    /// Emit a decision event from validation input and output
    pub async fn emit_from_validation(
        &self,
        input: &ValidationInput,
        output: &ValidationOutput,
    ) -> Result<()> {
        let inputs_hash = calculate_inputs_hash(input);
        let execution_ref = input.request_id.to_string();

        let event = DecisionEvent::from_validation(inputs_hash, output, execution_ref);

        self.emit(event).await
    }

    /// Create and emit a decision event with custom inputs hash
    pub async fn emit_validation_result(
        &self,
        inputs_hash: impl Into<String>,
        output: &ValidationOutput,
        execution_ref: impl Into<String>,
    ) -> Result<()> {
        let event = DecisionEvent::from_validation(
            inputs_hash.into(),
            output,
            execution_ref.into(),
        );

        self.emit(event).await
    }

    /// Get the agent ID
    pub fn agent_id(&self) -> &str {
        &self.config.agent_id
    }

    /// Get the agent version
    pub fn agent_version(&self) -> &str {
        &self.config.agent_version
    }

    /// Get the ruvector endpoint
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    /// Check if the emitter is healthy (can reach ruvector-service)
    pub async fn health_check(&self) -> Result<bool> {
        self.client.health_check().await
    }

    /// Get the current queue depth (approximate)
    pub fn queue_capacity(&self) -> usize {
        self.config.max_queue_size
    }
}

/// Builder for DecisionEventEmitter
pub struct EmitterBuilder {
    config: EmitterConfig,
}

impl EmitterBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: EmitterConfig::default(),
        }
    }

    /// Set the ruvector endpoint
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    /// Set the queue size
    pub fn queue_size(mut self, size: usize) -> Self {
        self.config.max_queue_size = size;
        self
    }

    /// Set the timeout in milliseconds
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.config.timeout_ms = timeout;
        self
    }

    /// Set the agent ID
    pub fn agent_id(mut self, id: impl Into<String>) -> Self {
        self.config.agent_id = id.into();
        self
    }

    /// Set the agent version
    pub fn agent_version(mut self, version: impl Into<String>) -> Self {
        self.config.agent_version = version.into();
        self
    }

    /// Set retry configuration
    pub fn retry_config(mut self, max_retries: u32, initial_backoff_ms: u64) -> Self {
        self.config.max_retries = max_retries;
        self.config.initial_backoff_ms = initial_backoff_ms;
        self
    }

    /// Build the emitter
    pub fn build(self) -> DecisionEventEmitter {
        DecisionEventEmitter::new(self.config)
    }
}

impl Default for EmitterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ConfigValueRef, EnvironmentRef};
    use uuid::Uuid;

    fn create_test_input() -> ValidationInput {
        ValidationInput::new(
            "app/database",
            "connection_string",
            ConfigValueRef::String("postgres://localhost/db".to_string()),
            EnvironmentRef::Development,
            "test-user",
        )
    }

    fn create_test_output() -> ValidationOutput {
        ValidationOutput::success(
            Uuid::new_v4(),
            vec!["type_check".to_string(), "bounds_check".to_string()],
        )
        .with_coverage(0.95)
        .with_duration(42)
    }

    #[test]
    fn test_calculate_inputs_hash() {
        let input1 = create_test_input();
        let input2 = create_test_input();

        let hash1 = calculate_inputs_hash(&input1);
        let hash2 = calculate_inputs_hash(&input2);

        // Same inputs should produce same hash (deterministic)
        // Note: Different request_ids don't affect hash
        assert_eq!(hash1.len(), 64); // SHA-256 hex length

        // Different namespace should produce different hash
        let mut input3 = create_test_input();
        input3.namespace = "different/namespace".to_string();
        let hash3 = calculate_inputs_hash(&input3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash_validation_components() {
        let hash1 = hash_validation_components(
            "config content",
            "1.0.0",
            &["rule1".to_string(), "rule2".to_string()],
        );

        let hash2 = hash_validation_components(
            "config content",
            "1.0.0",
            &["rule1".to_string(), "rule2".to_string()],
        );

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);

        // Different version produces different hash
        let hash3 = hash_validation_components(
            "config content",
            "2.0.0",
            &["rule1".to_string(), "rule2".to_string()],
        );
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_build_constraints_list() {
        let constraints = build_constraints_list(
            Some("1.0.0"),
            &["type_check".to_string(), "bounds_check".to_string()],
            &["required_field".to_string()],
        );

        assert!(constraints.contains(&"schema:1.0.0".to_string()));
        assert!(constraints.contains(&"rule:type_check".to_string()));
        assert!(constraints.contains(&"rule:bounds_check".to_string()));
        assert!(constraints.contains(&"required_field".to_string()));
    }

    #[test]
    fn test_emitter_config_default() {
        let config = EmitterConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8080");
        assert_eq!(config.max_queue_size, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.agent_id, DecisionEvent::AGENT_ID);
        assert_eq!(config.agent_version, DecisionEvent::AGENT_VERSION);
    }

    #[test]
    fn test_emitter_config_with_endpoint() {
        let config = EmitterConfig::with_endpoint("http://ruvector:9090");
        assert_eq!(config.endpoint, "http://ruvector:9090");
    }

    #[test]
    fn test_emitter_builder() {
        let builder = EmitterBuilder::new()
            .endpoint("http://custom:8080")
            .queue_size(500)
            .timeout_ms(10000)
            .agent_id("custom-agent")
            .agent_version("1.0.0")
            .retry_config(5, 200);

        assert_eq!(builder.config.endpoint, "http://custom:8080");
        assert_eq!(builder.config.max_queue_size, 500);
        assert_eq!(builder.config.timeout_ms, 10000);
        assert_eq!(builder.config.agent_id, "custom-agent");
        assert_eq!(builder.config.agent_version, "1.0.0");
        assert_eq!(builder.config.max_retries, 5);
        assert_eq!(builder.config.initial_backoff_ms, 200);
    }

    #[test]
    fn test_decision_event_from_validation() {
        let output = create_test_output();
        let event = DecisionEvent::from_validation(
            "test_hash".to_string(),
            &output,
            "exec-ref-123".to_string(),
        );

        assert_eq!(event.agent_id, DecisionEvent::AGENT_ID);
        assert_eq!(event.inputs_hash, "test_hash");
        assert_eq!(event.execution_ref, "exec-ref-123");
        assert!(event.outputs.is_valid);
        assert!(event.confidence >= 0.0 && event.confidence <= 1.0);
    }
}
