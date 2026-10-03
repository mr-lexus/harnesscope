use harnesscope::{
    capture,
    storage::{task_adapters::TaskAdapter, Database, Repository},
};
use serde_json::{json, Value};

fn adapter(id: &str, namespace: &str, tool: &str, source: &str, pointer: &str) -> TaskAdapter {
    serde_json::from_value(json!({"schema_version":1,"id":id,"namespace":namespace,"tool":tool,"task_id":{"source":source,"pointer":pointer},"title_pointer":"/title"})).unwrap()
}
fn record(repo: &Repository, position: &str, session: &str, turn: &str, payload: Value) {
    let value = json!({"type":"response_item","payload":payload});
    let mut input = capture::from_record("test", "synthetic", position, &value);
    input.session_id = Some(session.into());
    input.turn_id = Some(turn.into());
    repo.record_observation(&input, &capture::sanitize(&value))
        .unwrap();
}
fn call(repo: &Repository, pos: &str, tool: &str, args: Value) {
    record(
        repo,
        pos,
        "session",
        "turn",
        json!({"type":"function_call","call_id":"call","name":tool,"arguments":args}),
    );
}
fn result(repo: &Repository, pos: &str, value: Value) {
    record(
        repo,
        pos,
        "session",
        "turn",
        json!({"type":"function_call_output","call_id":"call","output":value}),
    );
}

#[test]
fn opt_in_exact_matching_and_reindex_preserve_human_outcomes() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    call(
        &repo,
        "1",
        "mcp__tracker__read",
        json!("{\"taskId\":\"A-1\"}"),
    );
    result(
        &repo,
        "2",
        json!({"title":"First","description":"Version one"}),
    );
    assert_eq!(repo.retro_tasks("", 20).unwrap()["items"], json!([]));
    let config = adapter("read", "team", "mcp__tracker__read", "arguments", "/taskId");
    repo.register_task_adapter(&config).unwrap();
    repo.reindex_evidence().unwrap();
    repo.mark_retro_task("mcp:team:A-1", "accepted", "Human approval")
        .unwrap();
    result(
        &repo,
        "3",
        json!({"description":"Updated","comments":["Changed requirement"]}),
    );
    result(
        &repo,
        "3",
        json!({"description":"Updated","comments":["Changed requirement"]}),
    );
    repo.reindex_evidence().unwrap();
    let detail = repo.retro_task_detail("mcp:team:A-1").unwrap();
    assert_eq!(detail["versions"].as_array().unwrap().len(), 2);
    assert_eq!(detail["marks"][0]["kind"], "accepted");
    assert!(repo.disable_task_adapter("read").unwrap());
    result(&repo, "4", json!({"description":"Disabled"}));
    assert_eq!(repo.retro_task_detail("mcp:team:A-1").unwrap(), detail);
    repo.register_task_adapter(&config).unwrap();
    call(
        &repo,
        "5",
        "mcp__tracker__read_extra",
        json!({"taskId":"A-1"}),
    );
    result(
        &repo,
        "6",
        json!({"description":"Wrong tool, reused call ID"}),
    );
    assert_eq!(repo.retro_task_detail("mcp:team:A-1").unwrap(), detail);
}

#[test]
fn result_ids_support_mcp_envelopes_and_separate_namespaces() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    for (n, tool) in [("one", "mcp__one__read"), ("two", "mcp__two__read")] {
        repo.register_task_adapter(&adapter(n, n, tool, "result", "/id"))
            .unwrap();
        call(&repo, &format!("{n}-call"), tool, json!({}));
        let body = json!({"id":42,"title":"Task","password":"canary-never-retain"});
        let output = if n == "one" {
            json!({"structuredContent":body})
        } else {
            json!({"content":[{"type":"text","text":body.to_string()}]})
        };
        result(&repo, &format!("{n}-result"), json!(output.to_string()));
    }
    assert_eq!(
        repo.retro_tasks("", 20).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for n in ["one", "two"] {
        let detail = repo.retro_task_detail(&format!("mcp:{n}:42")).unwrap();
        assert_eq!(detail["versions"].as_array().unwrap().len(), 1);
        let hash = detail["versions"][0]["object_hash"].as_str().unwrap();
        assert!(!repo
            .evidence_object(hash)
            .unwrap()
            .unwrap()
            .to_string()
            .contains("canary-never-retain"));
    }
}

