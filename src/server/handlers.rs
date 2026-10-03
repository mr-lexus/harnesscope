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
use crate::storage::retrospective::{RetrospectiveFilter, Review};
use crate::storage::Repository;

pub async fn retrospective_handler(
    State(state): State<AppState>,
    Query(filter): Query<RetrospectiveFilter>,
) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || state.repo.retrospective(&filter)).await {
        Ok(Ok(report)) => (StatusCode::OK, Json(serde_json::to_value(report).unwrap())),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Unable to load retrospective"})),
        ),
    }
}

pub async fn get_review_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.repo.find_execution_by_id(&id) {
        Ok(Some(_)) => match state.repo.get_review(&id) {
            Ok(review) => (StatusCode::OK, Json(serde_json::to_value(review).unwrap())),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"Unable to read review"})),
            ),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"Execution not found"})),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Unable to read execution"})),
        ),
    }
}

pub async fn save_review_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(review): Json<Review>,
) -> impl IntoResponse {
    if let Err(message) = review.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":message})),
        );
    }
    match state.repo.find_execution_by_id(&id) {
        Ok(Some(_)) => match state.repo.save_review(&id, &review) {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status":"saved"}))),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"Unable to save review"})),
            ),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"Execution not found"})),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Unable to read execution"})),
        ),
    }
}

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<Repository>,
    pub engine: Arc<CorrelationEngine>,
    pub outbox: Option<crate::outbox::Outbox>,
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
    pub usage: Option<crate::storage::usage::TokenUsage>,
    pub execution: Execution,
    pub runtime: Option<RuntimeInstance>,
    pub session: Option<Session>,
    pub agents: Vec<AgentInstance>,
    pub components: Vec<ExecutionComponent>,
    pub git_snapshots: Vec<GitSnapshot>,
    pub events_total: u64,
}

#[derive(Serialize)]
pub struct SessionDetailResponse {
    pub session: Session,
    pub bindings: Vec<RuntimeSessionBinding>,
    pub executions: Vec<Execution>,
    pub events_total: u64,
    pub child_forks: Vec<Session>,
    pub conflicts: Vec<SessionConflict>,
}

#[derive(Serialize)]
pub struct IngestResult {
    pub accepted: usize,
    pub processed: usize,
    pub event_ids: Vec<String>,
    pub errors: Vec<IngestError>,
}

