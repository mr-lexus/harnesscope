use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use harnesscope::{
    adapters::{codex::execution_id, scan_sources},
    server::{correlation::CorrelationEngine, handlers::AppState, routes::build_router},
    storage::{repository::ExecutionFilter, Database, Repository},
};
use serde_json::{json, Value};
use std::{
    io::Write,
    sync::{atomic::AtomicBool, Arc},
};
use tower::ServiceExt;

const FIXTURE: &str = include_str!("fixtures/rollout-codex.jsonl");
fn setup() -> (Database, Arc<Repository>, Arc<CorrelationEngine>) {
    let db = Database::open_in_memory().unwrap();
    let repo = Arc::new(Repository::new(db.clone()));
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));
    (db, repo, engine)
}
fn scan(repo: &Repository, engine: &CorrelationEngine) {
    scan_sources(repo, engine, &AtomicBool::new(false)).unwrap();
}
fn count(repo: &Repository) -> usize {
    repo.list_executions(&ExecutionFilter::default()).unwrap().1
}
fn append(path: &std::path::Path, text: &str) {
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

#[test]
fn repeated_session_metadata_preserves_usage_across_checkpoint_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-repeated-meta.jsonl");
    let db_path = dir.path().join("test.db");
    let lines: Vec<_> = FIXTURE.split_inclusive('\n').collect();
    std::fs::write(&path, lines[..8].concat()).unwrap();
    {
        let repo = Arc::new(Repository::new(Database::open(&db_path).unwrap()));
        let engine = CorrelationEngine::new(repo.clone());
        repo.add_source(&path, false).unwrap();
        scan(&repo, &engine);
    }
    // The repeated meta sits between two equal legacy cumulative samples.
    // Resetting the baseline here would charge the first 120 tokens twice.
    append(&path, lines[0]);
    append(&path, &lines[8..].concat());
    let repo = Arc::new(Repository::new(Database::open(&db_path).unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    scan(&repo, &engine);
    for (turn, total) in [("turn-1", 120), ("turn-2", 100)] {
        assert_eq!(
            repo.get_usage(&execution_id("fixture-thread", turn))
                .unwrap()
                .unwrap()
                .total_tokens,
            total
        );
    }
    assert_eq!(count(&repo), 2);
}

#[test]
fn modern_usage_replaces_legacy_counters_and_accepts_final_sample_after_completion() {
    let (_, repo, engine) = setup();
    let mut state = harnesscope::adapters::codex::CodexState::default();
    let usage = |n| json!({"input_tokens":n,"cached_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":n,"cache_write_input_tokens":0});
    let records = vec![
        json!({"type":"session_meta","payload":{"id":"modern","cwd":"/fixture"}}),
        json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"one"}}),
        json!({"type":"token_usage_record","payload":{"thread_id":"modern","turn_id":"one","turn_token_usage":usage(10),"thread_token_usage":usage(10)}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":usage(1000)}}}),
        json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"one"}}),
        json!({"type":"token_usage_record","payload":{"thread_id":"modern","turn_id":"one","turn_token_usage":usage(12),"thread_token_usage":usage(12)}}),
        json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"two"}}),
        json!({"type":"token_usage_record","payload":{"thread_id":"modern","turn_id":"two","turn_token_usage":usage(5),"thread_token_usage":usage(17)}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":usage(1007)}}}),
    ];
    for mut record in records {
        record["timestamp"] = json!("2026-10-03T12:00:00Z");
        let converted = state.convert(&record, false).unwrap();
        engine
            .process_events_with(&converted.events, || Ok(()))
            .unwrap();
        // A restart between any two records preserves the modern/legacy baseline.
        state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    }
    assert_eq!(
        repo.get_usage(&execution_id("modern", "one"))
            .unwrap()
            .unwrap()
            .total_tokens,
        12
    );
    assert_eq!(
        repo.get_usage(&execution_id("modern", "two"))
            .unwrap()
            .unwrap()
            .total_tokens,
        5
    );
    let wrong = json!({"timestamp":"2026-10-03T12:00:00Z","type":"token_usage_record","payload":{"thread_id":"other","turn_id":"two","turn_token_usage":usage(100)}});
    assert!(state.convert(&wrong, false).unwrap().events.is_empty());
}

#[test]
fn large_private_output_is_consumed_but_never_stored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-large.jsonl");
    let mut rows = FIXTURE.to_owned();
    rows.push_str(&json!({"timestamp":"2026-10-03T12:00:00Z","type":"response_item","payload":{"type":"function_call_output","output":"private-content".repeat(700_000)}}).to_string());
    rows.push('\n');
    std::fs::write(&path, rows).unwrap();
    let (db, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    let file = &repo.list_source_files(&source.id).unwrap()[0];
    assert_eq!(file.status, "READY");
    assert_eq!(file.byte_offset, std::fs::metadata(path).unwrap().len());
    assert_eq!(
        db.with_conn(|c| c.query_row(
            "SELECT COUNT(*) FROM events WHERE payload_json LIKE '%private-content%'",
            [],
            |r| r.get::<_, i64>(0)
        ))
        .unwrap(),
        0
    );
}

