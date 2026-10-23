//! Frozen live-TCP acceptance contract. Run unchanged on baseline and candidate.
use axum::Router;
use llm_config_api::{create_router, SecurityState};
use llm_config_core::ConfigManager;
use llm_config_security::{
    InputValidator, PolicyEnforcer, RateLimitConfig, RateLimiter, SecurityPolicy,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn policy() -> SecurityPolicy {
    SecurityPolicy {
        require_tls: false,
        allowed_origins: vec!["https://example.com".into()],
        max_request_size: 256,
        ..SecurityPolicy::default()
    }
}

async fn status(policy: SecurityPolicy, request: Vec<u8>) -> u16 {
    let dir = tempfile::tempdir().unwrap();
    let manager = Arc::new(ConfigManager::new(dir.path().to_str().unwrap()).unwrap());
    let state = SecurityState::with_components(
        RateLimiter::new(RateLimitConfig::default()),
        InputValidator::default(),
        PolicyEnforcer::new(policy),
    );
    let app: Router = create_router(manager, state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream.write_all(&request).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    })
    .await;
    server.abort();
    let _ = server.await;
    let response = response.expect("HTTP request deadline");
    let line = std::str::from_utf8(response.split(|b| *b == b'\n').next().unwrap()).unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

fn get(path: &str, headers: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{headers}\r\n")
        .into_bytes()
}

#[tokio::test]
async fn disallowed_origin() {
    assert_eq!(
        status(
            policy(),
            get("/api/v1/configs/test/key", "Origin: https://evil.com\r\n")
        )
        .await,
        403
    );
}

#[tokio::test]
async fn oversized_declared_body() {
    let body = "x".repeat(300);
    let request = format!("POST /api/v1/configs/test/key HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    assert_eq!(status(policy(), request.into_bytes()).await, 413);
}

#[tokio::test]
async fn oversized_chunked_body() {
    let body = "x".repeat(300);
    let request = format!("POST /api/v1/configs/test/key HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len());
    assert_eq!(status(policy(), request.into_bytes()).await, 413);
}

#[tokio::test]
async fn invalid_origin_bytes() {
    let mut request = get(
        "/api/v1/configs/test/key",
        "Origin: https://example.com\r\n",
    );
    let offset = request.windows(7).position(|w| w == b"example").unwrap();
    request[offset] = 0xff;
    assert_eq!(status(policy(), request).await, 403);
}

#[tokio::test]
async fn health() {
    assert_eq!(status(policy(), get("/health", "")).await, 200);
}

#[tokio::test]
async fn allowed_origin() {
    assert_eq!(
        status(
            policy(),
            get(
                "/api/v1/configs/test/key",
                "Origin: https://example.com\r\n"
            )
        )
        .await,
        404
    );
}

#[tokio::test]
async fn originless_request() {
    assert_eq!(
        status(policy(), get("/api/v1/configs/test/key", "")).await,
        404
    );
}

#[tokio::test]
async fn valid_small_body() {
    let body = r#"{"value":"value","env":"development"}"#;
    let request = format!("POST /api/v1/configs/test/key HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    assert_eq!(status(policy(), request.into_bytes()).await, 200);
}

#[tokio::test]
async fn blocked_ip() {
    let mut p = policy();
    p.blocked_ips = vec!["127.0.0.1".into()];
    assert_eq!(status(p, get("/api/v1/configs/test/key", "")).await, 403);
}

#[tokio::test]
async fn blocked_endpoint() {
    let mut p = policy();
    p.blocked_endpoints = vec!["/api/v1/configs/admin/*".into()];
    assert_eq!(status(p, get("/api/v1/configs/admin/key", "")).await, 403);
}

#[tokio::test]
async fn tls_required() {
    let mut p = policy();
    p.require_tls = true;
    assert_eq!(status(p, get("/api/v1/configs/test/key", "")).await, 426);
}

// Additional regressions are not part of the frozen four-item optimization score.
#[tokio::test]
async fn repeated_origins_fail_closed() {
    assert_eq!(
        status(
            policy(),
            get(
                "/api/v1/configs/test/key",
                "Origin: https://example.com\r\nOrigin: https://evil.com\r\n"
            )
        )
        .await,
        403
    );
}

#[tokio::test]
async fn encoded_endpoint_cannot_bypass_blocklist() {
    let mut p = policy();
    p.blocked_endpoints = vec!["/api/v1/configs/admin/*".into()];
    assert_eq!(status(p, get("/api/v1/configs/%61dmin/key", "")).await, 403);
}

#[tokio::test]
async fn encoded_script_rejected() {
    assert_eq!(
        status(
            policy(),
            get(
                "/api/v1/configs/test/%3Cscript%3Ealert%281%29%3C%2Fscript%3E",
                ""
            )
        )
        .await,
        400
    );
}

#[tokio::test]
async fn invalid_utf8_path_rejected() {
    assert_eq!(
        status(policy(), get("/api/v1/configs/test/%FF", "")).await,
        400
    );
}

#[tokio::test]
async fn small_chunked_body_preserved() {
    let body = r#"{"value":"value","env":"development"}"#;
    let request = format!("POST /api/v1/configs/test/key HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n", body.len());
    assert_eq!(status(policy(), request.into_bytes()).await, 200);
}
