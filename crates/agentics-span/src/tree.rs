//! Span tree builder for assembling repo + agent span hierarchies.
//!
//! Enforces the invariant that at least one agent span must exist
//! for the execution to be considered valid.

use crate::context::ExecutionContext;
use crate::span::{ExecutionSpan, SpanStatus};

/// Builder for constructing the execution span tree.
///
/// Creates a repo-level span on construction, then allows adding
/// agent-level spans. On finalization, enforces that at least one
/// agent span was emitted.
pub struct SpanTreeBuilder {
    repo_span: ExecutionSpan,
    agent_spans: Vec<ExecutionSpan>,
}

impl SpanTreeBuilder {
    /// Create a new builder with a repo-level span.
    pub fn new(ctx: &ExecutionContext, repo_name: &str) -> Self {
        Self {
            repo_span: ExecutionSpan::new_repo(ctx.execution_id, ctx.parent_span_id, repo_name),
            agent_spans: Vec::new(),
        }
    }

    /// Start a new agent-level span parented to the repo span.
    pub fn start_agent_span(&self, agent_name: &str) -> ExecutionSpan {
        ExecutionSpan::new_agent(
            self.repo_span.execution_id,
            self.repo_span.span_id,
            agent_name,
        )
    }

    /// Add a completed (or failed) agent span to the tree.
    pub fn add_completed_agent_span(&mut self, span: ExecutionSpan) {
        self.agent_spans.push(span);
    }

    /// Finalize the tree, enforcing all invariants.
    ///
    /// - If no agent spans were emitted, the repo span is marked FAILED.
    /// - If any agent span failed, the repo span is marked FAILED.
    /// - Otherwise, the repo span is marked Completed.
    ///
    /// Returns the complete span tree with agent spans nested as children.
    pub fn finalize(mut self) -> ExecutionSpan {
        if self.agent_spans.is_empty() {
            self.repo_span
                .fail("No agent spans emitted — execution is INVALID".to_string());
        } else {
            let any_failed = self
                .agent_spans
                .iter()
                .any(|s| s.status == SpanStatus::Failed);
            if any_failed {
                self.repo_span
                    .fail("One or more agent spans failed".to_string());
            } else {
                self.repo_span.complete();
            }
        }

        self.repo_span.children = self.agent_spans;
        self.repo_span
    }

    /// Finalize the tree as failed with an explicit error.
    ///
    /// The repo span is marked FAILED, but all collected agent spans
    /// (including partial ones) are still included.
    pub fn finalize_failed(mut self, error: String) -> ExecutionSpan {
        self.repo_span.fail(error);
        self.repo_span.children = self.agent_spans;
        self.repo_span
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: Uuid::new_v4(),
            parent_span_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn test_finalize_no_agents_is_failed() {
        let ctx = test_ctx();
        let tree = SpanTreeBuilder::new(&ctx, "config-manager");
        let span = tree.finalize();

        assert_eq!(span.status, SpanStatus::Failed);
        assert!(span.error.is_some());
        assert!(span.error.unwrap().contains("No agent spans"));
        assert!(span.children.is_empty());
    }

    #[test]
    fn test_finalize_with_completed_agent() {
        let ctx = test_ctx();
        let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
        let mut agent = tree.start_agent_span("schema-truth");
        agent.complete();
        tree.add_completed_agent_span(agent);
        let span = tree.finalize();

        assert_eq!(span.status, SpanStatus::Completed);
        assert!(span.error.is_none());
        assert_eq!(span.children.len(), 1);
        assert_eq!(span.children[0].name, "schema-truth");
        assert_eq!(span.children[0].status, SpanStatus::Completed);
    }

    #[test]
    fn test_finalize_with_failed_agent() {
        let ctx = test_ctx();
        let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
        let mut agent = tree.start_agent_span("schema-truth");
        agent.fail("engine error".to_string());
        tree.add_completed_agent_span(agent);
        let span = tree.finalize();

        assert_eq!(span.status, SpanStatus::Failed);
        assert!(span.error.is_some());
        assert_eq!(span.children.len(), 1);
    }

    #[test]
    fn test_finalize_failed_preserves_spans() {
        let ctx = test_ctx();
        let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");
        let mut agent = tree.start_agent_span("schema-truth");
        agent.complete();
        tree.add_completed_agent_span(agent);
        let span = tree.finalize_failed("explicit failure".to_string());

        assert_eq!(span.status, SpanStatus::Failed);
        assert_eq!(span.error, Some("explicit failure".to_string()));
        assert_eq!(span.children.len(), 1); // spans preserved
    }

    #[test]
    fn test_parent_span_id_chain() {
        let ctx = test_ctx();
        let tree = SpanTreeBuilder::new(&ctx, "config-manager");
        let agent = tree.start_agent_span("schema-truth");

        // Agent's parent is repo span
        assert_eq!(agent.parent_span_id, tree.repo_span.span_id);
        // Repo's parent is the Core span
        assert_eq!(tree.repo_span.parent_span_id, ctx.parent_span_id);
        // All share the same execution_id
        assert_eq!(agent.execution_id, ctx.execution_id);
    }

    #[test]
    fn test_multiple_agents() {
        let ctx = test_ctx();
        let mut tree = SpanTreeBuilder::new(&ctx, "config-manager");

        let mut a1 = tree.start_agent_span("schema-truth");
        a1.complete();
        tree.add_completed_agent_span(a1);

        let mut a2 = tree.start_agent_span("integration-health");
        a2.complete();
        tree.add_completed_agent_span(a2);

        let span = tree.finalize();
        assert_eq!(span.status, SpanStatus::Completed);
        assert_eq!(span.children.len(), 2);
    }
}
