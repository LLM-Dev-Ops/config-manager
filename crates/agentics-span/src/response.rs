//! Response envelope that wraps agent output with the execution span tree.

use serde::{Deserialize, Serialize};

use crate::span::{ExecutionSpan, SpanStatus};

/// Response envelope for instrumented endpoints.
///
/// Always includes the span tree, even on failure. The `success` field
/// reflects the repo span status — it is `false` if any agent failed
/// or if no agent spans were emitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEnvelope<T> {
    /// Whether the execution completed successfully.
    pub success: bool,
    /// The agent output data (present on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message (present on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The complete execution span tree (always present).
    pub span_tree: ExecutionSpan,
}

impl<T: Serialize> ExecutionEnvelope<T> {
    /// Create a successful response. The `success` field is derived from
    /// the span tree status — if the tree was finalized as failed (e.g.
    /// no agent spans), this will still be `false`.
    pub fn success(data: T, span_tree: ExecutionSpan) -> Self {
        Self {
            success: span_tree.status == SpanStatus::Completed,
            data: Some(data),
            error: None,
            span_tree,
        }
    }

    /// Create a failure response. Span tree is always included.
    pub fn failure(error: String, span_tree: ExecutionSpan) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            span_tree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::ExecutionSpan;
    use uuid::Uuid;

    #[test]
    fn test_success_envelope() {
        let mut span =
            ExecutionSpan::new_repo(Uuid::new_v4(), Uuid::new_v4(), "config-manager");
        span.complete();

        let envelope = ExecutionEnvelope::success("test data".to_string(), span);
        assert!(envelope.success);
        assert_eq!(envelope.data, Some("test data".to_string()));
        assert!(envelope.error.is_none());
    }

    #[test]
    fn test_failure_envelope() {
        let mut span =
            ExecutionSpan::new_repo(Uuid::new_v4(), Uuid::new_v4(), "config-manager");
        span.fail("no agents".to_string());

        let envelope =
            ExecutionEnvelope::<String>::failure("no agents".to_string(), span);
        assert!(!envelope.success);
        assert!(envelope.data.is_none());
        assert_eq!(envelope.error, Some("no agents".to_string()));
    }

    #[test]
    fn test_success_with_failed_tree_is_not_success() {
        let mut span =
            ExecutionSpan::new_repo(Uuid::new_v4(), Uuid::new_v4(), "config-manager");
        span.fail("agent failed".to_string());

        let envelope = ExecutionEnvelope::success("data".to_string(), span);
        // success is derived from span status, so this is false
        assert!(!envelope.success);
    }

    #[test]
    fn test_json_serialization() {
        let mut span =
            ExecutionSpan::new_repo(Uuid::new_v4(), Uuid::new_v4(), "config-manager");
        span.complete();

        let envelope = ExecutionEnvelope::success(serde_json::json!({"valid": true}), span);
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("span_tree"));
        assert!(json.contains("\"success\":true"));
    }
}
