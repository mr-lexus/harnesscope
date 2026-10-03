use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use harnesscope::{
    domain::events::IngestEvent,
    server::{correlation::CorrelationEngine, handlers::AppState, routes::build_router},
    storage::{
        event_pages::{EventOwner, EventQuery},
        Database, Repository,
    },
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

fn event(i: usize) -> IngestEvent {
    serde_json::from_value(json!({"event_id":format!("e{i}"),"timestamp":"2026-01-01T00:00:00Z","event_type":if i==0 {"execution.started"} else if i & 1 == 0 {"test%observed"} else {"test.observed"},"source":"fixture","runtime_id":"r","session_id":"s","execution_id":"e","payload":{}})).unwrap()
}
#[tokio::test]
async fn stable_cursor_search_and_bounded_detail_under_concurrent_appends() {
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));
    engine
        .process_events_with(&(0..131).map(event).collect::<Vec<_>>(), || Ok(()))
        .unwrap();
    let first = repo
        .event_page(EventOwner::Execution, "e", &EventQuery::default())
        .unwrap();
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.total, 131);
    assert_eq!(first.items[0].event_id, "e130");
    // Delayed arrival has an old timestamp. It must appear on Latest without shifting older pages.
    let mut late = event(131);
    late.timestamp = "2025-01-01T00:00:00Z".into();
    engine.process_event(&late).unwrap();
    let mut ids: Vec<_> = first.items.iter().map(|e| e.event_id.clone()).collect();
    let mut cursor = first.next_cursor;
    while let Some(before_id) = cursor {
        let page = repo
            .event_page(
                EventOwner::Execution,
                "e",
                &EventQuery {
                    before_id: Some(before_id),
                    ..Default::default()
                },
            )
            .unwrap();
        ids.extend(page.items.iter().map(|e| e.event_id.clone()));
        cursor = page.next_cursor;
    }
    assert_eq!(ids.len(), 131);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 131);
    assert_eq!(
        repo.event_page(EventOwner::Execution, "e", &EventQuery::default())
            .unwrap()
            .items[0]
            .event_id,
        "e131"
    );
    let search = repo
        .event_page(
            EventOwner::Session,
            "s",
            &EventQuery {
                search: Some("%OBSERVED".into()),
                limit: Some(5000),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(search.total, 65, "percent is literal, not a wildcard");
    assert_eq!(search.items.len(), 65);
    let app = build_router(AppState {
        repo,
        engine,
        outbox: None,
        shutdown_tx: None,
    });
    for path in ["/api/v1/executions/e", "/api/v1/sessions/s"] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let value: Value =
            serde_json::from_slice(&to_bytes(res.into_body(), 100_000).await.unwrap()).unwrap();
        assert_eq!(value["events_total"], 132);
        assert!(value.get("events").is_none());
    }
    for (path, status) in [
        ("/api/v1/executions/e/events?limit=10000", StatusCode::OK),
        (
            "/api/v1/executions/e/events?before_id=-1",
            StatusCode::BAD_REQUEST,
        ),
        ("/api/v1/sessions/missing/events", StatusCode::NOT_FOUND),
    ] {
        let res = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), status);
        if status == StatusCode::OK {
            let value: Value =
                serde_json::from_slice(&to_bytes(res.into_body(), 100_000).await.unwrap()).unwrap();
            assert_eq!(value["items"].as_array().unwrap().len(), 100);
            assert!(value["next_cursor"].as_i64().is_some());
        }
    }
}
