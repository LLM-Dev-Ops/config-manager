//! Axum extractor for execution context from HTTP headers.
//!
//! Reads `X-Parent-Span-Id` and `X-Execution-Id` from request headers.
//! Rejects with 400 if `X-Parent-Span-Id` is missing or invalid.

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::context::ExecutionContext;

/// Axum extractor that reads execution context from HTTP headers.
///
/// **Enforcement**: Requests without a valid `X-Parent-Span-Id` header
/// are rejected with `400 BAD_REQUEST`. This ensures no operation
/// executes without being part of an execution graph.
pub struct ExecutionContextExtractor(pub ExecutionContext);

/// Rejection type for missing or invalid execution context headers.
pub struct ExecutionContextRejection {
    message: String,
}

impl IntoResponse for ExecutionContextRejection {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "MISSING_PARENT_SPAN_ID",
                "message": self.message
            })),
        )
            .into_response()
    }
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for ExecutionContextExtractor {
    type Rejection = ExecutionContextRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;

        let parent_span_id = headers
            .get("x-parent-span-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| Uuid::parse_str(v).ok())
            .ok_or_else(|| ExecutionContextRejection {
                message: "X-Parent-Span-Id header is required and must be a valid UUID"
                    .to_string(),
            })?;

        let execution_id = headers
            .get("x-execution-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| Uuid::parse_str(v).ok())
            .unwrap_or_else(Uuid::new_v4);

        Ok(Self(ExecutionContext {
            execution_id,
            parent_span_id,
        }))
    }
}
