use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use harnesscope::{
    domain::events::IngestEvent,
    server::{correlation::CorrelationEngine, handlers::AppState, routes::build_router},
    storage::{
        repository::{ExecutionFilter, SessionFilter},
        retrospective::{RetrospectiveFilter, Review},
        Database, Repository,
    },
};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use tower::ServiceExt;

fn setup() -> (Database, Arc<Repository>, Arc<CorrelationEngine>) {
    let db = Database::open_in_memory().unwrap();
    let repo = Arc::new(Repository::new(db.clone()));
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));
    (db, repo, engine)
}
fn event(
    id: &str,
    kind: &str,
    runtime: &str,
    session: &str,
    execution: &str,
    payload: Value,
) -> IngestEvent {
    IngestEvent {
        event_id: id.into(),
        event_type: kind.into(),
        source: "test".into(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        runtime_id: Some(runtime.into()),
        session_id: Some(session.into()),
        execution_id: Some(execution.into()),
        agent_instance_id: None,
        payload,
        extra: HashMap::new(),
    }
}
fn start(engine: &CorrelationEngine, id: &str) {
    engine
        .process_event(&event(
            &format!("start-{id}"),
            "execution.started",
            "runtime",
            "session",
            id,
            json!({"capture_scope":"TURN"}),
        ))
        .unwrap();
}

#[test]
fn duplicate_delivery_does_not_reopen_execution_or_double_count_invocations() {
    let (_, repo, engine) = setup();
    let started = event("start", "execution.started", "r", "s", "e", json!({}));
    engine.process_event(&started).unwrap();
    let tool = event(
        "tool",
        "mcp.invoked",
        "r",
        "s",
        "e",
        json!({"name":"context7"}),
    );
    engine.process_event(&tool).unwrap();
    engine.process_event(&tool).unwrap();
    engine
        .process_event(&event(
            "stop",
            "execution.completed",
            "r",
            "s",
            "e",
            json!({"exit_code":0}),
        ))
        .unwrap();
    engine.process_event(&started).unwrap();
    let mut second_start = started.clone();
    second_start.event_id = "different-start-id".into();
    engine.process_event(&second_start).unwrap();
    assert_eq!(
        repo.find_execution_by_id("e").unwrap().unwrap().status,
        "COMPLETED"
    );
    assert_eq!(
        repo.list_components_for_execution("e").unwrap()[0].invocations_count,
        1
    );
    let agents = repo.list_agent_instances_for_execution("e").unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].status, "COMPLETED");
}

#[test]
fn failed_event_rolls_back_all_projection_changes_and_can_be_retried() {
    let (db, repo, engine) = setup();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT, 'injected failure'); END;")).unwrap();
    let started = event("atomic", "execution.started", "r", "s", "e", json!({}));
    assert!(engine.process_event(&started).is_err());
    assert_eq!(repo.get_health_metrics().unwrap().total_executions, 0);
    assert!(repo.find_runtime_by_id("r").unwrap().is_none());
    assert!(repo.find_session_by_id("s").unwrap().is_none());
    db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_event"))
        .unwrap();
    engine.process_event(&started).unwrap();
    assert!(repo.event_exists("atomic").unwrap());
}

#[test]
fn wrapper_resume_aliases_are_persistent_and_resume_counts_are_not_multiplied() {
    let (_, repo, engine) = setup();
    engine
        .process_event(&event(
            "s1",
            "session.identified",
            "r1",
            "s1",
            "e1",
            json!({"runner_name":"agent","native_session_id":"native"}),
        ))
        .unwrap();
    engine
        .process_event(&event(
            "e1",
            "execution.started",
            "r1",
            "s1",
            "e1",
            json!({}),
        ))
        .unwrap();
    engine
        .process_event(&event(
            "s2",
            "session.identified",
            "r2",
            "fresh-wrapper-id",
            "e2",
            json!({"runner_name":"agent","native_session_id":"native"}),
        ))
        .unwrap();
    // Recreate the engine to prove aliases are persisted, not a request-local map.
    let engine = CorrelationEngine::new(repo.clone());
    engine
        .process_event(&event(
            "e2",
            "execution.started",
            "r2",
            "fresh-wrapper-id",
            "e2",
            json!({}),
        ))
        .unwrap();
    assert_eq!(
        repo.find_execution_by_id("e2").unwrap().unwrap().session_id,
        "s1"
    );
    let (sessions, total) = repo.list_sessions(&SessionFilter::default()).unwrap();
    assert_eq!(total, 1);
    assert_eq!(sessions[0].execution_count, 2);
    assert_eq!(sessions[0].resume_count, 1);
    assert!(repo
        .list_events_for_session("s1")
        .unwrap()
        .iter()
        .any(|e| e.event_id == "s2"));
}

