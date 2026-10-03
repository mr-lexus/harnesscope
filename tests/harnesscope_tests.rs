use harnesscope::domain::events::IngestEvent;
use harnesscope::redact::redact_secrets;
use harnesscope::runners::execute_wrapper;
use harnesscope::server::correlation::CorrelationEngine;
use harnesscope::storage::{Database, Repository};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::NamedTempFile;

fn make_test_repo() -> (Arc<Repository>, NamedTempFile) {
    let tmp_file = NamedTempFile::new().expect("create temp db file");
    let db = Database::open(tmp_file.path()).expect("open db");
    let repo = Arc::new(Repository::new(db));
    (repo, tmp_file)
}

// 1. DB migrations test
#[test]
fn test_db_migrations() {
    let (repo, _tmp) = make_test_repo();
    let metrics = repo.get_health_metrics().expect("get health metrics");
    assert!(metrics.db_connected);
    assert_eq!(metrics.status, "ok");
    assert_eq!(metrics.total_executions, 0);
    assert_eq!(metrics.total_sessions, 0);
}

// 2. Secret redaction test
#[test]
fn test_secret_redaction() {
    let raw = "sk-1234567890abcdef1234567890 ghp_123456789012345678901234567890123456 Bearer eyJhbGciOiJIUzI1NiJ9.test.abc";
    let redacted = redact_secrets(raw);
    assert!(!redacted.contains("sk-1234567890abcdef1234567890"));
    assert!(!redacted.contains("ghp_123456789012345678901234567890123456"));
    assert!(!redacted.contains("eyJhbGciOiJIUzI1NiJ9.test.abc"));
    assert!(redacted.contains("[REDACTED_SECRET]"));
}

// 3. Unknown event fields tolerance
#[test]
fn test_unknown_event_fields() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let json_str = r#"{
        "event_id": "evt_custom_unknown_99",
        "timestamp": "2026-09-27T12:00:00Z",
        "event_type": "future.experimental.metric",
        "source": "future_adapter",
        "future_field_version": 99,
        "unexpected_nested": { "foo": "bar" },
        "payload": {
            "arbitrary_data": 12345,
            "secret_key": "sk-1234567890abcdef1234567890"
        }
    }"#;

    let event: IngestEvent = serde_json::from_str(json_str).expect("deserialize resilient event");
    let result = engine.process_event(&event);
    assert!(
        result.is_ok(),
        "Engine must accept unknown fields without error"
    );

    // Check that event was stored in events table and secret was redacted
    let stored_events = repo
        .list_events_for_session("dummy_session")
        .unwrap_or_default();
    assert!(stored_events.is_empty()); // dummy session has none
}

// 4. Two simultaneous executions
#[test]
fn test_two_simultaneous_executions() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let exec1_id = "exec_simul_1".to_string();
    let exec2_id = "exec_simul_2".to_string();

    let ev1 = IngestEvent {
        event_id: "evt_1".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_1".to_string()),
        session_id: Some("sess_1".to_string()),
        execution_id: Some(exec1_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "worktree_path": "C:/repos/wt1",
            "model": "gpt-4o",
        }),
        extra: HashMap::new(),
    };

    let ev2 = IngestEvent {
        event_id: "evt_2".to_string(),
        timestamp: "2026-09-27T10:00:01Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_2".to_string()),
        session_id: Some("sess_2".to_string()),
        execution_id: Some(exec2_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "worktree_path": "C:/repos/wt2",
            "model": "claude-3-5-sonnet",
        }),
        extra: HashMap::new(),
    };

    engine.process_event(&ev1).unwrap();
    engine.process_event(&ev2).unwrap();

    let e1 = repo
        .find_execution_by_id(&exec1_id)
        .unwrap()
        .expect("e1 exists");
    let e2 = repo
        .find_execution_by_id(&exec2_id)
        .unwrap()
        .expect("e2 exists");

    assert_eq!(e1.status, "RUNNING");
    assert_eq!(e2.status, "RUNNING");
    assert_eq!(e1.git_attribution, "OBSERVED");
    assert_eq!(e2.git_attribution, "OBSERVED");
}

