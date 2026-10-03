//! Read selected known history columns only. Never migrate or copy a Codex DB.
use crate::{
    capture,
    storage::{sources::TelemetrySource, Repository},
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};

pub fn scan(repo: &Repository, source: &TelemetrySource) -> Result<(), String> {
    let path = std::path::Path::new(&source.path);
    if path
        .canonicalize()
        .map_err(|_| "Codex history is unavailable")?
        != path
    {
        return Err("Registered history path changed location".into());
    }
    let conn = Connection::open_with_flags(&source.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "Cannot open Codex history read-only")?;
    conn.busy_timeout(std::time::Duration::from_millis(100))
        .map_err(|_| "Cannot configure history reader")?;
    conn.pragma_update(None, "query_only", true)
        .map_err(|_| "Cannot configure history reader")?;
    conn.pragma_update(None, "trusted_schema", false)
        .map_err(|_| "Cannot configure history reader")?;
    let table: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='thread_items')",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Cannot inspect history schema")?;
    if !table {
        return Err("Unsupported Codex SQLite history schema: thread_items must be a table".into());
    }
    let mut stmt=conn.prepare("SELECT rowid,thread_id,turn_id,item_id,updated_at_ordinal,item_json FROM thread_items WHERE rowid>?1 ORDER BY rowid LIMIT 200").map_err(|_|"Unsupported Codex SQLite history schema: expected thread_items v1 columns")?;
    let start = repo
        .evidence_cursor(&source.id)
        .map_err(|_| "Cannot read history cursor")?;
    let mut rows = stmt
        .query([start])
        .map_err(|_| "Cannot query Codex history")?;
    let mut last = start;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(|_| "Cannot read Codex history row")? {
        last = row.get(0).map_err(|_| "Unsupported history row")?;
        let thread: String = row.get(1).map_err(|_| "Unsupported history thread")?;
        let turn: Option<String> = row.get(2).map_err(|_| "Unsupported history turn")?;
        let item: String = row.get(3).map_err(|_| "Unsupported history item")?;
        let ordinal: i64 = row.get(4).map_err(|_| "Unsupported history ordinal")?;
        let text: String = row.get(5).map_err(|_| "Unsupported history JSON")?;
        let record = if text.len() > capture::MAX_OBJECT {
            json!({"type":"item_completed","payload":null})
        } else {
            let value: Value =
                serde_json::from_str(&text).map_err(|_| "Invalid Codex history item JSON")?;
            json!({"type":"item_completed","payload":{"item":value}})
        };
        let mut input = capture::from_record(
            "codex_sqlite",
            &source.path,
            &format!(
                "{}:{}:{}",
                capture::metadata(&thread),
                capture::metadata(&item),
                ordinal
            ),
            &record,
        );
        input.session_id = Some(capture::metadata(&thread));
        input.turn_id = turn.map(|s| capture::metadata(&s));
        input.native_id = Some(capture::metadata(&item));
        // A detached result cannot be checked against its original input. JSONL
        // or PostToolUse with paired arguments remains the preferred source.
        let p = &record["payload"]["item"];
        let safe = if !source.include_content {
            capture::SafeContent::excluded("metadata_only_policy")
        } else if text.len() > capture::MAX_OBJECT {
            capture::SafeContent::excluded("object_limit")
        } else if p.get("output").is_some() {
            capture::SafeContent::excluded("unpaired_sqlite_tool_output")
        } else {
            capture::sanitize(&record)
        };
        repo.record_observation(&input, &safe)
            .map_err(|_| "Cannot archive Codex history")?;
        count += 1;
    }
    repo.save_evidence_cursor(&source.id, if count < 200 { 0 } else { last })
        .map_err(|_| "Cannot advance history cursor")?;
    Ok(())
}

impl Repository {
    pub fn evidence_cursor(&self, source: &str) -> rusqlite::Result<i64> {
        self.db.with_conn(|c| {
            c.query_row(
                "SELECT position FROM evidence_cursors WHERE source=?1",
                [source],
                |r| r.get(0),
            )
            .optional()
            .map(|v| v.unwrap_or(0))
        })
    }
    pub fn save_evidence_cursor(&self, source: &str, position: i64) -> rusqlite::Result<()> {
        self.db.with_conn(|c|{c.execute("INSERT INTO evidence_cursors VALUES(?1,?2) ON CONFLICT(source) DO UPDATE SET position=excluded.position",rusqlite::params![source,position])?;Ok(())})
    }
}