#[test]
fn collection_status_distinguishes_disconnected_paused_backlog_and_error() {
    let (_, repo, engine) = setup();
    assert_eq!(repo.collection_status().unwrap()["status"], "NOT_CONNECTED");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-status.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let source = repo.add_source(&path, false).unwrap();
    assert_eq!(repo.collection_status().unwrap()["status"], "STARTING");
    scan(&repo, &engine);
    assert_eq!(repo.collection_status().unwrap()["status"], "COLLECTING");
    repo.set_source_enabled(&source.id, false).unwrap();
    assert_eq!(repo.collection_status().unwrap()["status"], "PAUSED");
    repo.set_source_enabled(&source.id, true).unwrap();
    append(&path, "broken\n");
    scan(&repo, &engine);
    assert_eq!(
        repo.collection_status().unwrap()["status"],
        "NEEDS_ATTENTION"
    );
}

#[test]
fn native_turns_tools_models_and_usage_are_idempotent_and_metadata_only() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (db, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    assert_eq!(
        count(&repo),
        2,
        "{:?}",
        repo.list_source_files(&source.id).unwrap()
    );
    let id = execution_id("fixture-thread", "turn-1");
    let first = repo.find_execution_by_id(&id).unwrap().unwrap();
    assert_eq!(first.model, "fixture-model");
    assert_eq!(first.reasoning_effort, "high");
    assert_eq!(first.capture_scope, "TURN");
    assert_eq!(first.status, "COMPLETED");
    assert_eq!(first.duration_ms, Some(7000));
    assert_eq!(first.exit_code, None);
    assert_eq!(first.prompt_summary, None);
    assert_eq!(
        repo.find_session_by_id(&first.session_id)
            .unwrap()
            .unwrap()
            .native_session_id,
        "fixture-thread"
    );
    assert_eq!(repo.get_usage(&id).unwrap().unwrap().total_tokens, 120);
    assert_eq!(
        repo.list_components_for_execution(&id).unwrap()[0].invocations_count,
        1
    );
    let second = execution_id("fixture-thread", "turn-2");
    assert_eq!(
        repo.find_execution_by_id(&second).unwrap().unwrap().status,
        "CANCELLED"
    );
    assert_eq!(repo.get_usage(&second).unwrap().unwrap().total_tokens, 100);
    assert_eq!(repo.get_health_metrics().unwrap().active_runtimes, 0);
    let before = repo.list_events_for_execution(&id).unwrap().len();
    scan(&repo, &engine);
    // A relocated copy also has stable native IDs.
    let copy = temp.path().join("rollout-copy.jsonl");
    std::fs::copy(&path, &copy).unwrap();
    repo.add_source(&copy, false).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 2);
    assert_eq!(repo.list_events_for_execution(&id).unwrap().len(), before);
    assert_eq!(repo.get_usage(&second).unwrap().unwrap().total_tokens, 100);
    let file = repo.list_source_files(&source.id).unwrap().remove(0);
    assert_eq!(file.status, "READY");
    assert_eq!(file.byte_offset, FIXTURE.len() as u64);
    let stored: String = db
        .with_conn(|c| {
            c.query_row("SELECT group_concat(payload_json) FROM events", [], |r| {
                r.get(0)
            })
        })
        .unwrap();
    assert!(!stored.contains("DO_NOT_STORE"));
    assert!(!file.state_json.contains("DO_NOT_STORE"));
}

