//! Execution context passed between the Core and this repo.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Execution context provided by the Core when invoking this repo.
///
/// Contains the identifiers needed to link this repo's spans back into
/// the global execution graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique identifier for the overall execution.
    pub execution_id: Uuid,
    /// Span ID of the parent (Core-level span) that invoked this repo.
    pub parent_span_id: Uuid,
}
