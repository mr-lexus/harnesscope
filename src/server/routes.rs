use axum::{
    extract::{DefaultBodyLimit, Request},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};

use super::handlers::*;
use super::static_files::static_handler;

pub fn build_router(state: AppState) -> Router {
    let api_router = Router::new()
        .route("/health", get(health_handler))
        .route("/evidence", get(super::evidence::page))
        .route("/evidence/context", get(super::evidence::context))
        .route("/evidence/coverage", get(super::evidence::coverage))
        .route("/evidence/export", get(super::evidence::export))
        .route("/evidence/diff", get(super::evidence::diff))
        .route("/evidence/objects/:hash", get(super::evidence::object))
        .route(
            "/evidence/objects/:hash/pages",
            get(super::evidence::object_page),
        )
        .route(
            "/tasks",
            get(super::evidence::tasks).post(super::evidence::create_task),
        )
        .route("/tasks/:id", get(super::evidence::task))
        .route("/tasks/:id/links", post(super::evidence::link_task))
        .route("/tasks/:id/marks", post(super::evidence::mark_task))
        .route(
            "/workflow",
            get(super::evidence::workflows).post(super::evidence::add_workflow),
        )
        .route("/capture/hooks", post(super::evidence::hook))
        .route("/collection", get(super::sources::collection))
        .route("/monitoring", get(super::monitoring::status))
        .route("/outbox/retry", post(super::monitoring::retry))
        .route("/delivery", post(super::monitoring::delivery))
        .route(
            "/sources",
            get(super::sources::list).post(super::sources::add),
        )
        .route(
            "/sources/:id",
            axum::routing::patch(super::sources::update).delete(super::sources::remove),
        )
        .route("/executions", get(list_executions_handler))
        .route("/executions/:id", get(get_execution_detail_handler))
        .route("/executions/:id/events", get(execution_events_handler))
        .route(
            "/executions/:id/review",
            get(get_review_handler).put(save_review_handler),
        )
        .route("/retrospective", get(retrospective_handler))
        .route("/sessions", get(list_sessions_handler))
        .route("/sessions/:id", get(get_session_detail_handler))
        .route("/sessions/:id/events", get(session_events_handler))
        .route("/stats", get(get_stats_handler))
        .route("/conflicts", get(list_conflicts_handler))
        .route("/events", post(ingest_events_handler))
        .route("/demo/seed", post(seed_demo_handler))
        .route("/shutdown", post(shutdown_handler));

    Router::new()
        .nest("/api/v1", api_router)
        .route("/v1/logs", post(super::evidence::otlp))
        .route("/v1/traces", post(super::evidence::otlp))
        .fallback(static_handler)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn(local_requests_only))
        .with_state(state)
}

async fn local_requests_only(request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    if let Some(host) = host {
        let valid = host
            .parse::<axum::http::uri::Authority>()
            .ok()
            .is_some_and(|a| {
                let name = a.host().trim_matches(['[', ']']);
                name.eq_ignore_ascii_case("localhost")
                    || name
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            });
        if !valid {
            return (StatusCode::FORBIDDEN, "Only loopback hosts are allowed").into_response();
        }
    }
    if let Some(origin) = request.headers().get(header::ORIGIN) {
        let same_origin = origin
            .to_str()
            .ok()
            .and_then(|s| s.parse::<axum::http::Uri>().ok())
            .is_some_and(|uri| {
                uri.scheme_str() == Some("http") && uri.authority().map(|a| a.as_str()) == host
            });
        if !same_origin {
            return (
                StatusCode::FORBIDDEN,
                "Cross-origin requests are not allowed",
            )
                .into_response();
        }
    }
    if request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site")
    {
        return (StatusCode::FORBIDDEN, "Cross-site requests are not allowed").into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert("x-frame-options", "DENY".parse().unwrap());
    response
        .headers_mut()
        .insert("referrer-policy", "no-referrer".parse().unwrap());
    response
}
