//! Core execution span types for the Agentics execution system.
//!
//! Defines `ExecutionSpan`, `SpanType`, and `SpanStatus` used to build
//! hierarchical execution graphs: Core -> Repo -> Agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of an execution span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Running,
    Completed,
    Failed,
}

/// Type of execution span in the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    Repo,
    Agent,
}

/// A single execution span in the Agentics execution graph.
///
/// Spans form a tree: Core -> Repo -> Agent(s).
/// Each span is append-only, causally ordered via `parent_span_id`,
/// and JSON-serializable without loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSpan {
    pub span_id: Uuid,
    pub parent_span_id: Uuid,
    pub execution_id: Uuid,
    pub span_type: SpanType,
    pub status: SpanStatus,
    pub name: String,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub attributes: HashMap<String, serde_json::Value>,
    pub artifacts: Vec<serde_json::Value>,
    pub children: Vec<ExecutionSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ExecutionSpan {
    /// Create a new repo-level span.
    pub fn new_repo(execution_id: Uuid, parent_span_id: Uuid, repo_name: &str) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "repo_name".to_string(),
            serde_json::Value::String(repo_name.to_string()),
        );

        Self {
            span_id: Uuid::new_v4(),
            parent_span_id,
            execution_id,
            span_type: SpanType::Repo,
            status: SpanStatus::Running,
            name: repo_name.to_string(),
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            attributes,
            artifacts: Vec::new(),
            children: Vec::new(),
            error: None,
        }
    }

    /// Create a new agent-level span.
    pub fn new_agent(execution_id: Uuid, parent_span_id: Uuid, agent_name: &str) -> Self {
        let mut attributes = HashMap::new();
        attributes.insert(
            "agent_name".to_string(),
            serde_json::Value::String(agent_name.to_string()),
        );

        Self {
            span_id: Uuid::new_v4(),
            parent_span_id,
            execution_id,
            span_type: SpanType::Agent,
            status: SpanStatus::Running,
            name: agent_name.to_string(),
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            attributes,
            artifacts: Vec::new(),
            children: Vec::new(),
            error: None,
        }
    }

    /// Mark the span as completed.
    pub fn complete(&mut self) {
        let now = Utc::now();
        self.status = SpanStatus::Completed;
        self.ended_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .max(0) as u64,
        );
    }

    /// Mark the span as failed with an error message.
    pub fn fail(&mut self, error: String) {
        let now = Utc::now();
        self.status = SpanStatus::Failed;
        self.ended_at = Some(now);
        self.duration_ms = Some(
            (now - self.started_at)
                .num_milliseconds()
                .max(0) as u64,
        );
        self.error = Some(error);
    }

    /// Attach an artifact to this span.
    pub fn attach_artifact(&mut self, artifact: serde_json::Value) {
        self.artifacts.push(artifact);
    }

    /// Add a child span.
    pub fn add_child(&mut self, child: ExecutionSpan) {
        self.children.push(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_repo_span() {
        let exec_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let span = ExecutionSpan::new_repo(exec_id, parent_id, "config-manager");

        assert_eq!(span.span_type, SpanType::Repo);
        assert_eq!(span.status, SpanStatus::Running);
        assert_eq!(span.name, "config-manager");
        assert_eq!(span.execution_id, exec_id);
        assert_eq!(span.parent_span_id, parent_id);
        assert_eq!(
            span.attributes.get("repo_name"),
            Some(&serde_json::Value::String("config-manager".to_string()))
        );
    }

    #[test]
    fn test_new_agent_span() {
        let exec_id = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let span = ExecutionSpan::new_agent(exec_id, parent_id, "schema-truth");

        assert_eq!(span.span_type, SpanType::Agent);
        assert_eq!(span.status, SpanStatus::Running);
        assert_eq!(span.name, "schema-truth");
    }

    #[test]
    fn test_complete_span() {
        let mut span = ExecutionSpan::new_agent(Uuid::new_v4(), Uuid::new_v4(), "test");
        span.complete();

        assert_eq!(span.status, SpanStatus::Completed);
        assert!(span.ended_at.is_some());
        assert!(span.duration_ms.is_some());
        assert!(span.error.is_none());
    }

    #[test]
    fn test_fail_span() {
        let mut span = ExecutionSpan::new_agent(Uuid::new_v4(), Uuid::new_v4(), "test");
        span.fail("something went wrong".to_string());

        assert_eq!(span.status, SpanStatus::Failed);
        assert!(span.ended_at.is_some());
        assert_eq!(span.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_attach_artifact() {
        let mut span = ExecutionSpan::new_agent(Uuid::new_v4(), Uuid::new_v4(), "test");
        span.attach_artifact(serde_json::json!({"result": "ok"}));

        assert_eq!(span.artifacts.len(), 1);
        assert_eq!(span.artifacts[0], serde_json::json!({"result": "ok"}));
    }

    #[test]
    fn test_json_serialization_roundtrip() {
        let mut span = ExecutionSpan::new_repo(Uuid::new_v4(), Uuid::new_v4(), "config-manager");
        let mut child = ExecutionSpan::new_agent(span.execution_id, span.span_id, "schema-truth");
        child.attach_artifact(serde_json::json!({"valid": true}));
        child.complete();
        span.add_child(child);
        span.complete();

        let json = serde_json::to_string(&span).unwrap();
        let deserialized: ExecutionSpan = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.span_id, span.span_id);
        assert_eq!(deserialized.children.len(), 1);
        assert_eq!(deserialized.children[0].name, "schema-truth");
    }
}
