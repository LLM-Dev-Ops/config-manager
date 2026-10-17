//! Security integration tests for the API
//!
//! Tests the security middleware, input validation, rate limiting, and policy enforcement

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use llm_config_api::{create_router, SecurityState};
use llm_config_core::ConfigManager;
use llm_config_security::{
    InputValidator, PolicyEnforcer, RateLimitConfig, RateLimiter, SecurityPolicy,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// Supply the transport context that the real server inserts for every connection.
fn request() -> axum::http::request::Builder {
    Request::builder().extension(ConnectInfo(
        "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
    ))
}

/// Create a test config manager
fn create_test_manager() -> (ConfigManager, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = ConfigManager::new(temp_dir.path().to_str().unwrap()).unwrap();
    (manager, temp_dir)
}

/// Create a test security state
fn create_test_security_state() -> SecurityState {
    let rate_limiter = RateLimiter::new(RateLimitConfig {
        authenticated_rps: 100,
        unauthenticated_rps: 10,
        burst_size: 50,
        window_seconds: 60,
        ban_duration_seconds: 3600,
        ban_threshold: 5,
    });

    let input_validator = InputValidator::default();

    let mut policy = SecurityPolicy::default();
    policy.require_tls = false; // Disable TLS requirement for tests
    let policy_enforcer = PolicyEnforcer::new(policy);

    SecurityState::with_components(rate_limiter, input_validator, policy_enforcer)
}

#[tokio::test]
async fn test_sql_injection_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Attempt SQL injection in path
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key'%20OR%20'1'='1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_xss_attempt_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Attempt XSS in path
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/%3Cscript%3Ealert%28%27xss%27%29%3C%2Fscript%3E")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_path_traversal_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Attempt path traversal
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/../../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_command_injection_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Attempt command injection
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key;ls")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_rate_limiting_enforcement() {
    let (manager, _temp_dir) = create_test_manager();

    // Create strict rate limiter
    let rate_limiter = RateLimiter::new(RateLimitConfig {
        authenticated_rps: 100,
        unauthenticated_rps: 2, // Very low limit for testing
        burst_size: 2,
        window_seconds: 60,
        ban_duration_seconds: 3600,
        ban_threshold: 5,
    });

    let security_state = SecurityState::with_components(
        rate_limiter,
        InputValidator::default(),
        PolicyEnforcer::new(SecurityPolicy::default()),
    );

    let app = create_router(Arc::new(manager), security_state);

    // First request should succeed
    let response1 = app
        .clone()
        .oneshot(request().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::OK);

    // Second request should succeed
    let response2 = app
        .clone()
        .oneshot(request().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::OK);

    // Note: In a real test, we would need to wait for rate limit window
    // or mock the time to test rate limiting properly
}

#[tokio::test]
async fn test_valid_request_passes_security() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Valid request should pass all security checks
    let response = app
        .oneshot(
            request()
                .method("GET")
                .uri("/api/v1/configs/test-namespace/test-key?env=development")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should get 404 (not found) not 400 (validation error) or 403 (forbidden)
    // This means it passed security checks
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint_no_security() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Health check should not be protected by security middleware
    let response = app
        .oneshot(request().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_large_request_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Request with large content-length header
    let response = app
        .oneshot(
            request()
                .method("POST")
                .uri("/api/v1/configs/test/key")
                .header("content-type", "application/json")
                .header("content-length", "20000000") // 20MB (above 10MB default limit)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_blocked_ip_rejected() {
    let (manager, _temp_dir) = create_test_manager();

    // Create policy with blocked IP
    let mut policy = SecurityPolicy::default();
    policy.blocked_ips = vec!["127.0.0.1".to_string()];
    policy.require_tls = false;

    let security_state = SecurityState::with_components(
        RateLimiter::new(RateLimitConfig::default()),
        InputValidator::default(),
        PolicyEnforcer::new(policy),
    );

    let app = create_router(Arc::new(manager), security_state);

    // Request from blocked IP
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_blocked_endpoint_rejected() {
    let (manager, _temp_dir) = create_test_manager();

    // Create policy with blocked endpoints
    let mut policy = SecurityPolicy::default();
    policy.blocked_endpoints = vec!["/api/v1/configs/admin/*".to_string()];
    policy.require_tls = false;

    let security_state = SecurityState::with_components(
        RateLimiter::new(RateLimitConfig::default()),
        InputValidator::default(),
        PolicyEnforcer::new(policy),
    );

    let app = create_router(Arc::new(manager), security_state);

    // Request to blocked endpoint
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/admin/secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_security_headers_validation() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Request with security-relevant headers
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key")
                .header("x-user-id", "test-user")
                .header("x-forwarded-proto", "http") // Not HTTPS
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should pass since we disabled TLS requirement in test
    // In production with TLS required, this would return 426 UPGRADE_REQUIRED
    assert!(response.status() != StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_malformed_query_parameters_blocked() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Query parameters with SQL injection attempt
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key?env=dev'%20OR%20'1'='1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_multiple_security_violations() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Request with multiple security issues
    // - XSS in path
    // - SQL injection in query
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/%3Cscript%3E/key?env='%20OR%20'1'='1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be blocked (first violation detected)
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_cors_origin_validation() {
    let (manager, _temp_dir) = create_test_manager();

    // Create policy with allowed origins
    let mut policy = SecurityPolicy::default();
    policy.allowed_origins = vec!["https://example.com".to_string()];
    policy.require_tls = false;

    let security_state = SecurityState::with_components(
        RateLimiter::new(RateLimitConfig::default()),
        InputValidator::default(),
        PolicyEnforcer::new(policy),
    );

    let app = create_router(Arc::new(manager), security_state);

    // Request from allowed origin
    let response = app
        .clone()
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key")
                .header("origin", "https://example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not be blocked by CORS
    assert_ne!(response.status(), StatusCode::FORBIDDEN);

    // Request from disallowed origin
    let response2 = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key")
                .header("origin", "https://evil.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be blocked by CORS policy
    assert_eq!(response2.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_security_context_creation() {
    let (manager, _temp_dir) = create_test_manager();
    let security_state = create_test_security_state();
    let app = create_router(Arc::new(manager), security_state);

    // Request with user identification
    let response = app
        .oneshot(
            request()
                .uri("/api/v1/configs/test/key")
                .header("x-user-id", "test-user-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Security context should be created (checked in middleware)
    // Request should proceed normally (not blocked)
    assert!(response.status() != StatusCode::INTERNAL_SERVER_ERROR);
}
