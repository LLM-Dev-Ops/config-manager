//! Security middleware for API requests
//!
//! This module provides security middleware layers including:
//! - Input validation
//! - Rate limiting
//! - Policy enforcement
//! - Request/response sanitization

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use llm_config_security::{
    InputValidator, PolicyEnforcer, RateLimiter, SecurityContext, SecurityError,
};
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
    security
        .input_validator
        .validate(path)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::BAD_REQUEST))?;

    // Validate query parameters
    if !query.is_empty() {
        security
            .input_validator
            .validate(query)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::BAD_REQUEST))?;
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
    let endpoint = request.uri().path();
    security
        .policy_enforcer
        .check_endpoint(endpoint)
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
    let endpoint = request.uri().path();
    security
        .policy_enforcer
        .check_endpoint(endpoint)
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::FORBIDDEN))?;

    // 5. Input validation
    let uri = request.uri();
    security
        .input_validator
        .validate(uri.path())
        .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::BAD_REQUEST))?;

    if let Some(query) = uri.query() {
        security
            .input_validator
            .validate(query)
            .map_err(|e| SecurityResponse::from_security_error(e, StatusCode::BAD_REQUEST))?;
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
    use llm_config_security::{RateLimitConfig, SecurityPolicy};
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_security_state() -> SecurityState {
        let rate_limiter = RateLimiter::new(RateLimitConfig {
            authenticated_rps: 100,
            unauthenticated_rps: 10,
            burst_size: 50,
            window_seconds: 60,
            ban_duration_seconds: 3600,
            ban_threshold: 10,
        });

        let input_validator = InputValidator::default();
        let policy_enforcer = PolicyEnforcer::new(SecurityPolicy::default());

        SecurityState::with_components(rate_limiter, input_validator, policy_enforcer)
    }

    #[test]
    fn test_security_state_creation() {
        let state = SecurityState::new();
        assert!(Arc::strong_count(&state.rate_limiter) == 1);
        assert!(Arc::strong_count(&state.input_validator) == 1);
        assert!(Arc::strong_count(&state.policy_enforcer) == 1);
    }

    #[test]
    fn test_security_response() {
        let response = SecurityResponse::new(
            StatusCode::FORBIDDEN,
            "Access denied".to_string(),
        );
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert_eq!(response.message, "Access denied");
    }
}
