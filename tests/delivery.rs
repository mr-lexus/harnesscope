use harnesscope::{
    domain::events::IngestEvent,
    outbox::Outbox,
    server::correlation::CorrelationEngine,
    storage::{Database, Repository},
};
use std::sync::Arc;
fn event(id: &str, kind: &str) -> IngestEvent {
    serde_json::from_value(serde_json::json!({"event_id":id,"event_type":kind,"source":"wrapper","runtime_id":"runtime","payload":{"runner_name":"test","cwd":"/tmp"}})).unwrap()
}

#[test]
fn queue_size_migrates_existing_backlog_and_tracks_utf8_rollback_and_ack() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queue.db");
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    let mut e = event("unicode", "runtime.started");
    e.payload["text"] = "Привет 🦀".into();
    q.enqueue("r", &[e]).unwrap();
    let c = rusqlite::Connection::open(dir.path().join("queue.db.outbox.sqlite3")).unwrap();
    // Model an outbox created by the preceding version; opening must backfill once.
    c.execute_batch("DROP TRIGGER queue_size_insert; DROP TRIGGER queue_size_delete; DROP TRIGGER queue_size_update; DROP TABLE queue_usage;").unwrap();
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    let check = || {
        let (counter,actual): (i64,i64) = c.query_row("SELECT payload_bytes,(SELECT COALESCE(SUM(length(CAST(payload AS BLOB))),0) FROM batches) FROM queue_usage",[],|r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(counter, actual);
        actual
    };
    let before = check();
    assert!(before > 0);
    c.execute_batch("BEGIN; UPDATE batches SET payload='temporary'; ROLLBACK;")
        .unwrap();
    assert_eq!(check(), before);
    q.enqueue("r", &[event("next", "runtime.stopped")]).unwrap();
    assert!(check() > before);
    let claim = q.claim().unwrap().unwrap();
    q.ack(&claim).unwrap();
    check();
    let claim = q.claim().unwrap().unwrap();
    q.ack(&claim).unwrap();
    assert_eq!(check(), 0);
    // The budget check must reject a new batch without evicting or changing existing rows.
    c.execute("UPDATE queue_usage SET payload_bytes=268435456", [])
        .unwrap();
    assert!(q
        .enqueue("r", &[event("full", "runtime.started")])
        .unwrap_err()
        .contains("full"));
    assert!(q.pending().unwrap().is_empty());
}

#[test]
fn temporary_writer_contention_does_not_drop_durable_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("contended.db");
    let queue = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    let connection =
        rusqlite::Connection::open(dir.path().join("contended.db.outbox.sqlite3")).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(900));
        connection.execute_batch("COMMIT").unwrap();
    });
    // Previously a 500 ms busy timeout rejected this durable write outright.
    queue
        .enqueue("r", &[event("contended", "runtime.started")])
        .unwrap();
    release.join().unwrap();
    assert_eq!(queue.snapshot().unwrap().pending_events, 1);
    let claim = queue.claim().unwrap().unwrap();
    assert_eq!(claim.events[0].event_id, "contended");
    queue.ack(&claim).unwrap();
    assert_eq!(queue.snapshot().unwrap().pending_events, 0);
}

