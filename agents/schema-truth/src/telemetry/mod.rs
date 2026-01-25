//! Telemetry emission for schema_violation_signal
//!
//! Non-blocking emission to ruvector-service.

use crate::contracts::*;
use std::env;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Telemetry emitter for schema violation signals
pub struct TelemetryEmitter {
    sender: mpsc::Sender<SchemaViolationSignal>,
}

impl TelemetryEmitter {
    /// Create new emitter
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);

        // Spawn background task
        tokio::spawn(Self::background_emitter(receiver));

        Self { sender }
    }

    /// Emit a signal
    pub async fn emit(&self, signal: SchemaViolationSignal) -> Result<(), String> {
        self.sender
            .send(signal)
            .await
            .map_err(|e| format!("Failed to queue signal: {}", e))
    }

    /// Background emission task
    async fn background_emitter(mut receiver: mpsc::Receiver<SchemaViolationSignal>) {
        let client = RuvectorClient::new();

        while let Some(signal) = receiver.recv().await {
            info!(
                event_id = %signal.event_id,
                signal_type = %signal.signal_type,
                "Emitting schema violation signal"
            );

            if let Err(e) = client.emit_signal(&signal).await {
                error!(error = %e, "Failed to emit signal to ruvector-service");
            }
        }
    }
}

impl Default for TelemetryEmitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Ruvector service client
pub struct RuvectorClient {
    url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl RuvectorClient {
    /// Create new client from environment
    pub fn new() -> Self {
        Self {
            url: env::var("RUVECTOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            api_key: env::var("RUVECTOR_API_KEY").ok(),
            client: reqwest::Client::new(),
        }
    }

    /// Emit signal to ruvector-service
    pub async fn emit_signal(&self, signal: &SchemaViolationSignal) -> Result<(), String> {
        let url = format!("{}/api/v1/signals", self.url);

        let mut request = self.client.post(&url).json(signal);

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "Ruvector returned error: {}",
                response.status()
            ))
        }
    }

    /// Emit batch of signals
    pub async fn emit_batch(&self, batch: &SchemaViolationSignalBatch) -> Result<(), String> {
        let url = format!("{}/api/v1/signals/batch", self.url);

        let mut request = self.client.post(&url).json(batch);

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "Ruvector returned error: {}",
                response.status()
            ))
        }
    }
}

impl Default for RuvectorClient {
    fn default() -> Self {
        Self::new()
    }
}
