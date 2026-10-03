use harnesscope::{
    adapters::{codex::CodexState, scan_sources},
    capture,
    server::correlation::CorrelationEngine,
    storage::{evidence::EvidenceFilter, Database, Repository},
};
use serde_json::json;
use std::{
    io::Write,
    sync::{atomic::AtomicBool, Arc},
};

fn input(position: &str) -> capture::ObservationInput {
    let mut i = capture::from_record(
        "test",
        "synthetic",
        position,
        &json!({"type":"response_item","timestamp":"2026-10-01T12:00:00Z"}),
    );
    i.session_id = Some("thread-1".into());
    i.project = Some("project".into());
    i
}

#[test]
fn archive_is_idempotent_paginated_and_reindexable_without_source() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    for n in 0..5 {
        let i = input(&n.to_string());
        let s = capture::sanitize(&json!({"type":"unknown_future_record","payload":{"answer":n}}));
        repo.record_observation(&i, &s).unwrap();
        repo.record_observation(&i, &s).unwrap();
    }
    let f = EvidenceFilter {
        limit: Some(2),
        ..Default::default()
    };
    let first = repo.evidence_page(&f).unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    repo.record_observation(&input("later"), &capture::sanitize(&json!("later")))
        .unwrap();
    let second = repo
        .evidence_page(&EvidenceFilter {
            after: first["next"].as_i64(),
            through: first["through"].as_i64(),
            limit: Some(200),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 3);
    let before = repo.evidence_page(&EvidenceFilter::default()).unwrap();
    assert_eq!(repo.reindex_evidence().unwrap(), 6);
    assert_eq!(
        before,
        repo.evidence_page(&EvidenceFilter::default()).unwrap()
    );
}

#[test]
fn secrets_and_secret_file_outputs_are_excluded_before_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("safe.db");
    let repo = Repository::new(Database::open(&db_path).unwrap());
    let raw = json!({"password":"secret-canary-a","nested":{"apiKey":"secret-canary-b"},"attributes":[{"key":"authorization","value":{"stringValue":"secret-canary-c"}}],"body":"bearer secret-canary-d"});
    let safe = capture::sanitize(&raw);
    repo.record_observation(&input("one"), &safe).unwrap();
    let mut state = CodexState::default();
    let call = json!({"type":"response_item","payload":{"type":"function_call","call_id":"call-1","name":"shell","arguments":"cat .env"}});
    let (_, safe) = state.observation("file", "2", &call, true);
    assert!(safe.value().is_none());
    let result = json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"UNLABELLED=secret-canary-e"}});
    let (mut i, safe) = state.observation("file", "3", &result, true);
    i.session_id = Some("thread-1".into());
    assert_eq!(safe.reason.as_deref(), Some("secret_related_tool_call"));
    repo.record_observation(&i, &safe).unwrap();
    let backup = dir.path().join("backup.db");
    harnesscope::storage::backup::create(&db_path, &backup).unwrap();
    let export = dir.path().join("export.jsonl");
    repo.export_evidence(&EvidenceFilter::default(), &export)
        .unwrap();
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(
                !String::from_utf8_lossy(&bytes).contains("secret-canary"),
                "leaked into {:?}",
                entry.path()
            );
        }
    }
}

#[test]
fn large_objects_survive_portable_backup_restore_and_detect_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.db");
    let repo = Repository::new(Database::open(&source).unwrap());
    repo.record_observation(&input("1"), &capture::sanitize(&json!("x".repeat(90_000))))
        .unwrap();
    let page = repo.evidence_page(&EvidenceFilter::default()).unwrap();
    let hash = page["items"][0]["object_hash"].as_str().unwrap();
    let object = repo.evidence_dir().unwrap().unwrap().join(hash);
    assert!(object.exists());
    let backup = dir.path().join("backup.db");
    harnesscope::storage::backup::create(&source, &backup).unwrap();
    let restored = dir.path().join("restored.db");
    harnesscope::storage::backup::restore(&backup, &restored).unwrap();
    let restored = Repository::new(Database::open(&restored).unwrap());
    assert_eq!(
        restored.evidence_object(hash).unwrap(),
        repo.evidence_object(hash).unwrap()
    );
    std::fs::write(object, "corrupt").unwrap();
    assert!(repo.evidence_object(hash).is_err());
    assert!(harnesscope::storage::backup::verify(&source).is_err());
}