#[derive(Serialize)]
pub struct IngestError {
    pub event_id: String,
    pub message: String,
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
    if count == 0 || count > 500 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Send between 1 and 500 events"})),
        );
    }
    for event in &events {
        if let Err(message) = event.validate() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": message, "event_id": event.event_id})),
            );
        }
    }
    let result = tokio::task::spawn_blocking(move || {
        let mut event_ids = Vec::with_capacity(count);
        let mut errors = Vec::new();

        for ev in &events {
            match state.engine.process_event(ev) {
                Ok(id) => event_ids.push(id),
                Err(e) => {
                    tracing::error!("Failed to process event {}: {:?}", ev.event_id, e);
                    errors.push(IngestError {
                        event_id: ev.event_id.clone(),
                        message: "Event could not be applied; verify references and event order"
                            .into(),
                    });
                }
            }
        }

        (
            if errors.is_empty() {
                StatusCode::ACCEPTED
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            },
            Json(
                serde_json::to_value(IngestResult {
                    accepted: event_ids.len(),
                    processed: event_ids.len(),
                    event_ids,
                    errors,
                })
                .unwrap(),
            ),
        )
    })
    .await;
    result.unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Ingestion worker failed"})),
        )
    })
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
                Json(
                    serde_json::to_value(PaginatedResponse {
                        items,
                        total,
                        page,
                        page_size,
                        total_pages,
                    })
                    .unwrap(),
                ),
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
    tokio::task::spawn_blocking(move || match state.repo.find_execution_by_id(&id) {
        Ok(Some(exec)) => {
            let detail = (|| -> rusqlite::Result<ExecutionDetailResponse> {
                Ok(ExecutionDetailResponse {
                    usage: state.repo.get_usage(&id)?,
                    runtime: state.repo.find_runtime_by_id(&exec.runtime_id)?,
                    session: state.repo.find_session_by_id(&exec.session_id)?,
                    agents: state.repo.list_agent_instances_for_execution(&id)?,
                    components: state.repo.list_components_for_execution(&id)?,
                    git_snapshots: state.repo.list_git_snapshots_for_execution(&id)?,
                    events_total: state
                        .repo
                        .count_events(crate::storage::event_pages::EventOwner::Execution, &id)?,
                    execution: exec,
                })
            })();
            match detail {
                Ok(detail) => (StatusCode::OK, Json(serde_json::to_value(detail).unwrap())),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error":"Unable to load complete execution detail"})),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Execution not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    })
    .await
    .unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Detail worker failed"})),
        )
    })
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
                Json(
                    serde_json::to_value(PaginatedResponse {
                        items,
                        total,
                        page,
                        page_size,
                        total_pages,
                    })
                    .unwrap(),
                ),
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
    tokio::task::spawn_blocking(move || match state.repo.find_session_by_id(&id) {
        Ok(Some(session)) => {
            let detail = (|| -> rusqlite::Result<SessionDetailResponse> {
                Ok(SessionDetailResponse {
                    bindings: state.repo.list_bindings_for_session(&id)?,
                    executions: state.repo.list_executions_for_session(&id)?,
                    events_total: state
                        .repo
                        .count_events(crate::storage::event_pages::EventOwner::Session, &id)?,
                    child_forks: state.repo.list_child_forks(&id)?,
                    conflicts: state.repo.list_conflicts_for_session(&id)?,
                    session,
                })
            })();
            match detail {
                Ok(detail) => (StatusCode::OK, Json(serde_json::to_value(detail).unwrap())),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error":"Unable to load complete session detail"})),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Session not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    })
    .await
    .unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Detail worker failed"})),
        )
    })
}

pub async fn list_conflicts_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.repo.list_all_conflicts() {
        Ok(conflicts) => (
            StatusCode::OK,
            Json(serde_json::to_value(conflicts).unwrap()),
        ),
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
        Ok(msg) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "message": msg })),
        ),
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

pub async fn execution_events_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<crate::storage::event_pages::EventQuery>,
) -> impl IntoResponse {
    events_handler(
        state,
        id,
        query,
        crate::storage::event_pages::EventOwner::Execution,
    )
    .await
}
pub async fn session_events_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<crate::storage::event_pages::EventQuery>,
) -> impl IntoResponse {
    events_handler(
        state,
        id,
        query,
        crate::storage::event_pages::EventOwner::Session,
    )
    .await
}
async fn events_handler(
    state: AppState,
    id: String,
    query: crate::storage::event_pages::EventQuery,
    owner: crate::storage::event_pages::EventOwner,
) -> (StatusCode, Json<serde_json::Value>) {
    use crate::storage::event_pages::EventOwner;
    if query.before_id.is_some_and(|id| id <= 0)
        || query.search.as_ref().is_some_and(|s| s.len() > 200)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"error":"Use a positive cursor and a search of at most 200 bytes"}),
            ),
        );
    }
    tokio::task::spawn_blocking(move || {
        let exists = match owner {
            EventOwner::Execution => state.repo.find_execution_by_id(&id).map(|v| v.is_some()),
            EventOwner::Session => state.repo.find_session_by_id(&id).map(|v| v.is_some()),
        };
        match exists {
            Ok(false) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error":"Event owner not found"})),
            ),
            Ok(true) => match state.repo.event_page(owner, &id, &query) {
                Ok(page) => (StatusCode::OK, Json(serde_json::to_value(page).unwrap())),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error":"Unable to load events"})),
                ),
            },
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"Unable to load event owner"})),
            ),
        }
    })
    .await
    .unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Event worker failed"})),
        )
    })
}