#[test]
fn concurrent_producers_and_consumers_preserve_per_stream_order() {
    use std::sync::Mutex;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queue.db");
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    std::thread::scope(|scope| {
        for producer in 0..8 {
            let path = &path;
            scope.spawn(move || {
                let q = Outbox::open(path, "http://127.0.0.1:4242").unwrap();
                for part in 0..4 {
                    let events: Vec<_> = (part * 100..(part + 1) * 100)
                        .map(|i| event(&format!("{producer}:{i}"), "benchmark.observed"))
                        .collect();
                    q.enqueue(&producer.to_string(), &events).unwrap();
                }
            });
        }
    });
    assert_eq!(q.snapshot().unwrap().pending_events, 3200);
    let observed = Mutex::new(vec![Vec::new(); 8]);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let q = &q;
            let observed = &observed;
            scope.spawn(move || {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                loop {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "concurrent delivery timed out"
                    );
                    if let Some(claim) = q.claim().unwrap() {
                        for e in &claim.events {
                            let (producer, index) = e.event_id.split_once(':').unwrap();
                            observed.lock().unwrap()[producer.parse::<usize>().unwrap()]
                                .push(index.parse::<usize>().unwrap());
                        }
                        q.ack(&claim).unwrap();
                    } else if q.snapshot().unwrap().pending_events == 0 {
                        break;
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            });
        }
    });
    for stream in observed.into_inner().unwrap() {
        assert_eq!(stream, (0..400).collect::<Vec<_>>());
    }
}
#[test]
fn queue_is_durable_redacted_and_bound_to_destination() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.db");
    let q = Outbox::open(&path, "http://localhost:4242").unwrap();
    let mut e = event("start", "runtime.started");
    e.payload["token"] = "secret-on-disk".into();
    e.extra.insert("password".into(), "extension-secret".into());
    q.enqueue("runtime", &[e]).unwrap();
    drop(q);
    assert!(Outbox::open(&path, "http://127.0.0.1:4243")
        .unwrap()
        .claim()
        .unwrap()
        .is_none());
    let q = Outbox::open(&path, "http://127.0.0.1:4242/").unwrap();
    let claim = q.claim().unwrap().unwrap();
    let json = serde_json::to_string(&claim.events).unwrap();
    assert!(!json.contains("secret-on-disk"));
    assert!(!json.contains("extension-secret"));
    let c = rusqlite::Connection::open(temp.path().join("test.db.outbox.sqlite3")).unwrap();
    let raw: String = c
        .query_row("SELECT payload FROM batches", [], |r| r.get(0))
        .unwrap();
    assert!(!raw.contains("secret-on-disk"));
    q.ack(&claim).unwrap();
    assert!(q.pending().unwrap().is_empty());
}
#[test]
fn claims_keep_stream_order_and_do_not_block_other_runtimes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.db");
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    q.enqueue("a", &[event("a1", "runtime.started")]).unwrap();
    q.enqueue("a", &[event("a2", "runtime.stopped")]).unwrap();
    q.enqueue("b", &[event("b1", "runtime.started")]).unwrap();
    let summary = q.snapshot().unwrap();
    assert_eq!(summary.pending_batches, 3);
    assert_eq!(summary.pending_events, 3);
    assert_eq!(summary.batches.len(), 3);
    assert!(q.has_pending_stream("a").unwrap());
    assert!(!q.has_pending_stream("missing").unwrap());
    let a = q.claim().unwrap().unwrap();
    let b = q.claim().unwrap().unwrap();
    assert_eq!(a.events[0].event_id, "a1");
    assert_eq!(b.events[0].event_id, "b1");
    assert!(q.claim().unwrap().is_none());
    q.fail(&a, "bad payload", true).unwrap();
    q.ack(&b).unwrap();
    assert!(q.claim().unwrap().is_none());
    drop(q);
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    assert!(q.pending().unwrap()[0].blocked);
    q.retry().unwrap();
    let retry = q.claim().unwrap().unwrap();
    assert_eq!(retry.events[0].event_id, "a1");
    q.ack(&a).unwrap();
    assert_eq!(
        q.pending().unwrap().len(),
        2,
        "obsolete lease must not delete a reclaimed batch"
    );
    q.ack(&retry).unwrap();
    assert_eq!(q.claim().unwrap().unwrap().events[0].event_id, "a2");
}
#[test]
fn lost_ack_replays_without_duplicate_projections() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("test.db");
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    q.enqueue("runtime", &[event("start", "runtime.started")])
        .unwrap();
    let first = q.claim().unwrap().unwrap();
    engine
        .process_events_with(&first.events, || Ok(()))
        .unwrap();
    // Crash after commit and before acknowledgement: lease expires on disk.
    rusqlite::Connection::open(temp.path().join("test.db.outbox.sqlite3"))
        .unwrap()
        .execute("UPDATE batches SET lease_until=0", [])
        .unwrap();
    assert!(q.deliver_local(&engine).unwrap());
    assert!(q.pending().unwrap().is_empty());
    assert!(repo.find_runtime_by_id("runtime").unwrap().is_some());
}
#[test]
fn heartbeat_uses_observation_time_and_never_reopens_closed_process() {
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    engine
        .process_event(&event("start", "runtime.started"))
        .unwrap();
    assert_eq!(repo.observations().unwrap()[0].freshness, "UNMONITORED");
    let mut beat = event("beat", "runtime.heartbeat");
    beat.timestamp = (chrono::Utc::now() - chrono::Duration::seconds(100)).to_rfc3339();
    engine.process_event(&beat).unwrap();
    assert_eq!(repo.observations().unwrap()[0].freshness, "STALE");
    beat.timestamp = chrono::Utc::now().to_rfc3339();
    engine.process_event(&beat).unwrap();
    assert_eq!(repo.observations().unwrap()[0].freshness, "FRESH");
    beat.timestamp = (chrono::Utc::now() - chrono::Duration::seconds(200)).to_rfc3339();
    engine.process_event(&beat).unwrap();
    assert_eq!(repo.observations().unwrap()[0].freshness, "FRESH");
    engine
        .process_event(&event("end", "runtime.stopped"))
        .unwrap();
    beat.timestamp = chrono::Utc::now().to_rfc3339();
    engine.process_event(&beat).unwrap();
    assert_eq!(repo.observations().unwrap()[0].freshness, "ENDED");
}
#[test]
fn delivery_rolls_back_whole_batch_when_relationship_is_invalid() {
    let repo = Arc::new(Repository::new(Database::open_in_memory().unwrap()));
    let engine = CorrelationEngine::new(repo.clone());
    let mut bad = event("beat", "runtime.heartbeat");
    bad.runtime_id = Some("missing".into());
    assert!(engine
        .process_events_with(&[event("start", "runtime.started"), bad], || Ok(()))
        .is_err());
    assert!(repo.find_runtime_by_id("runtime").unwrap().is_none());
}

#[test]
fn http_success_without_exact_ack_keeps_events_on_disk() {
    use std::io::{BufRead, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut reader = std::io::BufReader::new(&mut socket);
        let mut size = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                size = value.trim().parse::<usize>().unwrap();
            }
        }
        reader.read_exact(&mut vec![0; size]).unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"event_ids\":[]}").unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let queue = Outbox::open(&temp.path().join("ack.db"), &url).unwrap();
    queue
        .enqueue("runtime", &[event("start", "runtime.started")])
        .unwrap();
    assert!(queue.deliver_http().unwrap());
    server.join().unwrap();
    let pending = queue.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].attempts, 1);
    assert_eq!(
        pending[0].last_error.as_deref(),
        Some("Incomplete delivery acknowledgement")
    );
    assert!(
        queue.claim().unwrap().is_none(),
        "retry backoff must persist"
    );
}