#[test]
fn rollback_does_not_publish_observations_and_retry_is_safe() {
    let db = Database::open_in_memory().unwrap();
    let repo = Repository::new(db.clone());
    let result: rusqlite::Result<()> = repo.transaction(|| {
        repo.record_observation(
            &input("1"),
            &capture::sanitize(&json!({"type":"world_state"})),
        )?;
        Err(rusqlite::Error::InvalidQuery)
    });
    assert!(result.is_err());
    assert_eq!(
        repo.evidence_page(&EvidenceFilter::default()).unwrap()["items"],
        json!([])
    );
    repo.record_observation(
        &input("1"),
        &capture::sanitize(&json!({"type":"world_state"})),
    )
    .unwrap();
    assert_eq!(repo.reindex_evidence().unwrap(), 1);
}

#[test]
fn workflow_changes_create_versions_and_credentials_are_never_read() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("AGENTS.md"), "Run tests before finishing.").unwrap();
    std::fs::write(root.join(".env"), "PASSWORD=secret-canary").unwrap();
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.register_workflow(&root).unwrap();
    repo.scan_workflows().unwrap();
    assert_eq!(
        repo.workflow_versions(None, 0).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    std::fs::write(root.join("AGENTS.md"), "Run focused regression tests.").unwrap();
    repo.scan_workflows().unwrap();
    let versions = repo.workflow_versions(None, 0).unwrap();
    assert_eq!(versions["items"].as_array().unwrap().len(), 2);
    assert!(versions["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["state"] == "discovered"));
}

#[test]
fn vaiz_versions_are_proven_by_call_ids_and_do_not_guess_links() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    for (n,value) in [json!({"type":"response_item","payload":{"type":"function_call","call_id":"v1","name":"mcp__vaiz__get_task","arguments":"{\"taskId\":\"ABC-123\"}"}}),json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"v1","output":{"description":"first"}}}),json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"v1","output":{"description":"updated","comments":["new requirement"]}}})].into_iter().enumerate(){repo.record_observation(&input(&n.to_string()),&capture::sanitize(&value)).unwrap();}
    let task = repo.retro_task_detail("vaiz:ABC-123").unwrap();
    assert_eq!(task["versions"].as_array().unwrap().len(), 2);
    assert_eq!(task["links"][0]["status"], "confirmed");
    let marks = task["marks"].as_array().unwrap();
    assert!(marks.is_empty());
    repo.mark_retro_task("vaiz:ABC-123", "accepted", "Reviewed by human")
        .unwrap();
    assert_eq!(
        repo.retro_task_detail("vaiz:ABC-123").unwrap()["marks"][0]["kind"],
        "accepted"
    );
}

