//! Security middleware for API requests
//!
//! This module provides security middleware layers including:
//! - Input validation
//! - Rate limiting
//! - Policy enforcement
//! - Request/response sanitization

use axum::{
    body::{to_bytes, Body},
    extract::{ConnectInfo, OriginalUri, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use llm_config_security::{
    InputValidator, PolicyEnforcer, RateLimiter, SecurityContext, SecurityError,
};
use percent_encoding::percent_decode_str;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

/// Security middleware state
#[derive(Clone)]
pub struct SecurityState {
    pub rate_limiter: Arc<RateLimiter>,
    pub input_validator: Arc<InputValidator>,
    pub policy_enforcer: Arc<PolicyEnforcer>,
}

impl SecurityState {
    /// Create a new security state with default configuration
    pub fn new() -> Self {
        Self {
            rate_limiter: Arc::new(RateLimiter::new(Default::default())),
            input_validator: Arc::new(InputValidator::default()),
            policy_enforcer: Arc::new(PolicyEnforcer::default()),
        }
    }

    /// Create a new security state with custom components
    pub fn with_components(
        rate_limiter: RateLimiter,
        input_validator: InputValidator,
        policy_enforcer: PolicyEnforcer,
    ) -> Self {
        Self {
            rate_limiter: Arc::new(rate_limiter),
            input_validator: Arc::new(input_validator),
            policy_enforcer: Arc::new(policy_enforcer),
        }
    }
}

impl Default for SecurityState {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate the same single percent-decoded representation used by Axum's Path
/// extractor. Otherwise URL-encoding could conceal an attack from the validator.
fn validate_uri_component(validator: &InputValidator, value: &str) -> Result<(), SecurityResponse> {
    let decoded = percent_decode_str(value).decode_utf8().map_err(|_| {
        SecurityResponse::new(StatusCode::BAD_REQUEST, "Invalid URI encoding".into())
    })?;
    validator
        .validate(&decoded)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::BAD_REQUEST))?;
    Ok(())
}

/// Rate limiting middleware
///
/// Checks requests against rate limits and automatically bans abusive IPs
pub async fn rate_limit_middleware(
    State(security): State<SecurityState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, SecurityResponse> {
    let ip = addr.ip();

    // Check if request has authentication (simplified - in production use proper auth)
    let is_authenticated = headers.get("authorization").is_some();

    // Check rate limit
    security
        .rate_limiter
        .check_request(ip, is_authenticated)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::TOO_MANY_REQUESTS))?;

    Ok(next.run(request).await)
}

/// Input validation middleware
///
/// Validates and sanitizes request paths and query parameters
pub async fn input_validation_middleware(
    State(security): State<SecurityState>,
    request: Request,
    next: Next,
) -> Result<Response, SecurityResponse> {
    let uri = request.uri();
    let path = uri.path();
    let query = uri.query().unwrap_or("");

    // Validate path
    validate_uri_component(&security.input_validator, path)?;

    // Validate query parameters
    if !query.is_empty() {
        validate_uri_component(&security.input_validator, query)?;
    }

    Ok(next.run(request).await)
}

/// Policy enforcement middleware
///
/// Enforces security policies including IP blocking and TLS requirements
pub async fn policy_enforcement_middleware(
    State(security): State<SecurityState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, SecurityResponse> {
    let ip = addr.ip();

    // Check if IP is blocked
    security
        .policy_enforcer
        .check_ip(&ip.to_string())
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;

    // Check TLS (in production, check X-Forwarded-Proto or similar)
    let is_tls = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "https")
        .unwrap_or(false);

    security
        .policy_enforcer
        .check_tls(is_tls, "1.2")
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::UPGRADE_REQUIRED))?;

    // Check CORS origin
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        security
            .policy_enforcer
            .check_origin(origin)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;
    }

    // Check request size
    if let Some(content_length) = headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        security
            .policy_enforcer
            .check_request_size(content_length)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::PAYLOAD_TOO_LARGE))?;
    }

    // Check endpoint access
    let endpoint = request
        .extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path())
        .unwrap_or_else(|| request.uri().path());
    let endpoint = percent_decode_str(endpoint).decode_utf8().map_err(|_| {
        SecurityResponse::new(StatusCode::BAD_REQUEST, "Invalid URI encoding".into())
    })?;
    security
        .policy_enforcer
        .check_endpoint(&endpoint)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;

    Ok(next.run(request).await)
}

