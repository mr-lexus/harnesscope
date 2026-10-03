use harnesscope::storage::{backup, Database, Repository};
use rusqlite::Connection;
use std::process::Command;

#[test]
fn snapshot_includes_committed_wal_and_restores_reviews_with_collectors_paused() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.db");
    let db = Database::open(&source).unwrap();
    let repo = Repository::new(db.clone());
    harnesscope::demo::seed_demo_data(&repo).unwrap();
    db.with_conn(|c| c.execute_batch("INSERT INTO telemetry_sources(id,adapter,path,created_at) VALUES('source','codex','/fixture',datetime('now'));
        INSERT INTO execution_reviews(execution_id,outcome,notes,experiment,updated_at) SELECT id,'ACCEPTED','keep this review','baseline',datetime('now') FROM executions LIMIT 1;
        INSERT INTO source_files(source_id,path,byte_offset,line_number,updated_at) VALUES('source','/fixture/rollout.jsonl',123,2,datetime('now'));" )).unwrap();
    assert!(dir.path().join("live.db-wal").metadata().unwrap().len() > 0);
    let snapshot = dir.path().join("snapshot.db");
    let report = backup::create(&source, &snapshot).unwrap();
    assert!(report.events > 0);
    assert_eq!(report.reviews, 1);
    assert_eq!(report.enabled_sources, 1);
    // A writer can keep an uncommitted transaction while the WAL snapshot is made.
    db.with_conn(|c| c.execute_batch("BEGIN IMMEDIATE; DELETE FROM execution_reviews;"))
        .unwrap();
    let second = dir.path().join("second.db");
    assert_eq!(backup::create(&source, &second).unwrap().reviews, 1);
    db.with_conn(|c| c.execute_batch("ROLLBACK")).unwrap();
    let output = dir.path().join("restored.db");
    let restored = backup::restore(&snapshot, &output).unwrap();
    assert_eq!(restored.events, report.events);
    assert_eq!(restored.reviews, 1);
    assert_eq!(restored.enabled_sources, 0);
    let c = Connection::open(&output).unwrap();
    assert_eq!(
        c.query_row("SELECT byte_offset FROM source_files", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        123
    );
    assert_eq!(
        c.query_row("SELECT notes FROM execution_reviews", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "keep this review"
    );
    assert_eq!(
        backup::verify(&snapshot).unwrap().enabled_sources,
        1,
        "restore must not modify the backup"
    );
    assert!(!dir.path().join("snapshot.db-wal").exists());
}

#[test]
fn refuses_overwrite_missing_corrupt_foreign_and_future_databases() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.db");
    let _db = Database::open(&source).unwrap();
    let output = dir.path().join("existing.db");
    std::fs::write(&output, b"must survive").unwrap();
    assert!(backup::create(&source, &output).is_err());
    assert!(backup::restore(&source, &output).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"must survive");
    assert!(backup::verify(&output).is_err());
    let missing = dir.path().join("missing.db");
    assert!(backup::verify(&missing).is_err());
    assert!(backup::create(&missing, &dir.path().join("absent.db")).is_err());
    assert!(!missing.exists());
    let foreign = dir.path().join("foreign.db");
    Connection::open(&foreign)
        .unwrap()
        .execute_batch("CREATE TABLE x(id INTEGER)")
        .unwrap();
    assert!(backup::create(&foreign, &dir.path().join("bad.db")).is_err());
    assert!(!dir.path().join("bad.db").exists());
    let conn = Connection::open(&source).unwrap();
    conn.execute(
        "INSERT INTO _schema_migrations(version,applied_at) VALUES(999,'future')",
        [],
    )
    .unwrap();
    assert!(backup::verify(&source).unwrap_err().contains("Unsupported"));
    conn.execute("DELETE FROM _schema_migrations WHERE version=999", [])
        .unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF; INSERT INTO session_aliases(alias,session_id) VALUES('orphan','missing');").unwrap();
    assert!(backup::verify(&source).unwrap_err().contains("Foreign-key"));
}

#[test]
fn cli_create_verify_restore_and_failure_exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.db");
    let _db = Database::open(&source).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_harnesscope"))
            .args(args)
            .current_dir(dir.path())
            .env("HARNESSCOPE_DB_PATH", &source)
            .output()
            .unwrap()
    };
    for args in [
        vec!["backup", "create", "--output", "copy.db"],
        vec!["backup", "verify", "--file", "copy.db"],
        vec![
            "backup",
            "restore",
            "--file",
            "copy.db",
            "--output",
            "restore.db",
        ],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["integrity"], "ok");
        assert_eq!(json["scope"], "main_database");
    }
    assert!(!run(&["backup", "create", "--output", "copy.db"])
        .status
        .success());
    assert!(!run(&["backup", "verify", "--file", "missing.db"])
        .status
        .success());
}

#[test]
fn concurrent_writes_are_copied_at_transaction_boundaries() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("live.db");
    let _db = Database::open(&source).unwrap();
    let stop = AtomicBool::new(false);
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut conn = Connection::open(&source).unwrap();
            conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
            let mut batch = 0;
            while !stop.load(Ordering::Relaxed) {
                let tx = conn.transaction().unwrap();
                for part in 0..2 {
                    tx.execute("INSERT INTO events(event_id,timestamp,event_type,source,payload_json) VALUES(?1,'2026-01-01','test','fixture','{}')",[format!("{batch}:{part}")]).unwrap();
                }
                tx.commit().unwrap();
                if batch==0 { ready_tx.send(()).unwrap(); }
                batch+=1;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        ready_rx.recv().unwrap();
        let report = backup::create(&source, &dir.path().join("snapshot.db"));
        stop.store(true, Ordering::Relaxed);
        let report = report.unwrap();
        assert!(report.events >= 2);
        assert_eq!(report.events % 2, 0);
    });
}
