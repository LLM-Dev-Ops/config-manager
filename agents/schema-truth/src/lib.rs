//! Schema Truth Agent
//!
//! Deterministic schema validation agent that defines configuration truth
//! and emits schema_violation_signal to ruvector-service.
//!
//! # Performance Budgets
//! - MAX_TOKENS: 800
//! - MAX_LATENCY_MS: 1500
//!
//! # Design Principles
//! - Deterministic: Same input always produces same output
//! - Stateless: No side effects during validation
//! - Traceable: Full audit trail via DecisionEvents

pub mod client;
pub mod engine;
pub mod handler;
pub mod telemetry;

// Re-export contracts
#[path = "../contracts/mod.rs"]
pub mod contracts;

pub use contracts::*;
