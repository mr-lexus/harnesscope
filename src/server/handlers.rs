use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::domain::events::IngestPayload;
use crate::domain::models::*;
use crate::server::correlation::CorrelationEngine;
use crate::storage::repository::{ExecutionFilter, SessionFilter};
use crate::storage::Repository;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repository>,
    pub engine: Arc<CorrelationEngine>,
    pub shutdown_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Serialize)]
pub struct ExecutionDetailResponse {
    pub execution: Execution,
    pub runtime: Option<RuntimeInstance>,
    pub session: Option<Session>,
    pub agents: Vec<AgentInstance>,
    pub components: Vec<ExecutionComponent>,
    pub git_snapshots: Vec<GitSnapshot>,
    pub events: Vec<Event>,
}

#[derive(Serialize)]
pub struct SessionDetailResponse {
    pub session: Session,
    pub bindings: Vec<RuntimeSessionBinding>,
    pub executions: Vec<Execution>,
    pub events: Vec<Event>,
    pub child_forks: Vec<Session>,
    pub conflicts: Vec<SessionConflict>,
}

#[derive(Serialize)]
pub struct IngestResult {
    pub accepted: usize,
    pub processed: usize,
    pub event_ids: Vec<String>,
}

pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.repo.get_health_metrics() {
        Ok(metrics) => (StatusCode::OK, Json(serde_json::to_value(metrics).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string(),
                "db_connected": false
            })),
        ),
    }
}

pub async fn ingest_events_handler(
    State(state): State<AppState>,
    Json(payload): Json<IngestPayload>,
) -> impl IntoResponse {
    let events = payload.into_events();
    let count = events.len();
    let mut event_ids = Vec::with_capacity(count);

    for ev in &events {
        match state.engine.process_event(ev) {
            Ok(id) => event_ids.push(id),
            Err(e) => {
                tracing::error!("Failed to process event {}: {:?}", ev.event_id, e);
            }
        }
    }

    (
        StatusCode::ACCEPTED,
        Json(IngestResult {
            accepted: count,
            processed: event_ids.len(),
            event_ids,
        }),
    )
}

pub async fn list_executions_handler(
    State(state): State<AppState>,
    Query(filter): Query<ExecutionFilter>,
) -> impl IntoResponse {
    let page = filter.page.unwrap_or(1).max(1);
    let page_size = filter.page_size.unwrap_or(20).clamp(1, 100);

    match state.repo.list_executions(&filter) {
        Ok((items, total)) => {
            let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PaginatedResponse {
                    items,
                    total,
                    page,
                    page_size,
                    total_pages,
                }).unwrap()),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn get_execution_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo.find_execution_by_id(&id) {
        Ok(Some(exec)) => {
            let runtime = state.repo.find_runtime_by_id(&exec.runtime_id).ok().flatten();
            let session = state.repo.find_session_by_id(&exec.session_id).ok().flatten();
            let agents = state.repo.list_agent_instances_for_execution(&id).unwrap_or_default();
            let components = state.repo.list_components_for_execution(&id).unwrap_or_default();
            let git_snapshots = state.repo.list_git_snapshots_for_execution(&id).unwrap_or_default();
            let events = state.repo.list_events_for_execution(&id).unwrap_or_default();

            (
                StatusCode::OK,
                Json(serde_json::to_value(ExecutionDetailResponse {
                    execution: exec,
                    runtime,
                    session,
                    agents,
                    components,
                    git_snapshots,
                    events,
                }).unwrap()),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Execution not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn list_sessions_handler(
    State(state): State<AppState>,
    Query(filter): Query<SessionFilter>,
) -> impl IntoResponse {
    let page = filter.page.unwrap_or(1).max(1);
    let page_size = filter.page_size.unwrap_or(20).clamp(1, 100);

    match state.repo.list_sessions(&filter) {
        Ok((items, total)) => {
            let total_pages = ((total as f64) / (page_size as f64)).ceil() as u32;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PaginatedResponse {
                    items,
                    total,
                    page,
                    page_size,
                    total_pages,
                }).unwrap()),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn get_session_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo.find_session_by_id(&id) {
        Ok(Some(session)) => {
            let bindings = state.repo.list_bindings_for_session(&id).unwrap_or_default();
            let executions = state.repo.list_executions_for_session(&id).unwrap_or_default();
            let events = state.repo.list_events_for_session(&id).unwrap_or_default();
            let child_forks = state.repo.list_child_forks(&id).unwrap_or_default();
            let conflicts = state.repo.list_conflicts_for_session(&id).unwrap_or_default();

            (
                StatusCode::OK,
                Json(serde_json::to_value(SessionDetailResponse {
                    session,
                    bindings,
                    executions,
                    events,
                    child_forks,
                    conflicts,
                }).unwrap()),
            )
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn list_conflicts_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.repo.list_all_conflicts() {
        Ok(conflicts) => (StatusCode::OK, Json(serde_json::to_value(conflicts).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn get_stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.repo.get_stats() {
        Ok(stats) => (StatusCode::OK, Json(serde_json::to_value(stats).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn seed_demo_handler(State(state): State<AppState>) -> impl IntoResponse {
    match crate::demo::seed_demo_data(&state.repo) {
        Ok(msg) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "message": msg }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn shutdown_handler(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(tx) = &state.shutdown_tx {
        let _ = tx.send(()).await;
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "shutting down" })),
    )
}