#[test]
fn gzip_history_collects_content_without_changing_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-test.jsonl.gz");
    let mut writer = flate2::write::GzEncoder::new(
        std::fs::File::create(&path).unwrap(),
        flate2::Compression::default(),
    );
    writer
        .write_all(include_bytes!("fixtures/rollout-codex.jsonl"))
        .unwrap();
    writer.finish().unwrap();
    let original = std::fs::read(&path).unwrap();
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    repo.add_source(&path, true).unwrap();
    scan_sources(&repo, &engine, &AtomicBool::new(false)).unwrap();
    assert!(
        repo.evidence_page(&EvidenceFilter::default()).unwrap()["items"]
            .as_array()
            .unwrap()
            .len()
            > 5
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn sqlite_history_reader_never_reads_credentials_or_mutates_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("thread_history_1.sqlite");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TABLE thread_items(thread_id TEXT,turn_id TEXT,item_id TEXT,updated_at_ordinal INTEGER,item_json TEXT);CREATE TABLE credentials(secret TEXT);INSERT INTO credentials VALUES('secret-canary');").unwrap();
    c.execute(
        "INSERT INTO thread_items VALUES('thread','turn','item',1,?1)",
        [
            json!({"type":"message","role":"user","content":[{"text":"Implement the feature"}]})
                .to_string(),
        ],
    )
    .unwrap();
    drop(c);
    let original = std::fs::read(&path).unwrap();
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let source = repo.add_source(&path, true).unwrap();
    harnesscope::adapters::codex_sqlite::scan(&repo, &source).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(
        repo.evidence_page(&EvidenceFilter::default()).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn export_import_is_idempotent_and_mcp_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let source = Repository::new(Database::open_in_memory().unwrap());
    source
        .record_observation(
            &input("a"),
            &capture::sanitize(&json!({"type":"message","text":"task"})),
        )
        .unwrap();
    let path = dir.path().join("package.jsonl");
    source
        .export_evidence(&EvidenceFilter::default(), &path)
        .unwrap();
    let destination = Repository::new(Database::open(dir.path().join("import.db")).unwrap());
    harnesscope::capture_io::import(&destination, &path).unwrap();
    harnesscope::capture_io::import(&destination, &path).unwrap();
    assert_eq!(destination.reindex_evidence().unwrap(), 1);
    let read = Repository::new(Database::open_read_only(&dir.path().join("import.db")).unwrap());
    let response =
        harnesscope::mcp::dispatch(&read, &json!({"id":1,"method":"tools/list"})).unwrap();
    assert!(response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["annotations"]["readOnlyHint"] == true));
    assert!(read.create_retro_task("cannot write", None).is_err());
    assert!(
        harnesscope::mcp::dispatch(&read, &json!({"method":"notifications/initialized"})).is_none()
    );
}

#[test]
fn hook_queue_never_writes_unsanitized_input_and_retries_offline() {
    use std::process::{Command, Stdio};
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("capture.db");
    let mut child = Command::new(env!("CARGO_BIN_EXE_harnesscope"))
        .args(["capture", "hook"])
        .env("HARNESSCOPE_DB_PATH", &db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    write!(child.stdin.take().unwrap(),"{}",json!({"hook_event_name":"UserPromptSubmit","session_id":"s","prompt":"password=secret-canary"})).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    for entry in std::fs::read_dir(harnesscope::capture_io::queue_dir(&db)).unwrap() {
        assert!(!std::fs::read_to_string(entry.unwrap().path())
            .unwrap()
            .contains("secret-canary"));
    }
    let repo = Repository::new(Database::open(&db).unwrap());
    assert_eq!(harnesscope::capture_io::drain(&repo, &db).unwrap(), 1);
    assert_eq!(harnesscope::capture_io::drain(&repo, &db).unwrap(), 0);
    assert_eq!(
        repo.evidence_page(&EvidenceFilter::default()).unwrap()["items"][0]["kind"],
        "UserPromptSubmit"
    );
}

#[test]
fn capture_setup_preserves_other_hooks_and_otel_and_is_reversible() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let original =
        json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":"my-existing-review"}]}]}});
    std::fs::write(root.join("hooks.json"), original.to_string()).unwrap();
    let config = "[otel]\nexporter = \"none\"\n# keep this comment\n";
    std::fs::write(root.join("config.toml"), config).unwrap();
    let exe = root.join("harnesscope");
    harnesscope::capture_setup::configure(&root, &exe, 4242, false).unwrap();
    harnesscope::capture_setup::configure(&root, &exe, 4242, false).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).unwrap(),
        config
    );
    let updated: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(updated["hooks"]["Stop"].as_array().unwrap().len(), 2);
    harnesscope::capture_setup::configure(&root, &exe, 4242, true).unwrap();
    let updated: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(updated["hooks"]["Stop"], original["hooks"]["Stop"]);
}