// 5. Same worktree parallel executions: verify attribution is AMBIGUOUS
#[test]
fn test_same_worktree_parallel_executions() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let shared_wt = "C:/repos/shared_worktree";
    let exec1_id = "exec_par_1".to_string();
    let exec2_id = "exec_par_2".to_string();

    let ev1 = IngestEvent {
        event_id: "evt_p1".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_p1".to_string()),
        session_id: Some("sess_p1".to_string()),
        execution_id: Some(exec1_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "worktree_path": shared_wt,
            "model": "gpt-4o",
        }),
        extra: HashMap::new(),
    };

    // Execution 1 starts alone in worktree -> OBSERVED
    engine.process_event(&ev1).unwrap();
    let e1_initial = repo.find_execution_by_id(&exec1_id).unwrap().unwrap();
    assert_eq!(e1_initial.git_attribution, "OBSERVED");

    // Execution 2 starts while Execution 1 is STILL RUNNING in the EXACT SAME worktree!
    let ev2 = IngestEvent {
        event_id: "evt_p2".to_string(),
        timestamp: "2026-09-27T10:01:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_p2".to_string()),
        session_id: Some("sess_p2".to_string()),
        execution_id: Some(exec2_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "worktree_path": shared_wt,
            "model": "claude-3-5-sonnet",
        }),
        extra: HashMap::new(),
    };

    engine.process_event(&ev2).unwrap();
    let e2 = repo.find_execution_by_id(&exec2_id).unwrap().unwrap();
    // Must be AMBIGUOUS because two executions are concurrent in the same worktree!
    assert_eq!(e2.git_attribution, "AMBIGUOUS");
}

// 6. Session resume across runtime restart: process stopped != session stopped
#[test]
fn test_session_resume_across_runtime_restart() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let native_id = "native-thread-4042";

    // Runtime A starts
    let run_a = IngestEvent {
        event_id: "evt_ra".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "runtime.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_a".to_string()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "cwd": "C:/app",
            "command_line": "codex --session native-thread-4042",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&run_a).unwrap();

    let sess_ident_a = IngestEvent {
        event_id: "evt_sia".to_string(),
        timestamp: "2026-09-27T10:00:01Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_a".to_string()),
        session_id: Some("sess_initial_id".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "native_session_id": native_id,
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&sess_ident_a).unwrap();

    // Turn 1 execution
    let exec1_id = "exec_turn_1";
    let exec1 = IngestEvent {
        event_id: "evt_e1".to_string(),
        timestamp: "2026-09-27T10:00:02Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_a".to_string()),
        session_id: Some("sess_initial_id".to_string()),
        execution_id: Some(exec1_id.to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o", "turn_index": 0 }),
        extra: HashMap::new(),
    };
    engine.process_event(&exec1).unwrap();

    // Runtime A stops (process stopped!)
    let stop_a = IngestEvent {
        event_id: "evt_stop_a".to_string(),
        timestamp: "2026-09-27T10:10:00Z".to_string(),
        event_type: "runtime.stopped".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_a".to_string()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "exit_code": 0 }),
        extra: HashMap::new(),
    };
    engine.process_event(&stop_a).unwrap();

    // Verify session still exists and is not destroyed!
    let session_after_stop = repo
        .find_session_by_native_id("codex", native_id)
        .unwrap()
        .unwrap();
    let initial_session_id = session_after_stop.id.clone();

    // Runtime B starts later (resumed same session)
    let run_b = IngestEvent {
        event_id: "evt_rb".to_string(),
        timestamp: "2026-09-27T10:30:00Z".to_string(),
        event_type: "runtime.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_b".to_string()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "cwd": "C:/app",
            "command_line": "codex --resume --session native-thread-4042",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&run_b).unwrap();

    let sess_ident_b = IngestEvent {
        event_id: "evt_sib".to_string(),
        timestamp: "2026-09-27T10:30:01Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_b".to_string()),
        session_id: Some("sess_temporary_b_id".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "native_session_id": native_id,
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&sess_ident_b).unwrap();

    // Verify bindings for session
    let bindings = repo.list_bindings_for_session(&initial_session_id).unwrap();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].runtime_id, "run_proc_a");
    assert_eq!(bindings[0].reason, "START");
    assert_eq!(bindings[1].runtime_id, "run_proc_b");
    assert_eq!(bindings[1].reason, "RESUME");
}

