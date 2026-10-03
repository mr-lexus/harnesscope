use super::handlers::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || -> rusqlite::Result<_> {
        let sources = state.repo.list_sources()?;
        let mut output = Vec::new();
        for source in sources {
            let files = state.repo.list_source_files(&source.id)?;
            output.push(json!({"source":source,"files":files}));
        }
        Ok(output)
    })
    .await
    {
        Ok(Ok(sources)) => (
            StatusCode::OK,
            Json(json!({"items":sources,"poll_interval_seconds":5})),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Cannot read sources"})),
        ),
    }
}

#[derive(Deserialize)]
pub struct AddSource {
    path: String,
    #[serde(default)]
    include_content: bool,
}

pub async fn add(State(state): State<AppState>, Json(input): Json<AddSource>) -> impl IntoResponse {
    if input.path.len() > 4096 || !std::path::Path::new(&input.path).is_absolute() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Enter an absolute local file or directory path"})),
        );
    }
    match tokio::task::spawn_blocking(move || {
        state
            .repo
            .add_source(std::path::Path::new(&input.path), input.include_content)
    })
    .await
    {
        Ok(Ok(source)) => (StatusCode::CREATED, Json(json!(source))),
        Ok(Err(error)) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Cannot register source"})),
        ),
    }
}

#[derive(Deserialize)]
pub struct SetEnabled {
    enabled: Option<bool>,
    include_content: Option<bool>,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SetEnabled>,
) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || {
        state
            .repo
            .update_source_policy(&id, input.enabled, input.include_content)
    })
    .await
    {
        Ok(Ok(true)) => (StatusCode::OK, Json(json!({"status":"saved"}))),
        Ok(Ok(false)) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"Source not found"})),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Cannot update source"})),
        ),
    }
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || state.repo.remove_source(&id)).await {
        Ok(Ok(true)) => (
            StatusCode::OK,
            Json(json!({"status":"removed","telemetry_retained":true})),
        ),
        Ok(Ok(false)) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"Source not found"})),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"Cannot remove source"})),
        ),
    }
}

pub async fn collection(State(state): State<AppState>) -> impl IntoResponse {
    match tokio::task::spawn_blocking(move || state.repo.collection_status()).await {
        Ok(Ok(mut status)) => {
            let home = std::env::var_os("CODEX_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".codex")));
            status["detected_codex_path"] =
                json!(home.map(|p| p.join("sessions")).filter(|p| p.is_dir()));
            (StatusCode::OK, Json(status))
        }
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"Collection status unavailable"})),
        ),
    }
}