#[test]
fn unknown_parent_does_not_drop_session_and_unknown_entities_do_not_merge() {
    let (_, repo, engine) = setup();
    engine.process_event(&event("fork","session.identified","r","s","e",json!({"runner_name":"agent","native_session_id":"child","parent_session_id":"not-imported"}))).unwrap();
    assert!(repo
        .find_session_by_id("s")
        .unwrap()
        .unwrap()
        .parent_session_id
        .is_none());
    for id in ["e1", "e2"] {
        let mut e = event(id, "execution.started", "r", "s", id, json!({}));
        e.runtime_id = None;
        e.session_id = None;
        engine.process_event(&e).unwrap();
    }
    let a = repo.find_execution_by_id("e1").unwrap().unwrap();
    let b = repo.find_execution_by_id("e2").unwrap().unwrap();
    assert_ne!(a.session_id, b.session_id);
    assert_ne!(a.runtime_id, b.runtime_id);
}

#[test]
fn structured_redaction_preserves_json_and_protects_projected_fields() {
    let (_, repo, engine) = setup();
    let raw = json!({"env":{"OPENAI_API_KEY":"short-secret"},"headers":{"Authorization":"Basic abc"},"nested":[{"access_token":"abc\\\"def"}],"command":"run --password 'two words'", "text":"API_KEY=unprefixed-secret"});
    let safe = harnesscope::redact::redact_value(&raw).to_string();
    for secret in [
        "short-secret",
        "Basic abc",
        "two words",
        "unprefixed-secret",
        "abc\\",
    ] {
        assert!(!safe.contains(secret), "leaked {secret}");
    }
    serde_json::from_str::<Value>(&safe).unwrap();
    engine
        .process_event(&event(
            "secret-title",
            "session.identified",
            "r",
            "s",
            "e",
            json!({"title":"Fix API_KEY=short-secret"}),
        ))
        .unwrap();
    assert!(!repo
        .find_session_by_id("s")
        .unwrap()
        .unwrap()
        .title
        .unwrap()
        .contains("short-secret"));
}

