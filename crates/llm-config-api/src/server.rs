//! HTTP server implementation

use crate::middleware::{comprehensive_security_middleware, SecurityState};
use crate::routes::{
    delete_config, execute_get_config, execute_set_config, get_config, get_history, health_check,
    list_configs, rollback_config, set_config, ApiState,
};
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use llm_config_core::ConfigManager;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_cors: bool,
    pub enable_security: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            enable_cors: true,
            enable_security: true,
        }
    }
}

/// Create and configure the Axum router
pub fn create_router(manager: Arc<ConfigManager>, security_state: SecurityState) -> Router {
    let api_state = ApiState { manager };

    // API v1 routes with security middleware
    let api_routes = Router::new()
        // Config operations
        .route("/configs/:namespace/:key", get(get_config))
        .route("/configs/:namespace/:key", post(set_config))
        .route("/configs/:namespace/:key", delete(delete_config))
        .route("/configs/:namespace", get(list_configs))
        // Version history and rollback
        .route("/configs/:namespace/:key/history", get(get_history))
        .route(
            "/configs/:namespace/:key/rollback/:version",
            post(rollback_config),
        )
        // Instrumented execution endpoints (require X-Parent-Span-Id header)
        .route(
            "/execution/configs/:namespace/:key",
            get(execute_get_config),
        )
        .route(
            "/execution/configs/:namespace/:key",
            post(execute_set_config),
        )
        .layer(middleware::from_fn_with_state(
            security_state.clone(),
            comprehensive_security_middleware,
        ))
        .with_state(api_state);

    // Main router with health check (no security on health endpoint)
    Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1", api_routes)
}

/// Start the HTTP server
pub async fn serve(
    manager: Arc<ConfigManager>,
    config: ServerConfig,
) -> anyhow::Result<()> {
    // Create security state
    let security_state = if config.enable_security {
        SecurityState::new()
    } else {
        SecurityState::new() // Always create but can be configured differently
    };

    let app = create_router(manager, security_state);

    // Add middleware layers
    let app = app
        .layer(TraceLayer::new_for_http())
        .layer(if config.enable_cors {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            CorsLayer::permissive()
        });

    // Bind to address
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!(
        "Starting LLM Config API server on {} (security: {})",
        addr,
        config.enable_security
    );

    // Create listener
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Serve with graceful shutdown
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

/// Graceful shutdown handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down gracefully");
        },
    }
}

// Integration tests will be added in a separate test file
// For now, the API can be tested manually using curl or similar tools