/// Security context middleware
///
/// Creates a security context for audit logging and tracking
pub async fn security_context_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let ip = addr.ip();

    // Extract user ID from headers (simplified - in production use proper auth)
    let user_id = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    // Create security context
    let context = SecurityContext::new(user_id, ip.to_string());

    // Store context in request extensions for use in handlers
    request.extensions_mut().insert(context);

    next.run(request).await
}

/// Comprehensive security middleware
///
/// Combines all security checks in a single middleware
pub async fn comprehensive_security_middleware(
    State(security): State<SecurityState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, SecurityResponse> {
    let ip = addr.ip();
    let is_authenticated = headers.get("authorization").is_some();

    // 1. Rate limiting
    security
        .rate_limiter
        .check_request(ip, is_authenticated)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::TOO_MANY_REQUESTS))?;

    // 2. Policy enforcement - IP check
    security
        .policy_enforcer
        .check_ip(&ip.to_string())
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;

    // 3. Policy enforcement - TLS check
    let is_tls = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "https")
        .unwrap_or(false);

    security
        .policy_enforcer
        .check_tls(is_tls, "1.2")
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::UPGRADE_REQUIRED))?;

    // 4. Policy enforcement - endpoint check
    let endpoint = request
        .extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path())
        .unwrap_or_else(|| request.uri().path());
    let endpoint = percent_decode_str(endpoint).decode_utf8().map_err(|_| {
        SecurityResponse::new(StatusCode::BAD_REQUEST, "Invalid URI encoding".into())
    })?;
    security
        .policy_enforcer
        .check_endpoint(&endpoint)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;

    // Enforce browser policy even when this combined layer is used alone.
    // Invalid/non-ASCII and repeated Origin values must not bypass the allowlist.
    let origins = headers.get_all("origin");
    if origins.iter().count() > 1 {
        return Err(SecurityResponse::new(
            StatusCode::FORBIDDEN,
            "Invalid origin".into(),
        ));
    }
    if let Some(origin) = origins.iter().next() {
        let origin = origin
            .to_str()
            .map_err(|_| SecurityResponse::new(StatusCode::FORBIDDEN, "Invalid origin".into()))?;
        security
            .policy_enforcer
            .check_origin(origin)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;
    }

    // Check declared length early, then bound actual bytes as well: chunked
    // requests have no Content-Length and must obey exactly the same policy.
    if let Some(length) = headers.get("content-length") {
        let length = length
            .to_str()
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| {
                SecurityResponse::new(StatusCode::BAD_REQUEST, "Invalid content length".into())
            })?;
        security
            .policy_enforcer
            .check_request_size(length)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::PAYLOAD_TOO_LARGE))?;
    }
    let body = std::mem::take(request.body_mut());
    let bytes = to_bytes(body, security.policy_enforcer.max_request_size())
        .await
        .map_err(|_| {
            SecurityResponse::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Request body exceeds policy or is unreadable".into(),
            )
        })?;
    *request.body_mut() = Body::from(bytes);

    // 5. Input validation
    let uri = request.uri();
    validate_uri_component(&security.input_validator, uri.path())?;

    if let Some(query) = uri.query() {
        validate_uri_component(&security.input_validator, query)?;
    }

    // 6. Create security context
    let user_id = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    let context = SecurityContext::new(user_id, ip.to_string());
    request.extensions_mut().insert(context);

    Ok(next.run(request).await)
}

/// Security error response
pub struct SecurityResponse {
    status: StatusCode,
    message: String,
}

impl SecurityResponse {
    pub fn new(status: StatusCode, message: String) -> Self {
        Self { status, message }
    }

    pub fn from_security_error(error: SecurityError, status: StatusCode) -> Self {
        Self {
            status,
            message: error.public_message(),
        }
    }
}

impl IntoResponse for SecurityResponse {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.status.canonical_reason().unwrap_or("Security Error"),
            "message": self.message,
        });

        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_state_creation() {
        let state = SecurityState::new();
        assert!(Arc::strong_count(&state.rate_limiter) == 1);
        assert!(Arc::strong_count(&state.input_validator) == 1);
        assert!(Arc::strong_count(&state.policy_enforcer) == 1);
    }

    #[test]
    fn test_security_response() {
        let response = SecurityResponse::new(StatusCode::FORBIDDEN, "Access denied".to_string());
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert_eq!(response.message, "Access denied");
    }
}