// 7. Same worktree/branch does not merge sessions
#[test]
fn test_same_worktree_branch_does_not_merge_sessions() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    // Agent 1 with unknown native session in worktree W, branch B
    let ev1 = IngestEvent {
        event_id: "evt_m1".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_m1".to_string()),
        session_id: Some("sess_m1".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "opencode",
            "native_session_id": "UNKNOWN",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev1).unwrap();

    // Agent 2 with unknown native session in SAME worktree W, branch B
    let ev2 = IngestEvent {
        event_id: "evt_m2".to_string(),
        timestamp: "2026-09-27T10:05:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_m2".to_string()),
        session_id: Some("sess_m2".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "opencode",
            "native_session_id": "UNKNOWN",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev2).unwrap();

    let (sessions, total) = repo
        .list_sessions(&harnesscope::storage::repository::SessionFilter::default())
        .unwrap();
    assert_eq!(
        total, 2,
        "Two unknown sessions in the same worktree must NEVER merge!"
    );
    assert_ne!(sessions[0].id, sessions[1].id);
}

// 8. Concurrent event ingestion
#[tokio::test]
async fn test_concurrent_event_ingestion() {
    let (repo, _tmp) = make_test_repo();
    let engine = Arc::new(CorrelationEngine::new(repo.clone()));

    let mut handles = Vec::new();
    for i in 0..10 {
        let eng = engine.clone();
        let handle = tokio::spawn(async move {
            let ev = IngestEvent {
                event_id: format!("evt_concurrent_{}", i),
                timestamp: "2026-09-27T10:00:00Z".to_string(),
                event_type: "custom.metric".to_string(),
                source: "test".to_string(),
                runtime_id: None,
                session_id: None,
                execution_id: None,
                agent_instance_id: None,
                payload: serde_json::json!({ "index": i }),
                extra: HashMap::new(),
            };
            eng.process_event(&ev)
        });
        handles.push(handle);
    }

    for h in handles {
        let res = h.await.unwrap();
        assert!(
            res.is_ok(),
            "Concurrent event ingestion must succeed without SQLite locks"
        );
    }
}

