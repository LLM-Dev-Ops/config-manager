//! Agentics execution span types for Foundational Execution Unit instrumentation.
//!
//! This crate provides the shared types used by all agents and the API server
//! to emit execution spans into the Agentics hierarchical ExecutionGraph.
//!
//! # Span Hierarchy
//!
//! ```text
//! Core (external)
//!   └─ Repo (this repo: config-manager)
//!       └─ Agent (one or more: schema-truth, integration-health, config-validation)
//! ```
//!
//! # Usage
//!
//! 1. Use `ExecutionContextExtractor` in Axum handlers to extract execution context from headers.
//! 2. Use `SpanTreeBuilder` to create the repo span and agent spans.
//! 3. Use `ExecutionEnvelope` to wrap the response with the span tree.

pub mod context;
pub mod extract;
pub mod response;
pub mod span;
pub mod tree;

pub use context::ExecutionContext;
pub use extract::ExecutionContextExtractor;
pub use response::ExecutionEnvelope;
pub use span::{ExecutionSpan, SpanStatus, SpanType};
pub use tree::SpanTreeBuilder;