#[test]
fn otlp_json_deduplicates_retries_and_excludes_unpaired_tool_snippets() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    let batch = json!({"resourceLogs":[{"resource":{"attributes":[{"key":"service.version","value":{"stringValue":"fixture"}}]},"scopeLogs":[{"logRecords":[
        {"timeUnixNano":"1790812800000000000","attributes":[{"key":"event.name","value":{"stringValue":"codex.api_request"}},{"key":"conversation.id","value":{"stringValue":"thread"}},{"key":"authorization","value":{"stringValue":"secret-canary"}}]},
        {"timeUnixNano":"1790812801000000000","attributes":[{"key":"event.name","value":{"stringValue":"codex.tool_result"}},{"key":"output","value":{"stringValue":"unlabelled-secret"}}]}
    ]}]}]});
    repo.ingest_otlp(&batch).unwrap();
    repo.ingest_otlp(&batch).unwrap();
    let page = repo.evidence_page(&EvidenceFilter::default()).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["items"][1]["reason"], "unpaired_otel_tool_output");
    let hash = page["items"][0]["object_hash"].as_str().unwrap();
    assert!(!repo
        .evidence_object(hash)
        .unwrap()
        .unwrap()
        .to_string()
        .contains("secret-canary"));
    assert_eq!(page["items"][0]["session_id"], "thread");
}

#[test]
fn workflow_and_task_marks_survive_export_import_and_diff() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("AGENTS.md"), "Before\nSame\n").unwrap();
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.register_workflow(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "After\nSame\n").unwrap();
    repo.scan_workflows().unwrap();
    let versions = repo.workflow_versions(None, 0).unwrap();
    let a = versions["items"][0]["object_hash"].as_str().unwrap();
    let b = versions["items"][1]["object_hash"].as_str().unwrap();
    let diff = repo.evidence_diff(a, b).unwrap();
    assert_eq!(diff["removed"], json!(["Before"]));
    assert_eq!(diff["added"], json!(["After"]));
    let id = repo.create_retro_task("Review feature", None).unwrap();
    repo.link_retro_task(&id, "thread-1", None, true).unwrap();
    repo.mark_retro_task(&id, "rework", "Need focused test")
        .unwrap();
    repo.record_observation(&input("1"), &capture::sanitize(&json!("observed")))
        .unwrap();
    let package = root.join("retro.jsonl");
    repo.export_evidence(&EvidenceFilter::default(), &package)
        .unwrap();
    let other = Repository::new(Database::open_in_memory().unwrap());
    harnesscope::capture_io::import(&other, &package).unwrap();
    harnesscope::capture_io::import(&other, &package).unwrap();
    assert_eq!(
        other.workflow_versions(None, 0).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        other.retro_task_detail(&id).unwrap()["marks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn evidence_api_bounds_pages_blocks_cross_origin_and_exports() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));
    let router =
        harnesscope::server::routes::build_router(harnesscope::server::handlers::AppState {
            repo: repo.clone(),
            engine,
            shutdown_tx: None,
            outbox: None,
        });
    repo.record_observation(
        &input("1"),
        &capture::sanitize(&json!({"message":"preview"})),
    )
    .unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/evidence?limit=999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/evidence/export")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("\"type\":\"manifest\""));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/v1/logs")
                .method("POST")
                .header("host", "127.0.0.1:4242")
                .header("origin", "https://evil.example")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[test]
