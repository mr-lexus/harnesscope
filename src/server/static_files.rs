use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Asset;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() {
        path = "index.html".to_string();
    }

    if path.starts_with("api/")
        || path.split('/').any(|part| part == ".." || part == ".")
        || path.contains('\\')
    {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    // Try embedded asset
    if let Some(content) = Asset::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime.as_ref()).unwrap(),
            )
            .body(Body::from(content.data))
            .unwrap();
    }

    // If path has no extension, fallback to index.html for SPA routing
    if !path.contains('.') {
        if let Some(index) = Asset::get("index.html") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )
                .body(Body::from(index.data))
                .unwrap();
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"))
        .body(Body::from("404 Not Found"))
        .unwrap()
}