#[test]
fn partial_utf8_line_waits_and_restart_resumes_at_exact_byte_offset() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-live.jsonl");
    let head = FIXTURE.lines().take(2).collect::<Vec<_>>().join("\n") + "\n";
    let tail="{\"timestamp\":\"2026-10-03T10:00:03Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"user_message\",\"message\":\"Привет\"}}\n";
    let split = tail.find('П').unwrap() + 1;
    std::fs::write(&path, [head.as_bytes(), &tail.as_bytes()[..split]].concat()).unwrap();
    let db_path = temp.path().join("test.db");
    {
        let repo = Arc::new(Repository::new(Database::open(&db_path).unwrap()));
        let engine = CorrelationEngine::new(repo.clone());
        let source = repo.add_source(&path, true).unwrap();
        scan(&repo, &engine);
        let file = &repo.list_source_files(&source.id).unwrap()[0];
        assert_eq!(file.status, "WAITING", "{:?}", file);
        assert_eq!(file.byte_offset, head.len() as u64);
        let unchanged = file.clone();
        scan(&repo, &engine);
        let after = repo.list_source_files(&source.id).unwrap().remove(0);
        assert_eq!(
            after.updated_at, unchanged.updated_at,
            "An unchanged partial line must not be reprocessed"
        );
        assert_eq!(after.state_json, unchanged.state_json);
        assert_eq!(after.byte_offset, unchanged.byte_offset);
    }
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&tail.as_bytes()[split..])
        .unwrap();
    let repo = Arc::new(Repository::new(Database::open(&db_path).unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    scan(&repo, &engine);
    let first = repo
        .find_execution_by_id(&execution_id("fixture-thread", "turn-1"))
        .unwrap()
        .unwrap();
    assert_eq!(first.prompt_summary.as_deref(), Some("Привет"));
    assert_eq!(first.status, "RUNNING"); // EOF never means success or process death.
}

#[test]
fn malformed_file_does_not_advance_or_block_other_files_and_can_be_repaired() {
    let temp = tempfile::tempdir().unwrap();
    let bad = temp.path().join("rollout-bad.jsonl");
    let good = temp.path().join("rollout-good.jsonl");
    std::fs::write(&bad, "{broken}\n").unwrap();
    std::fs::write(&good, FIXTURE).unwrap();
    let (_, repo, engine) = setup();
    let source = repo.add_source(temp.path(), false).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 2);
    let files = repo.list_source_files(&source.id).unwrap();
    let broken = files.iter().find(|f| f.status == "ERROR").unwrap();
    assert_eq!(broken.byte_offset, 0);
    assert!(broken.last_error.as_ref().unwrap().contains("line 1"));
    assert!(repo.list_sources().unwrap()[0].last_error.is_some());
    std::fs::write(&bad, FIXTURE.replace("fixture-thread", "another-thread")).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 4);
    assert!(repo.list_sources().unwrap()[0].last_error.is_none());
}

#[test]
fn consumed_prefix_rewrite_and_truncation_are_visible_not_silently_replayed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (_, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    std::fs::write(&path, FIXTURE.replace("fixture-thread", "changed-thread")).unwrap();
    scan(&repo, &engine);
    let file = repo.list_source_files(&source.id).unwrap().remove(0);
    assert_eq!(file.byte_offset, FIXTURE.len() as u64);
    assert!(file.last_error.unwrap().contains("changed"));
    std::fs::write(&path, "").unwrap();
    scan(&repo, &engine);
    assert!(repo.list_source_files(&source.id).unwrap()[0]
        .last_error
        .as_ref()
        .unwrap()
        .contains("truncated"));
    assert_eq!(count(&repo), 2);
}

#[test]
fn checkpoint_and_all_projections_roll_back_on_database_failure() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (db, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_usage BEFORE INSERT ON execution_usage BEGIN SELECT RAISE(ABORT,'simulated failure'); END;")).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 0);
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].byte_offset,
        0
    );
    db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_usage"))
        .unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 2);
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].status,
        "READY"
    );
}

#[test]
fn stale_collector_checkpoint_rejects_the_entire_batch() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (_, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    let saved = repo.list_source_files(&source.id).unwrap().remove(0);
    let event = serde_json::from_value(json!({
        "event_id":"stale-collector-batch", "event_type":"adapter.observation",
        "source":"test", "payload":{}
    }))
    .unwrap();
    let second = CorrelationEngine::new(repo.clone());
    assert!(second
        .process_events_with(&[event], || repo.save_source_file(&saved, 0))
        .is_err());
    assert!(!repo.event_exists("stale-collector-batch").unwrap());
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].byte_offset,
        saved.byte_offset
    );
}

#[test]
fn pause_resume_and_disconnect_preserve_imported_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (_, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    repo.set_source_enabled(&source.id, false).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 0);
    repo.set_source_enabled(&source.id, true).unwrap();
    scan(&repo, &engine);
    repo.remove_source(&source.id).unwrap();
    assert_eq!(count(&repo), 2);
    assert!(repo.list_source_files(&source.id).unwrap().is_empty());
    repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    assert_eq!(count(&repo), 2);
}

#[test]
fn fork_does_not_charge_inherited_usage_and_counter_reset_is_not_negative() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-fork.jsonl");
    let mut records: Vec<Value> = FIXTURE
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    records[0]["payload"]["forked_from_id"] = json!("unimported-parent");
    // First cumulative sample (120) establishes a baseline; second is unchanged.
    records[12]["payload"]["info"]["total_token_usage"] = json!({"input_tokens":10,"cached_input_tokens":0,"output_tokens":2,"reasoning_output_tokens":0,"total_tokens":12});
    std::fs::write(
        &path,
        records.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let (_, repo, engine) = setup();
    repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    assert_eq!(
        repo.get_usage(&execution_id("fixture-thread", "turn-1"))
            .unwrap()
            .unwrap()
            .total_tokens,
        0
    );
    assert!(repo
        .get_usage(&execution_id("fixture-thread", "turn-2"))
        .unwrap()
        .is_none());
}