fn inherited_fork_prefix_is_archived_under_parent_without_charging_child() {
    let mut state = CodexState::default();
    let records = [
        json!({"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}),
        json!({"type":"session_meta","payload":{"id":"parent"}}),
        json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"old","started_at":100}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":0,"output_tokens":20,"reasoning_output_tokens":0,"total_tokens":120}}}}),
        json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"new","started_at":1790812800}}),
    ];
    for (n, mut record) in records.into_iter().enumerate() {
        record["timestamp"] = json!("2026-10-01T00:00:00Z");
        let (observation, _) = state.observation("fork", "position", &record, true);
        if n == 3 {
            assert_eq!(observation.session_id.as_deref(), Some("parent"));
            assert_eq!(observation.turn_id.as_deref(), Some("old"));
        }
        let result = state.convert(&record, true).unwrap();
        if n == 2 || n == 3 {
            assert!(result.events.is_empty());
        }
        if n == 4 {
            assert_eq!(result.events.len(), 1);
            assert_eq!(result.events[0].event_type, "execution.started");
            assert_eq!(observation.session_id.as_deref(), Some("child"));
        }
        state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    }
}

#[test]
fn delayed_output_from_a_secret_read_is_excluded_after_checkpoint_restart() {
    let mut state = CodexState::default();
    for record in [
        json!({"type":"response_item","payload":{"type":"function_call","call_id":"a","arguments":"{\"cmd\":\"cat .env\"}"}}),
        json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"a","output":"Process running with session ID 1234"}}),
    ] {
        state.observation("file", "position", &record, true);
    }
    state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    let (_,safe)=state.observation("file","next",&json!({"type":"response_item","payload":{"type":"function_call","call_id":"b","name":"write_stdin","arguments":"{\"session_id\":1234}"}}),true);
    assert!(safe.value().is_none());
    let (_,safe)=state.observation("file","last",&json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"b","output":"unlabelled-secret"}}),true);
    assert!(safe.value().is_none());
}

#[test]
fn context_distinguishes_window_limit_from_cumulative_usage() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.record_observation(&input("window"),&capture::sanitize(&json!({"type":"event_msg","payload":{"type":"task_started","model_context_window":128000}}))).unwrap();
    repo.record_observation(&input("usage"),&capture::sanitize(&json!({"type":"token_usage_record","payload":{"thread_token_usage":{"total_tokens":900000}}}))).unwrap();
    let page = repo.context_page(&EvidenceFilter::default()).unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["items"][0]["context"]["window_limit"]["value"], 128000);
    assert_eq!(
        page["items"][1]["context"]["active_context_tokens"]["certainty"],
        "unavailable"
    );
    assert!(page["items"][1]["context"]["active_context_tokens"]["value"].is_null());
}

#[test]
fn large_rollout_file_has_bounded_first_batch_and_no_whole_file_cap() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout-large.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "{}",
        json!({"timestamp":"2026-10-01T00:00:00Z","type":"session_meta","payload":{"id":"large"}})
    )
    .unwrap();
    for _ in 0..200 {
        writeln!(
            file,
            "{}",
            json!({"timestamp":"2026-10-01T00:00:01Z","type":"world_state","payload":{}})
        )
        .unwrap();
    }
    file.set_len(257 * 1024 * 1024).unwrap();
    drop(file);
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let source = repo.add_source(&path, true).unwrap();
    let engine = CorrelationEngine::new(repo.clone());
    scan_sources(&repo, &engine, &AtomicBool::new(false)).unwrap();
    let files = repo.list_source_files(&source.id).unwrap();
    assert_eq!(files[0].status, "BACKLOG");
    assert_eq!(files[0].line_number, 200);
    assert!(files[0].byte_offset < 1024 * 1024);
}

#[test]
fn workflow_deletion_has_an_explicit_version_and_reappearance_is_captured() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let path = root.join("AGENTS.md");
    std::fs::write(&path, "one").unwrap();
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.register_workflow(&root).unwrap();
    std::fs::remove_file(&path).unwrap();
    repo.scan_workflows().unwrap();
    repo.scan_workflows().unwrap();
    assert_eq!(
        repo.workflow_versions(None, 0).unwrap()["items"][1]["reason"],
        "workflow_missing_or_disallowed"
    );
    std::fs::write(path, "one").unwrap();
    repo.scan_workflows().unwrap();
    assert_eq!(
        repo.workflow_versions(None, 0).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}
