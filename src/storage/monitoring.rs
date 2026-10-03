use super::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct RuntimeObservation {
    pub runtime_id: String,
    pub runner: String,
    pub pid: Option<u32>,
    pub status: String,
    pub observed_at: Option<String>,
    pub received_at: Option<String>,
    pub freshness: String,
    pub execution_id: Option<String>,
    pub cwd: String,
}
impl Repository {
    pub fn observe_runtime(&self, id: &str, timestamp: &str) -> rusqlite::Result<()> {
        self.db.with_conn(|c| {
            c.execute("INSERT INTO runtime_observations(runtime_id,observed_at,received_at) VALUES(?1,?2,?3)
                ON CONFLICT(runtime_id) DO UPDATE SET observed_at=excluded.observed_at,received_at=excluded.received_at
                WHERE excluded.observed_at>runtime_observations.observed_at",
                rusqlite::params![id,timestamp,chrono::Utc::now().to_rfc3339()])?;
            Ok(())
        })
    }
    pub fn observations(&self) -> rusqlite::Result<Vec<RuntimeObservation>> {
        self.db.with_conn(|c| {
            let mut s = c.prepare("SELECT r.id,r.runner_name,r.pid,r.status,o.observed_at,o.received_at,
                (SELECT id FROM executions e WHERE e.runtime_id=r.id ORDER BY started_at DESC LIMIT 1),r.cwd,r.ended_at
                FROM runtime_instances r LEFT JOIN runtime_observations o ON o.runtime_id=r.id
                WHERE r.surface!='transcript' ORDER BY (r.status='RUNNING') DESC,r.started_at DESC LIMIT 200")?;
            let rows = s.query_map([], |r| {
                let status: String = r.get(3)?;
                let observed: Option<String> = r.get(4)?;
                let age = observed.as_ref().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                    .map(|t| (chrono::Utc::now()-t.with_timezone(&chrono::Utc)).num_seconds());
                let freshness = if r.get::<_,Option<String>>(8)?.is_some() { "ENDED" } else if status != "RUNNING" { "UNMONITORED" } else { match age {
                    Some(n) if n < -5 => "CLOCK_SKEW", Some(n) if n <= 45 => "FRESH", Some(_) => "STALE", None => "UNMONITORED"
                }};
                Ok(RuntimeObservation { runtime_id:r.get(0)?, runner:r.get(1)?, pid:r.get(2)?, status,
                    observed_at:observed, received_at:r.get(5)?, freshness:freshness.into(), execution_id:r.get(6)?,cwd:r.get(7)? })
            })?;
            rows.collect()
        })
    }
}