#[test]
fn retrospective_separates_quality_from_exit_status_and_excludes_demo() {
    let (_, repo, engine) = setup();
    start(&engine, "e");
    engine
        .process_event(&event(
            "complete",
            "execution.completed",
            "runtime",
            "session",
            "e",
            json!({"exit_code":0,"duration_ms":0}),
        ))
        .unwrap();
    repo.save_review(
        "e",
        &Review {
            outcome: "REWORK".into(),
            notes: "API_KEY=secret".into(),
            experiment: "tests-first".into(),
        },
    )
    .unwrap();
    harnesscope::demo::seed_demo_data(&repo).unwrap();
    let report = repo.retrospective(&RetrospectiveFilter::default()).unwrap();
    assert_eq!(report.total, 1);
    assert_eq!(report.reviewed, 1);
    assert_eq!(report.accepted, 0);
    assert_eq!(report.cohorts[0].completed, 1);
    assert_eq!(report.cohorts[0].rework, 1);
    assert_eq!(report.cohorts[0].avg_duration_ms, Some(0.0));
    assert!(!repo
        .get_review("e")
        .unwrap()
        .unwrap()
        .notes
        .contains("=secret"));
    let before = repo.get_health_metrics().unwrap().total_executions;
    harnesscope::demo::seed_demo_data(&repo).unwrap();
    assert_eq!(repo.get_health_metrics().unwrap().total_executions, before);
    let report = repo
        .retrospective(&RetrospectiveFilter {
            include_demo: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(report.total, before);
}

#[test]
fn pagination_extremes_and_timestamp_offsets_are_safe() {
    let (_, repo, engine) = setup();
    let mut e = event("old", "execution.started", "r", "s", "e", json!({}));
    e.timestamp = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    engine.process_event(&e).unwrap();
    assert_eq!(
        repo.list_executions(&ExecutionFilter {
            period: Some("1h".into()),
            ..Default::default()
        })
        .unwrap()
        .1,
        0
    );
    assert!(repo
        .list_executions(&ExecutionFilter {
            page: Some(u32::MAX),
            page_size: Some(100),
            ..Default::default()
        })
        .unwrap()
        .0
        .is_empty());
    assert!(repo
        .list_sessions(&SessionFilter {
            page: Some(u32::MAX),
            ..Default::default()
        })
        .unwrap()
        .0
        .is_empty());
}

#[tokio::test]
async fn api_blocks_cross_origin_dns_rebinding_and_static_traversal() {
    let (_, repo, engine) = setup();
    let app = build_router(AppState {
        repo,
        engine,
        shutdown_tx: None,
        outbox: None,
    });
    for request in [
        Request::builder()
            .uri("/api/v1/health")
            .header("host", "evil.example:4242")
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .uri("/api/v1/shutdown")
            .method("POST")
            .header("host", "127.0.0.1:4242")
            .header("origin", "https://evil.example")
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .uri("/api/v1/health")
            .header("host", "127.0.0.1:4242")
            .header("origin", "http://127.0.0.1:9999")
            .body(Body::empty())
            .unwrap(),
    ] {
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    for path in ["/../../Cargo.toml", "/api/v1/unknown", "/assets/missing.js"] {
        assert_eq!(
            app.clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header("host", "127.0.0.1:4242")
                .header("origin", "http://127.0.0.1:4242")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}

#[tokio::test]
async fn ingestion_reports_partial_failure_and_validates_before_writing() {
    let (_, repo, engine) = setup();
    let app = build_router(AppState {
        repo: repo.clone(),
        engine,
        shutdown_tx: None,
        outbox: None,
    });
    let events = json!([{"event_id":"ok","event_type":"custom","source":"test","payload":{}},{"event_id":"bad","event_type":"execution.completed","execution_id":"missing","source":"test","payload":{}}]);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/events")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(events.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(body["processed"], 1);
    assert_eq!(body["errors"][0]["event_id"], "bad");
    let invalid = json!([{"event_id":"not-written","event_type":"custom","source":"test","payload":{}},{"event_type":"custom","source":"test","timestamp":"invalid","payload":{}}]);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(invalid.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!repo.event_exists("not-written").unwrap());
}

#[test]
fn import_accepts_bom_jsonl_and_uses_stable_fallback_ids() {
    let input = "\u{feff}{\"event_type\":\"custom\",\"source\":\"test\",\"payload\":{}}\n";
    let a = harnesscope::ingest::parse_events(input).unwrap();
    let b = harnesscope::ingest::parse_events(input).unwrap();
    assert_eq!(a[0].event_id, b[0].event_id);
    assert!(harnesscope::ingest::parse_events("[]").is_err());
}

#[test]
fn documented_adapter_example_is_importable_and_links_partial_envelopes() {
    let (_, repo, engine) = setup();
    let events =
        harnesscope::ingest::parse_events(include_str!("../examples/events.jsonl")).unwrap();
    for event in &events {
        engine.process_event(event).unwrap();
    }
    for event in &events {
        engine.process_event(event).unwrap();
    }
    assert_eq!(repo.get_health_metrics().unwrap().total_executions, 1);
    assert_eq!(
        repo.list_components_for_execution("example:turn").unwrap()[0].invocations_count,
        1
    );
    assert!(repo
        .list_events_for_session("example:session")
        .unwrap()
        .iter()
        .any(|e| e.event_id == "example:tool:1"));
}

#[test]
fn runtime_stop_does_not_invent_success_for_missing_completion() {
    let (_, repo, engine) = setup();
    start(&engine, "e");
    engine
        .process_event(&event(
            "stopped",
            "runtime.stopped",
            "runtime",
            "session",
            "e",
            json!({"exit_code":0}),
        ))
        .unwrap();
    assert_eq!(
        repo.find_execution_by_id("e").unwrap().unwrap().status,
        "UNKNOWN"
    );
    assert_eq!(
        repo.list_agent_instances_for_execution("e").unwrap()[0].status,
        "UNKNOWN"
    );
}

#[test]
fn server_url_rejects_remote_destinations_and_accepts_ipv6_loopback() {
    for value in [
        "https://example.com",
        "http://127.0.0.1.evil.example:4242",
        "http://user:pass@127.0.0.1:4242",
        "http://127.0.0.1:4242/remote",
    ] {
        assert!(harnesscope::config::local_server_url(value).is_err());
    }
    assert!(harnesscope::config::local_server_url("http://[::1]:4343").is_ok());
}