#[test]
fn prompt_opt_in_redacts_and_never_copies_tool_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (db, repo, engine) = setup();
    repo.add_source(&path, true).unwrap();
    scan(&repo, &engine);
    let first = repo
        .find_execution_by_id(&execution_id("fixture-thread", "turn-1"))
        .unwrap()
        .unwrap();
    assert!(first.prompt_summary.unwrap().contains("Fix the sample"));
    let stored: String = db
        .with_conn(|c| {
            c.query_row("SELECT group_concat(payload_json) FROM events", [], |r| {
                r.get(0)
            })
        })
        .unwrap();
    assert!(!stored.contains("DO_NOT_STORE"));
    assert!(repo.add_source(&path, false).is_err());
}

#[test]
fn incomplete_native_turn_does_not_falsely_conflict_with_live_wrapper() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(
        &path,
        FIXTURE.lines().take(2).collect::<Vec<_>>().join("\n") + "\n",
    )
    .unwrap();
    let (_, repo, engine) = setup();
    repo.add_source(&path, false).unwrap();
    scan(&repo, &engine);
    for value in [
        json!({"event_type":"runtime.started","runtime_id":"wrapper","payload":{"runner_name":"codex"}}),
        json!({"event_type":"session.identified","runtime_id":"wrapper","session_id":"wrapper-session","payload":{"runner_name":"codex","native_session_id":"fixture-thread"}}),
        json!({"event_type":"execution.started","runtime_id":"wrapper","session_id":"wrapper-session","execution_id":"wrapper-execution","payload":{"capture_scope":"PROCESS"}}),
    ] {
        let mut value = value;
        value["source"] = json!("test");
        engine
            .process_event(&serde_json::from_value(value).unwrap())
            .unwrap();
    }
    assert_eq!(count(&repo), 2);
    assert_eq!(
        repo.list_executions(&ExecutionFilter {
            scope: Some("TURN".into()),
            ..Default::default()
        })
        .unwrap()
        .1,
        1
    );
    assert_eq!(
        repo.list_executions(&ExecutionFilter {
            scope: Some("PROCESS".into()),
            ..Default::default()
        })
        .unwrap()
        .1,
        1
    );
    assert!(repo.list_all_conflicts().unwrap().is_empty());
    assert_eq!(repo.get_health_metrics().unwrap().active_runtimes, 1);
}

#[tokio::test]
async fn sources_api_validates_paths_supports_pause_and_hides_parser_state() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-a.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    let (_, repo, engine) = setup();
    let router = build_router(AppState {
        repo: repo.clone(),
        engine: engine.clone(),
        shutdown_tx: None,
        outbox: None,
    });
    let request = |method: &str, uri: &str, body: Value| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let bad = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/sources",
            json!({"path":"relative"}),
        ))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    let response = router
        .clone()
        .oneshot(request("POST", "/api/v1/sources", json!({"path":path})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let source: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap();
    scan(&repo, &engine);
    let response = router
        .clone()
        .oneshot(request("GET", "/api/v1/sources", json!(null)))
        .await
        .unwrap();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!body.contains("state_json"));
    let uri = format!("/api/v1/sources/{}", source["id"].as_str().unwrap());
    let response = router
        .clone()
        .oneshot(request("PATCH", &uri, json!({"enabled":false})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!repo.list_sources().unwrap()[0].enabled);
    assert_eq!(
        router
            .oneshot(request("DELETE", &uri, json!(null)))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(count(&repo), 2);
}

#[test]
fn backlog_is_bounded_and_cancellation_does_not_drop_checkpoints() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rollout-large.jsonl");
    std::fs::write(&path, FIXTURE).unwrap();
    for _ in 0..250 {
        append(
            &path,
            "{\"timestamp\":\"2026-10-03T10:00:13Z\",\"type\":\"future_record\",\"payload\":{}}\n",
        );
    }
    let (_, repo, engine) = setup();
    let source = repo.add_source(&path, false).unwrap();
    scan_sources(&repo, &engine, &AtomicBool::new(true)).unwrap();
    assert!(repo.list_source_files(&source.id).unwrap().is_empty());
    scan(&repo, &engine);
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].line_number,
        200
    );
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].status,
        "BACKLOG"
    );
    assert_eq!(repo.collection_status().unwrap()["status"], "CATCHING_UP");
    scan(&repo, &engine);
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].status,
        "READY"
    );
    assert_eq!(
        repo.list_source_files(&source.id).unwrap()[0].ignored_count,
        250
    );
}