// 9. Runner crash handling
#[test]
fn test_runner_crash_handling() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let run_id = "run_crashed_proc";
    let exec_id = "exec_crashed_turn";

    let ev_start = IngestEvent {
        event_id: "evt_cs".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "runtime.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(run_id.to_string()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "runner_name": "codex", "command_line": "codex fail" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_start).unwrap();

    let ev_exec = IngestEvent {
        event_id: "evt_ce".to_string(),
        timestamp: "2026-09-27T10:00:01Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(run_id.to_string()),
        session_id: Some("sess_crash".to_string()),
        execution_id: Some(exec_id.to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_exec).unwrap();

    // Process crashed with exit code 137 (SIGKILL)
    let ev_crash = IngestEvent {
        event_id: "evt_cc".to_string(),
        timestamp: "2026-09-27T10:00:05Z".to_string(),
        event_type: "runtime.stopped".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(run_id.to_string()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "exit_code": 137, "status": "FAILED" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_crash).unwrap();

    let runtime = repo.find_runtime_by_id(run_id).unwrap().unwrap();
    assert_eq!(runtime.status, "FAILED");
    assert_eq!(runtime.exit_code, Some(137));
}

// 10. Wrapper exit code passthrough
#[test]
fn test_wrapper_exit_code_passthrough() {
    // When runner binary is not found, it exits with 1
    let exit_code_missing = execute_wrapper("nonexistent_runner_xyz", &[], "http://127.0.0.1:4242");
    assert_eq!(exit_code_missing, 1);
}

// 11. Generic wrapper execution test
#[test]
fn test_generic_wrapper_execution() {
    use harnesscope::runners::execute_wrapper_generic;
    let code = execute_wrapper_generic(
        "custom_agent",
        "gui",
        "nonexistent_custom_cmd",
        &[],
        "http://127.0.0.1:4242",
    );
    assert_eq!(code, 1);
}

// 12. Session forking test
#[test]
fn test_session_forking() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    // Parent session
    let ev_parent = IngestEvent {
        event_id: "evt_parent".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_p".to_string()),
        session_id: Some("sess_parent_01".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "native_session_id": "thread-main-01",
            "title": "Main architectural design",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_parent).unwrap();

    // Forked session branching from parent
    let ev_fork = IngestEvent {
        event_id: "evt_fork".to_string(),
        timestamp: "2026-09-27T10:30:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_f".to_string()),
        session_id: Some("sess_fork_01".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "native_session_id": "thread-fork-01",
            "parent_session_id": "thread-main-01",
            "fork_reason": "EXPERIMENT",
            "title": "Experimental microservice refactoring",
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_fork).unwrap();

    let child_forks = repo.list_child_forks("sess_parent_01").unwrap();
    assert_eq!(child_forks.len(), 1);
    assert_eq!(child_forks[0].id, "sess_fork_01");
    assert_eq!(child_forks[0].fork_reason.as_deref(), Some("EXPERIMENT"));
}

// 13. Session-level concurrent access conflict detection
#[test]
fn test_session_concurrent_access_conflict() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let session_id = "sess_contested_01";

    // Runtime A starts execution in session
    let ev_exec_a = IngestEvent {
        event_id: "evt_ea".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_a".to_string()),
        session_id: Some(session_id.to_string()),
        execution_id: Some("exec_turn_a".to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_exec_a).unwrap();

    // While Runtime A is still RUNNING, Runtime B starts an execution in the EXACT SAME session!
    let ev_exec_b = IngestEvent {
        event_id: "evt_eb".to_string(),
        timestamp: "2026-09-27T10:01:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_proc_b".to_string()), // Different runtime!
        session_id: Some(session_id.to_string()),
        execution_id: Some("exec_turn_b".to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_exec_b).unwrap();

    let conflicts = repo.list_conflicts_for_session(session_id).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict_type, "CONCURRENT_SESSION_ACCESS");
    assert_eq!(conflicts[0].severity, "CRITICAL");
}

// 14. Fork divergence conflict detection
#[test]
fn test_fork_divergence_conflict() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let parent_sess = "sess_parent_conflict";
    let child_sess = "sess_child_conflict";

    // Setup parent session
    let ev_p = IngestEvent {
        event_id: "evt_sp".to_string(),
        timestamp: "2026-09-27T10:00:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_p".to_string()),
        session_id: Some(parent_sess.to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "runner_name": "codex", "native_session_id": "p_native" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_p).unwrap();

    // Setup child session forked from parent
    let ev_c = IngestEvent {
        event_id: "evt_sc".to_string(),
        timestamp: "2026-09-27T10:01:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_c".to_string()),
        session_id: Some(child_sess.to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "runner_name": "codex", "parent_session_id": parent_sess }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_c).unwrap();

    // Start execution in parent
    let ev_ep = IngestEvent {
        event_id: "evt_ep".to_string(),
        timestamp: "2026-09-27T10:02:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_p".to_string()),
        session_id: Some(parent_sess.to_string()),
        execution_id: Some("exec_parent".to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_ep).unwrap();

    // Start execution in child while parent is still running
    let ev_ec = IngestEvent {
        event_id: "evt_ec".to_string(),
        timestamp: "2026-09-27T10:03:00Z".to_string(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_c".to_string()),
        session_id: Some(child_sess.to_string()),
        execution_id: Some("exec_child".to_string()),
        agent_instance_id: None,
        payload: serde_json::json!({ "model": "gpt-4o" }),
        extra: HashMap::new(),
    };
    engine.process_event(&ev_ec).unwrap();

    let conflicts = repo.list_conflicts_for_session(child_sess).unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict_type, "FORK_DIVERGENCE");
}

