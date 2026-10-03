//! Durable, redacted delivery batches. A lease serializes each runtime stream;
//! server acknowledgement may be lost, so consumers must deduplicate event IDs.
use crate::{domain::events::IngestEvent, redact::redact_value};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone)]
pub struct Outbox {
    path: PathBuf,
    destination: String,
    stream: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct PendingBatch {
    pub id: i64,
    pub stream: String,
    pub events: i64,
    pub created_at: i64,
    pub attempts: i64,
    pub next_attempt: i64,
    pub blocked: bool,
    pub last_error: Option<String>,
}
#[derive(Default, Serialize)]
pub struct QueueSnapshot {
    pub pending_events: i64,
    pub pending_batches: i64,
    pub blocked_batches: i64,
    pub batches: Vec<PendingBatch>,
}
pub struct Claim {
    pub id: i64,
    token: String,
    pub events: Vec<IngestEvent>,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Outbox {
    pub fn open(db_path: &Path, destination: &str) -> Result<Self, String> {
        let mut url = crate::config::local_server_url(destination)?;
        if url.host_str() == Some("localhost") {
            url.set_host(Some("127.0.0.1")).map_err(|e| e.to_string())?;
        }
        let mut path = db_path.as_os_str().to_owned();
        path.push(".outbox.sqlite3");
        let this = Self {
            path: path.into(),
            destination: url.origin().ascii_serialization(),
            stream: None,
        };
        if let Some(parent) = this.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        this.conn()?
            .execute_batch(
                "PRAGMA journal_mode=WAL;
            BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS batches (
              id INTEGER PRIMARY KEY AUTOINCREMENT, destination TEXT NOT NULL,
              stream TEXT NOT NULL, payload TEXT NOT NULL, events INTEGER NOT NULL,
              created_at INTEGER NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
              next_attempt INTEGER NOT NULL DEFAULT 0, blocked INTEGER NOT NULL DEFAULT 0,
              last_error TEXT, lease_until INTEGER NOT NULL DEFAULT 0, token TEXT);
            CREATE INDEX IF NOT EXISTS batch_order ON batches(destination,stream,id);
            CREATE TABLE IF NOT EXISTS queue_usage (
                id INTEGER PRIMARY KEY CHECK(id=1), payload_bytes INTEGER NOT NULL CHECK(payload_bytes>=0));
            INSERT INTO queue_usage(id,payload_bytes)
                SELECT 1,(SELECT COALESCE(SUM(length(CAST(payload AS BLOB))),0) FROM batches)
                WHERE NOT EXISTS(SELECT 1 FROM queue_usage WHERE id=1);
            CREATE TRIGGER IF NOT EXISTS queue_size_insert AFTER INSERT ON batches BEGIN
                UPDATE queue_usage SET payload_bytes=payload_bytes+length(CAST(NEW.payload AS BLOB)) WHERE id=1;
            END;
            CREATE TRIGGER IF NOT EXISTS queue_size_delete AFTER DELETE ON batches BEGIN
                UPDATE queue_usage SET payload_bytes=payload_bytes-length(CAST(OLD.payload AS BLOB)) WHERE id=1;
            END;
            CREATE TRIGGER IF NOT EXISTS queue_size_update AFTER UPDATE OF payload ON batches BEGIN
                UPDATE queue_usage SET payload_bytes=payload_bytes+length(CAST(NEW.payload AS BLOB))-length(CAST(OLD.payload AS BLOB)) WHERE id=1;
            END;
            COMMIT;",
            )
            .map_err(|e| e.to_string())?;
        Ok(this)
    }
    pub fn for_stream(mut self, stream: &str) -> Self {
        self.stream = Some(stream.to_string());
        self
    }
    fn conn(&self) -> Result<Connection, String> {
        let c = Connection::open(&self.path).map_err(|e| e.to_string())?;
        c.busy_timeout(Duration::from_millis(500))
            .map_err(|e| e.to_string())?;
        c.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        Ok(c)
    }
    pub fn enqueue(&self, stream: &str, events: &[IngestEvent]) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        let mut chunks = Vec::new();
        let mut chunk = Vec::new();
        let mut bytes = 2;
        for event in events {
            event.validate()?;
            // Redact before the first durable write, including extension fields.
            let safe = redact_value(&serde_json::to_value(event).map_err(|e| e.to_string())?);
            let size = serde_json::to_vec(&safe).map_err(|e| e.to_string())?.len() + 1;
            if size > 1024 * 1024 {
                return Err("Event exceeds the 1 MiB outbox limit".into());
            }
            if bytes + size > 1024 * 1024 || chunk.len() == 100 {
                chunks.push(serde_json::to_string(&chunk).unwrap());
                chunk.clear();
                bytes = 2;
            }
            chunk.push(safe);
            bytes += size;
        }
        if !chunk.is_empty() {
            chunks.push(serde_json::to_string(&chunk).unwrap());
        }
        let mut c = self.conn()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let size: i64 = tx
            .query_row(
                "SELECT payload_bytes FROM queue_usage WHERE id=1",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if size + chunks.iter().map(|s| s.len() as i64).sum::<i64>() > 256 * 1024 * 1024 {
            return Err("Outbox is full (256 MiB); existing events are retained".into());
        }
        for payload in chunks {
            let count = serde_json::from_str::<Vec<serde_json::Value>>(&payload)
                .unwrap()
                .len();
            tx.execute("INSERT INTO batches(destination,stream,payload,events,created_at) VALUES(?1,?2,?3,?4,?5)", params![self.destination,stream,payload,count,now()]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn claim(&self) -> Result<Option<Claim>, String> {
        let mut c = self.conn()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let row: Option<(i64,String)> = tx.query_row("SELECT b.id,b.payload FROM batches b
            WHERE destination=?1 AND (?3 IS NULL OR stream=?3) AND blocked=0 AND next_attempt<=?2 AND lease_until<=?2
            AND NOT EXISTS(SELECT 1 FROM batches p WHERE p.destination=b.destination AND p.stream=b.stream AND p.id<b.id)
            ORDER BY b.id LIMIT 1", params![self.destination,now(),self.stream], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e| e.to_string())?;
        let Some((id, payload)) = row else {
            return Ok(None);
        };
        let events = serde_json::from_str(&payload)
            .map_err(|e| format!("Invalid outbox batch {id}: {e}"))?;
        let token = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "UPDATE batches SET token=?1,lease_until=?2,attempts=attempts+1 WHERE id=?3",
            params![token, now() + 30_000, id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Some(Claim { id, token, events }))
    }
    pub fn ack(&self, claim: &Claim) -> Result<(), String> {
        self.conn()?
            .execute(
                "DELETE FROM batches WHERE id=?1 AND token=?2",
                params![claim.id, claim.token],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn fail(&self, claim: &Claim, error: &str, blocked: bool) -> Result<(), String> {
        self.conn()?.execute("UPDATE batches SET last_error=?1,blocked=?2,next_attempt=?3+MIN(60000,1000*(1 << MIN(attempts,6))),lease_until=0,token=NULL WHERE id=?4 AND token=?5",
            params![crate::redact::redact_secrets(error),blocked,now(),claim.id,claim.token]).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn pending(&self) -> Result<Vec<PendingBatch>, String> {
        self.read_pending(&self.conn()?, -1)
    }
    fn read_pending(&self, c: &Connection, limit: i64) -> Result<Vec<PendingBatch>, String> {
        let mut s = c.prepare("SELECT id,stream,events,created_at,attempts,next_attempt,blocked,last_error FROM batches WHERE destination=?1 ORDER BY id LIMIT ?2").map_err(|e| e.to_string())?;
        let rows = s
            .query_map(params![self.destination, limit], |r| {
                Ok(PendingBatch {
                    id: r.get(0)?,
                    stream: r.get(1)?,
                    events: r.get(2)?,
                    created_at: r.get(3)?,
                    attempts: r.get(4)?,
                    next_attempt: r.get(5)?,
                    blocked: r.get(6)?,
                    last_error: r.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    /// Bound panel memory even when a long offline period leaves many batches.
    pub fn snapshot(&self) -> Result<QueueSnapshot, String> {
        let mut c = self.conn()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let mut value = tx.query_row("SELECT COUNT(*),COALESCE(SUM(events),0),COALESCE(SUM(blocked),0) FROM batches WHERE destination=?1",[&self.destination],|r| Ok(QueueSnapshot {
            pending_batches:r.get(0)?,pending_events:r.get(1)?,blocked_batches:r.get(2)?,batches:Vec::new(),
        })).map_err(|e| e.to_string())?;
        value.batches = self.read_pending(&tx, 100)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(value)
    }
    pub fn has_pending_stream(&self, stream: &str) -> Result<bool, String> {
        self.conn()?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM batches WHERE destination=?1 AND stream=?2)",
                params![self.destination, stream],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }
    pub fn retry(&self) -> Result<(), String> {
        self.conn()?
            .execute(
                "UPDATE batches SET blocked=0,next_attempt=0 WHERE destination=?1",
                [&self.destination],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// One bounded request. Exact ID acknowledgement is required, never just HTTP 200.
    pub fn deliver_http(&self) -> Result<bool, String> {
        let Some(claim) = self.claim()? else {
            return Ok(false);
        };
        let result = (|| -> Result<(), (String, bool)> {
            let client = reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_millis(1500))
                .build()
                .map_err(|_| ("Unable to create delivery client".into(), false))?;
            let response = client
                .post(format!("{}/api/v1/delivery", self.destination))
                .json(&claim.events)
                .send()
                .map_err(|_| ("Server unavailable; events retained".into(), false))?;
            let status = response.status();
            if !status.is_success() {
                return Err((
                    format!("Delivery HTTP {status}"),
                    matches!(status.as_u16(), 400 | 413 | 422),
                ));
            }
            let ack: serde_json::Value = response
                .json()
                .map_err(|_| ("Invalid delivery acknowledgement".into(), false))?;
            if ack["event_ids"]
                != serde_json::json!(claim.events.iter().map(|e| &e.event_id).collect::<Vec<_>>())
            {
                return Err(("Incomplete delivery acknowledgement".into(), false));
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.ack(&claim)?,
            Err((error, blocked)) => self.fail(&claim, &error, blocked)?,
        }
        Ok(true)
    }
    pub fn deliver_local(
        &self,
        engine: &crate::server::correlation::CorrelationEngine,
    ) -> Result<bool, String> {
        let Some(claim) = self.claim()? else {
            return Ok(false);
        };
        match engine.process_events_with(&claim.events, || Ok(())) {
            Ok(()) => self.ack(&claim)?,
            Err(e) => self.fail(
                &claim,
                &e.to_string(),
                matches!(e, rusqlite::Error::InvalidParameterName(_)),
            )?,
        }
        Ok(true)
    }
}
