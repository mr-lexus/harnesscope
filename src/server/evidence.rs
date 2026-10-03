use super::handlers::AppState;
use crate::{capture, storage::evidence::EvidenceFilter};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
type Response = (StatusCode, Json<Value>);
async fn run(f: impl FnOnce() -> rusqlite::Result<Value> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(value)) => (StatusCode::OK, Json(value)),
        _ => (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error":"Evidence operation failed; inspect source diagnostics or request parameters"}),
            ),
        ),
    }
}
pub async fn page(State(s): State<AppState>, Query(f): Query<EvidenceFilter>) -> Response {
    run(move || s.repo.evidence_page(&f)).await
}
pub async fn coverage(State(s): State<AppState>) -> Response {
    run(move || s.repo.evidence_coverage()).await
}
pub async fn context(State(s): State<AppState>, Query(f): Query<EvidenceFilter>) -> Response {
    run(move || s.repo.context_page(&f)).await
}
pub async fn export(
    State(s): State<AppState>,
    Query(f): Query<EvidenceFilter>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let result = tokio::task::spawn_blocking(move || -> Result<tempfile::TempDir, String> {
        let dir = tempfile::tempdir().map_err(|_| "Cannot stage export")?;
        s.repo
            .export_evidence(&f, &dir.path().join("retrospective.jsonl"))?;
        Ok(dir)
    })
    .await;
    let dir = match result {
        Ok(Ok(dir)) => dir,
        _ => return (StatusCode::SERVICE_UNAVAILABLE, "Cannot prepare export").into_response(),
    };
    let file = match tokio::fs::File::open(dir.path().join("retrospective.jsonl")).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "Cannot read export").into_response(),
    };
    let stream = futures_util::stream::try_unfold((file, dir), |(mut file, dir)| async move {
        use tokio::io::AsyncReadExt;
        let mut bytes = vec![0u8; 64 * 1024];
        let n = file.read(&mut bytes).await?;
        if n == 0 {
            Ok::<_, std::io::Error>(None)
        } else {
            bytes.truncate(n);
            Ok(Some((bytes, (file, dir))))
        }
    });
    (
        [
            ("content-type", "application/x-ndjson"),
            (
                "content-disposition",
                "attachment; filename=retrospective.jsonl",
            ),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}
pub async fn object(State(s): State<AppState>, Path(hash): Path<String>) -> Response {
    match tokio::task::spawn_blocking(move || s.repo.evidence_object(&hash)).await {
        Ok(Ok(Some(value))) => (StatusCode::OK, Json(value)),
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"Object not found"})),
        ),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Object unavailable or corrupt"})),
        ),
    }
}
#[derive(Default, Deserialize)]
pub struct ObjectQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}
pub async fn object_page(
    State(s): State<AppState>,
    Path(hash): Path<String>,
    Query(q): Query<ObjectQuery>,
) -> Response {
    run(move || {
        s.repo
            .evidence_object_page(&hash, q.offset.unwrap_or(0), q.limit.unwrap_or(16000))
    })
    .await
}
#[derive(Deserialize)]
pub struct DiffQuery {
    before: String,
    after: String,
}
pub async fn diff(State(s): State<AppState>, Query(q): Query<DiffQuery>) -> Response {
    run(move || s.repo.evidence_diff(&q.before, &q.after)).await
}
#[derive(Default, Deserialize)]
pub struct TaskQuery {
    after: Option<String>,
    limit: Option<u32>,
}
pub async fn tasks(State(s): State<AppState>, Query(q): Query<TaskQuery>) -> Response {
    run(move || {
        s.repo
            .retro_tasks(q.after.as_deref().unwrap_or(""), q.limit.unwrap_or(50))
    })
    .await
}
pub async fn task(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    run(move || s.repo.retro_task_detail(&id)).await
}
#[derive(Deserialize)]
pub struct CreateTask {
    title: String,
    project: Option<String>,
}
pub async fn create_task(State(s): State<AppState>, Json(v): Json<CreateTask>) -> Response {
    run(move || {
        s.repo
            .create_retro_task(&v.title, v.project.as_deref())
            .map(|id| json!({"id":id}))
    })
    .await
}
#[derive(Deserialize)]
pub struct LinkTask {
    session_id: String,
    turn_id: Option<String>,
    confirmed: bool,
}
pub async fn link_task(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<LinkTask>,
) -> Response {
    run(move || {
        s.repo
            .link_retro_task(&id, &v.session_id, v.turn_id.as_deref(), v.confirmed)
            .map(|_| json!({"saved":true}))
    })
    .await
}
#[derive(Deserialize)]
pub struct MarkTask {
    kind: String,
    #[serde(default)]
    note: String,
}
pub async fn mark_task(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(v): Json<MarkTask>,
) -> Response {
    run(move || {
        s.repo
            .mark_retro_task(&id, &v.kind, &v.note)
            .map(|_| json!({"saved":true}))
    })
    .await
}
#[derive(Default, Deserialize)]
pub struct WorkflowQuery {
    root: Option<String>,
    after: Option<i64>,
}
pub async fn workflows(State(s): State<AppState>, Query(v): Query<WorkflowQuery>) -> Response {
    run(move || {
        s.repo
            .workflow_versions(v.root.as_deref(), v.after.unwrap_or(0))
    })
    .await
}
#[derive(Deserialize)]
pub struct Root {
    path: String,
}
pub async fn add_workflow(State(s): State<AppState>, Json(v): Json<Root>) -> Response {
    run(move || {
        s.repo
            .register_workflow(std::path::Path::new(&v.path))
            .map(|_| json!({"saved":true}))
            .map_err(rusqlite::Error::InvalidParameterName)
    })
    .await
}
pub async fn hook(State(s): State<AppState>, Json(v): Json<Value>) -> Response {
    run(move || {
        let id = uuid::Uuid::new_v4().to_string();
        let input = capture::from_record("codex_hook", "http", &id, &v);
        s.repo
            .record_observation(&input, &capture::sanitize(&v))
            .map(|id| json!({"id":id}))
    })
    .await
}
pub async fn otlp(State(s): State<AppState>, Json(v): Json<Value>) -> Response {
    if v.get("resourceLogs").is_none() && v.get("resourceSpans").is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Expected OTLP JSON resourceLogs or resourceSpans"})),
        );
    }
    run(move || {
        s.repo.ingest_otlp(&v)?;
        Ok(json!({}))
    })
    .await
}
