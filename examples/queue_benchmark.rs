//! cargo run --example queue_benchmark -- 100000
use harnesscope::{
    domain::events::IngestEvent,
    outbox::Outbox,
    server::correlation::CorrelationEngine,
    storage::{Database, Repository},
};
use std::{sync::Arc, time::Instant};
fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or("10000".into())
        .parse()
        .unwrap();
    assert!((100..=500_000).contains(&count));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queue.db");
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    let started = Instant::now();
    let mut windows = Vec::new();
    let mut window = Instant::now();
    for start in (0..count).step_by(100) {
        let events: Vec<IngestEvent> = (start..(start+100).min(count)).map(|i| serde_json::from_value(serde_json::json!({
            "event_id":format!("bench-{i}"),"event_type":"benchmark.observed","source":"synthetic", "payload":{"index":i,"text":"x".repeat(128)}
        })).unwrap()).collect();
        q.enqueue("benchmark", &events).unwrap();
        if (start + 100) % 10000 == 0 {
            windows.push(window.elapsed().as_secs_f64());
            window = Instant::now();
        }
    }
    let enqueue_seconds = started.elapsed().as_secs_f64();
    drop(q);
    let q = Outbox::open(&path, "http://127.0.0.1:4242").unwrap();
    assert_eq!(q.snapshot().unwrap().pending_events, count as i64);
    let repo = Arc::new(Repository::new(Database::open(&path).unwrap()));
    let engine = CorrelationEngine::new(repo);
    let started = Instant::now();
    while q.deliver_local(&engine).unwrap() {}
    let delivery_seconds = started.elapsed().as_secs_f64();
    assert_eq!(q.snapshot().unwrap().pending_events, 0);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let stored: usize = conn
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, count);
    println!(
        "{}",
        serde_json::json!({"events":count,"enqueue_seconds":enqueue_seconds,"enqueue_10k_windows_seconds":windows,"recovery_seconds":delivery_seconds,"stored_events":stored,"profile":"debug; 100-event enqueue; 128-byte payload; one offline stream; restart then local delivery"})
    );
}
