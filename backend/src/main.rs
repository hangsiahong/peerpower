mod config;
mod domain;
mod infrastructure;
mod presentation;
mod shared;

use axum::{
    extract::State,
    http::StatusCode,
    middleware,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::presentation::handlers::{
    admin_handlers, auth_handlers, earnings_handlers, message_handlers, provider_handlers,
    user_handlers,
};
use crate::presentation::middleware::auth_middleware;

use crate::config::AppConfig;
use crate::shared::{AppState, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    init_tracing();

    // Load configuration
    let config = AppConfig::from_env()?;
    let bind_addr = config.bind_address();

    tracing::info!("Starting PeerPower Backend");
    tracing::info!("Environment: {:?}", config.server.environment);
    tracing::info!("Instance ID: {}", config.instance.id);
    tracing::info!("Region: {}", config.instance.region);

    // Build the application
    let app = build_app(config).await?;

    // Start the server
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| shared::PeerPowerError::Configuration {
            message: format!("Failed to bind to {}: {}", bind_addr, e),
        })?;

    tracing::info!("Server listening on {}", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| shared::PeerPowerError::Internal {
            message: format!("Server error: {}", e),
        })?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

async fn build_app(config: AppConfig) -> Result<Router> {
    // Create shared application state with database connections
    let app_state = Arc::new(AppState::new(config).await?);

    // Auth routes (public)
    let auth_routes = Router::new()
        .route("/send-otp", post(auth_handlers::send_otp))
        .route("/verify-otp", post(auth_handlers::verify_otp))
        .route("/refresh", post(auth_handlers::refresh_token))
        .route("/logout", post(auth_handlers::logout));

    // Protected API routes (require authentication)
    let protected_routes = Router::new()
        .route("/users/profile", get(user_handlers::get_user_profile))
        .route("/users/profile", put(user_handlers::update_user_profile))
        .route(
            "/providers/register",
            post(provider_handlers::register_provider),
        )
        .route("/providers", get(provider_handlers::list_providers))
        .route(
            "/providers/:id",
            get(provider_handlers::get_provider_status),
        )
        .route(
            "/providers/:id/heartbeat",
            post(provider_handlers::provider_heartbeat),
        )
        .route(
            "/providers/:id/status",
            put(provider_handlers::update_provider_status),
        )
        .route("/messages/send", post(message_handlers::send_message))
        .route("/messages/:id", get(message_handlers::get_message_status))
        .route("/messages", get(message_handlers::list_messages))
        .route(
            "/messages/:message_id/delivery",
            post(message_handlers::confirm_delivery),
        )
        .route(
            "/earnings/summary",
            get(earnings_handlers::get_provider_earnings),
        )
        .route(
            "/earnings/history",
            get(earnings_handlers::get_earnings_history),
        )
        .route(
            "/earnings/stats",
            get(earnings_handlers::get_system_earnings_stats),
        )
        .route("/admin/stats", get(admin_handlers::get_system_stats))
        .route(
            "/admin/providers",
            get(admin_handlers::get_provider_performance),
        )
        .route(
            "/admin/messages",
            get(admin_handlers::get_message_analytics),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware::auth_middleware::<axum::body::Body>,
        ));

    // Public webhook routes (no authentication required)
    let webhook_routes = Router::new().route(
        "/webhooks/delivery/:message_id",
        post(message_handlers::delivery_webhook),
    );

    // API v1 routes
    let api_v1 = Router::new()
        .nest("/auth", auth_routes)
        .nest("/", protected_routes)
        .nest("/", webhook_routes);

    // Build the main router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/", get(root_handler))
        .nest("/api/v1", api_v1)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()), // TODO: Configure CORS properly for production
        )
        .with_state(app_state.clone());

    // Start the job processor
    let mut job_processor = crate::infrastructure::JobProcessor::new(app_state.clone());
    tokio::spawn(async move {
        if let Err(e) = job_processor.start().await {
            tracing::error!("Failed to start job processor: {}", e);
        }
    });

    Ok(app)
}

/// Health check endpoint - always returns healthy if the service is running
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "peerpower-backend",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Readiness check endpoint - checks if the service is ready to accept traffic
async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Check database connectivity
    let db_status = match state.database.health_check().await {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    // Check Redis connectivity
    let redis_status = match state.redis.health_check().await {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    let overall_status = if db_status == "ok" && redis_status == "ok" {
        "ready"
    } else {
        "not_ready"
    };

    let response = Json(json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "peerpower-backend",
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": db_status,
            "redis": redis_status,
            "external_services": "ok"  // TODO: Check FCM, Baray, etc.
        }
    }));

    if overall_status == "ready" {
        Ok(response)
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Root handler - basic API information
async fn root_handler() -> Json<Value> {
    Json(json!({
        "service": "PeerPower Backend API",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Decentralized SMS delivery network for Cambodia",
        "endpoints": {
            "health": "/health",
            "ready": "/ready",
            "api_v1": "/api/v1"
        },
        "documentation": "https://docs.peerpower.network"
    }))
}

/// Initialize tracing/logging
fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Default log level based on environment
        if cfg!(debug_assertions) {
            "debug,hyper=info,tower=info".into()
        } else {
            "info".into()
        }
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(true),
        )
        .init();
}

/// Placeholder handler for user profile (protected route)
async fn user_profile_handler() -> Json<Value> {
    // TODO: Implement user profile retrieval
    Json(json!({
        "message": "User profile endpoint - TODO: implement",
        "status": "placeholder"
    }))
}

/// Placeholder handler for provider registration (protected route)
async fn provider_register_handler() -> Json<Value> {
    // TODO: Implement provider registration
    Json(json!({
        "message": "Provider registration endpoint - TODO: implement",
        "status": "placeholder"
    }))
}

/// Placeholder handler for sending messages (protected route)
async fn send_message_handler() -> Json<Value> {
    // TODO: Implement message sending
    Json(json!({
        "message": "Send message endpoint - TODO: implement",
        "status": "placeholder"
    }))
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, starting graceful shutdown");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown");
        },
    }
}