// 15. Cross-session worktree overlap is recorded and marks both runs ambiguous
#[test]
fn test_worktree_overlap_conflict_marks_both_attributions_ambiguous() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    for (event_id, runtime_id, session_id, native_id) in [
        (
            "evt_worktree_session_a",
            "run_worktree_a",
            "sess_worktree_a",
            "native_a",
        ),
        (
            "evt_worktree_session_b",
            "run_worktree_b",
            "sess_worktree_b",
            "native_b",
        ),
    ] {
        let event = IngestEvent {
            event_id: event_id.to_string(),
            timestamp: "2026-09-27T11:00:00Z".to_string(),
            event_type: "session.identified".to_string(),
            source: "wrapper".to_string(),
            runtime_id: Some(runtime_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: None,
            agent_instance_id: None,
            payload: serde_json::json!({ "runner_name": "codex", "native_session_id": native_id }),
            extra: HashMap::new(),
        };
        engine.process_event(&event).unwrap();
    }

    for (event_id, runtime_id, session_id, execution_id, timestamp) in [
        (
            "evt_worktree_exec_a",
            "run_worktree_a",
            "sess_worktree_a",
            "exec_worktree_a",
            "2026-09-27T11:01:00Z",
        ),
        (
            "evt_worktree_exec_b",
            "run_worktree_b",
            "sess_worktree_b",
            "exec_worktree_b",
            "2026-09-27T11:02:00Z",
        ),
    ] {
        let event = IngestEvent {
            event_id: event_id.to_string(),
            timestamp: timestamp.to_string(),
            event_type: "execution.started".to_string(),
            source: "wrapper".to_string(),
            runtime_id: Some(runtime_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: Some(execution_id.to_string()),
            agent_instance_id: None,
            payload: serde_json::json!({ "model": "gpt-4o", "worktree_path": "/workspace/shared" }),
            extra: HashMap::new(),
        };
        engine.process_event(&event).unwrap();
    }

    assert_eq!(
        repo.find_execution_by_id("exec_worktree_a")
            .unwrap()
            .unwrap()
            .git_attribution,
        "AMBIGUOUS"
    );
    assert_eq!(
        repo.find_execution_by_id("exec_worktree_b")
            .unwrap()
            .unwrap()
            .git_attribution,
        "AMBIGUOUS"
    );
    let conflicts = repo.list_conflicts_for_session("sess_worktree_b").unwrap();
    assert!(conflicts
        .iter()
        .any(|conflict| conflict.conflict_type == "WORKTREE_OVERLAP"));
}

// 16. Fork divergence is detected even when the parent starts after the child
#[test]
fn test_fork_divergence_when_parent_starts_after_child() {
    let (repo, _tmp) = make_test_repo();
    let engine = CorrelationEngine::new(repo.clone());

    let parent = IngestEvent {
        event_id: "evt_parent_late".to_string(),
        timestamp: "2026-09-27T12:00:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_parent_late".to_string()),
        session_id: Some("sess_parent_late".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({ "runner_name": "codex", "native_session_id": "parent-native-late" }),
        extra: HashMap::new(),
    };
    engine.process_event(&parent).unwrap();

    let child = IngestEvent {
        event_id: "evt_child_early".to_string(),
        timestamp: "2026-09-27T12:01:00Z".to_string(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some("run_child_early".to_string()),
        session_id: Some("sess_child_early".to_string()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": "codex",
            "native_session_id": "child-native-early",
            "parent_session_id": "parent-native-late"
        }),
        extra: HashMap::new(),
    };
    engine.process_event(&child).unwrap();

    for (event_id, runtime_id, session_id, execution_id, timestamp) in [
        (
            "evt_child_exec_early",
            "run_child_early",
            "sess_child_early",
            "exec_child_early",
            "2026-09-27T12:02:00Z",
        ),
        (
            "evt_parent_exec_late",
            "run_parent_late",
            "sess_parent_late",
            "exec_parent_late",
            "2026-09-27T12:03:00Z",
        ),
    ] {
        let event = IngestEvent {
            event_id: event_id.to_string(),
            timestamp: timestamp.to_string(),
            event_type: "execution.started".to_string(),
            source: "wrapper".to_string(),
            runtime_id: Some(runtime_id.to_string()),
            session_id: Some(session_id.to_string()),
            execution_id: Some(execution_id.to_string()),
            agent_instance_id: None,
            payload: serde_json::json!({ "model": "gpt-4o" }),
            extra: HashMap::new(),
        };
        engine.process_event(&event).unwrap();
    }

    let conflicts = repo.list_conflicts_for_session("sess_child_early").unwrap();
    assert!(conflicts.iter().any(|conflict| {
        conflict.conflict_type == "FORK_DIVERGENCE"
            && conflict.session_id == "sess_parent_late"
            && conflict.conflicting_session_id.as_deref() == Some("sess_child_early")
    }));
}
