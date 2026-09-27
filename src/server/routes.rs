use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::handlers::*;
use super::static_files::static_handler;

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/executions", get(list_executions_handler))
        .route("/executions/:id", get(get_execution_detail_handler))
        .route("/sessions", get(list_sessions_handler))
        .route("/sessions/:id", get(get_session_detail_handler))
        .route("/stats", get(get_stats_handler))
        .route("/conflicts", get(list_conflicts_handler))
        .route("/events", post(ingest_events_handler))
        .route("/demo/seed", post(seed_demo_handler))
        .route("/shutdown", post(shutdown_handler));

    Router::new()
        .nest("/api/v1", api_router)
        .fallback(static_handler)
        .layer(cors)
        .with_state(state)
}