#[test]
fn errors_missing_ids_and_unrelated_calls_never_confirm_tasks() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.register_task_adapter(&adapter(
        "read",
        "team",
        "mcp__tracker__read",
        "arguments",
        "/id",
    ))
    .unwrap();
    call(&repo, "1", "mcp__tracker__read", json!({"id":"A-1"}));
    result(
        &repo,
        "2",
        json!({"isError":true,"content":[{"type":"text","text":"Missing task"}]}),
    );
    result(&repo, "3", json!({"error":{"message":"Access denied"}}));
    record(
        &repo,
        "4",
        "other-session",
        "turn",
        json!({"type":"function_call_output","call_id":"call","output":{"title":"Wrong session"}}),
    );
    record(
        &repo,
        "5",
        "session",
        "other-turn",
        json!({"type":"function_call_output","call_id":"call","output":{"title":"Wrong turn"}}),
    );
    call(
        &repo,
        "6",
        "mcp__tracker__read",
        json!({"id":"[REDACTED_SECRET]"}),
    );
    result(&repo, "7", json!({"title":"Unsafe identity"}));
    call(&repo, "8", "mcp__tracker__read", json!({"mention":"A-1"}));
    result(&repo, "9", json!({"title":"Missing identity"}));
    assert_eq!(repo.retro_tasks("", 20).unwrap()["items"], json!([]));
}

#[test]
fn mappings_are_validated_immutable_and_unambiguous() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    let first = adapter(
        "read",
        "team",
        "mcp__tracker__read",
        "result",
        "/data/a~1b/0/~0id",
    );
    repo.register_task_adapter(&first).unwrap();
    repo.register_task_adapter(&first).unwrap();
    let mut other = first.clone();
    other.namespace = "other".into();
    assert!(repo.register_task_adapter(&other).is_err());
    other.id = "duplicate-tool".into();
    assert!(repo.register_task_adapter(&other).is_err());
    for pointer in ["no-slash", "/~bad", "/~"] {
        let mut bad = first.clone();
        bad.task_id.pointer = pointer.into();
        assert!(repo.register_task_adapter(&bad).is_err());
    }
    assert!(serde_json::from_value::<TaskAdapter>(json!({"schema_version":1,"id":"x","namespace":"x","tool":"read","task_id":{"source":"arguments","pointer":"/id"},"api_key":"secret"})).is_err());
    call(&repo, "1", "mcp__tracker__read", json!({}));
    result(&repo, "2", json!({"data":{"a/b":[{"~id":"A-1"}]}}));
    assert_eq!(
        repo.retro_tasks("", 20).unwrap()["items"][0]["id"],
        "mcp:team:A-1"
    );
}

#[test]
fn post_tool_hook_contains_its_own_proven_invocation() {
    let repo = Repository::new(Database::open_in_memory().unwrap());
    repo.register_task_adapter(&adapter(
        "read",
        "team",
        "mcp__tracker__read",
        "arguments",
        "/id",
    ))
    .unwrap();
    // The pre-tool hook is absent. Both invocation and response are explicit.
    record(
        &repo,
        "post",
        "session",
        "turn",
        json!({"hook_event_name":"PostToolUse","tool_use_id":"hook-call","tool_name":"mcp__tracker__read","tool_input":{"id":"A-1"},"tool_response":{"title":"Read through hook"}}),
    );
    let detail = repo.retro_task_detail("mcp:team:A-1").unwrap();
    assert_eq!(detail["versions"].as_array().unwrap().len(), 1);
    assert_eq!(detail["links"][0]["status"], "confirmed");
    repo.reindex_evidence().unwrap();
    assert_eq!(repo.retro_task_detail("mcp:team:A-1").unwrap(), detail);
}

#[test]
fn schema6_migration_backs_up_and_adapters_survive_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.db");
    {
        let db = Database::open(&path).unwrap();
        db.with_conn(|c| {
            c.execute_batch(
                "DROP TABLE task_adapters; DROP INDEX evidence_items_call; DELETE FROM _schema_migrations WHERE version=7;",
            )
        })
        .unwrap();
    }
    let repo = Repository::new(Database::open(&path).unwrap());
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("before-v7")));
    repo.register_task_adapter(&adapter(
        "read",
        "team",
        "mcp__tracker__read",
        "arguments",
        "/id",
    ))
    .unwrap();
    let backup = dir.path().join("snapshot.db");
    harnesscope::storage::backup::create(&path, &backup).unwrap();
    let restored = dir.path().join("restored.db");
    harnesscope::storage::backup::restore(&backup, &restored).unwrap();
    let restored = Repository::new(Database::open(&restored).unwrap());
    assert_eq!(
        repo.task_adapters().unwrap(),
        restored.task_adapters().unwrap()
    );
}
