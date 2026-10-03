use super::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
type Response = (StatusCode, Json<Value>);

pub async fn delivery(
    State(state): State<AppState>,
    Json(events): Json<Vec<crate::domain::events::IngestEvent>>,
) -> Response {
    if events.is_empty() || events.len() > 500 || events.iter().any(|e| e.validate().is_err()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Invalid delivery batch"})),
        );
    }
    let ids: Vec<_> = events.iter().map(|e| e.event_id.clone()).collect();
    match tokio::task::spawn_blocking(move || state.engine.process_events_with(&events, || Ok(())))
        .await
    {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"event_ids":ids}))),
        Ok(Err(rusqlite::Error::InvalidParameterName(_))) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error":"Invalid event relationship"})),
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Delivery not committed; retry"})),
        ),
    }
}
pub async fn status(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let pending = state
            .outbox
            .as_ref()
            .map(|q| q.snapshot())
            .transpose()?
            .unwrap_or_default();
        Ok(
            json!({"runtimes":state.repo.observations().map_err(|e| e.to_string())?,
            "queue_available":state.outbox.is_some(),"pending_events":pending.pending_events,
            "pending_batches":pending.pending_batches,"blocked_batches":pending.blocked_batches,
            "batches":pending.batches,"heartbeat_seconds":15,"stale_after_seconds":45}),
        )
    })
    .await
    {
        Ok(Ok(value)) => (StatusCode::OK, Json(value)),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Monitoring unavailable"})),
        ),
    }
}
pub async fn retry(State(state): State<AppState>) -> Response {
    match tokio::task::spawn_blocking(move || {
        state
            .outbox
            .ok_or("Outbox unavailable".to_string())?
            .retry()
    })
    .await
    {
        Ok(Ok(())) => (StatusCode::OK, Json(json!({"status":"scheduled"}))),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Unable to retry delivery"})),
        ),
    }
}
