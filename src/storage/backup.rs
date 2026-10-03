//! Online SQLite snapshots, published only after validation. Never overwrite a database.
use rusqlite::{
    backup::{Backup, StepResult},
    Connection, OpenFlags,
};
use serde::Serialize;
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
pub struct BackupReport {
    pub path: String,
    pub scope: &'static str,
    pub schema_version: i64,
    pub events: u64,
    pub executions: u64,
    pub sessions: u64,
    pub reviews: u64,
    pub registered_sources: u64,
    pub enabled_sources: u64,
    pub bytes: u64,
    pub integrity: &'static str,
    pub excluded: [&'static str; 3],
}

fn read_only(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_millis(250))
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "trusted_schema", false)
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

/// No migrations, writes or implicit creation of a missing input.
pub fn verify(path: &Path) -> Result<BackupReport, String> {
    let mut conn = read_only(path)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let check: String = tx
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err(format!("SQLite integrity check failed: {check}"));
    }
    let broken: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if broken {
        return Err("Foreign-key check failed".into());
    }
    let version: i64 = tx
        .query_row("SELECT MAX(version) FROM _schema_migrations", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("Not a Harnesscope database: {e}"))?;
    if !(1..=6).contains(&version) {
        return Err(format!("Unsupported schema version {version}"));
    }
    for (since, table) in [
        (1, "runtime_instances"),
        (1, "sessions"),
        (1, "runtime_session_bindings"),
        (1, "executions"),
        (1, "agent_instances"),
        (1, "components"),
        (1, "execution_components"),
        (1, "git_snapshots"),
        (1, "config_snapshots"),
        (1, "events"),
        (2, "session_conflicts"),
        (3, "session_aliases"),
        (3, "execution_reviews"),
        (4, "telemetry_sources"),
        (4, "source_files"),
        (4, "execution_usage"),
        (5, "runtime_observations"),
        (6, "observations"),
        (6, "evidence_objects"),
        (6, "evidence_items"),
        (6, "retro_tasks"),
        (6, "task_links"),
        (6, "task_marks"),
        (6, "workflow_roots"),
        (6, "workflow_versions"),
        (6, "external_task_versions"),
        (6, "evidence_cursors"),
    ] {
        if version >= since {
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                    [table],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !exists {
                return Err(format!("Missing Harnesscope table: {table}"));
            }
        }
    }
    let count = |table: &str| -> Result<u64, String> {
        tx.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .map_err(|e| e.to_string())
    };
    if version >= 6 {
        let mut stmt = tx
            .prepare("SELECT hash,bytes,inline_json FROM evidence_objects")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let hash: String = row.get(0).map_err(|e| e.to_string())?;
            let bytes: u64 = row.get(1).map_err(|e| e.to_string())?;
            if !super::evidence::valid_hash(&hash) || bytes > crate::capture::MAX_OBJECT as u64 {
                return Err("Invalid evidence object".into());
            }
            let inline: Option<String> = row.get(2).map_err(|e| e.to_string())?;
            let text = match inline {
                Some(s) => s,
                None => std::fs::read_to_string(super::evidence::object_dir(path).join(&hash))
                    .map_err(|_| "Missing evidence object")?,
            };
            if text.len() as u64 != bytes || crate::redact::sha256_digest(&text) != hash {
                return Err("Corrupt evidence object".into());
            }
        }
    }
    let report = BackupReport {
        path: path.to_string_lossy().into(),
        scope: "main_database",
        schema_version: version,
        events: count("events")?,
        executions: count("executions")?,
        sessions: count("sessions")?,
        reviews: if version >= 3 {
            count("execution_reviews")?
        } else {
            0
        },
        registered_sources: if version >= 4 {
            count("telemetry_sources")?
        } else {
            0
        },
        enabled_sources: if version >= 4 {
            tx.query_row(
                "SELECT COUNT(*) FROM telemetry_sources WHERE enabled=1",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?
        } else {
            0
        },
        bytes: std::fs::metadata(path).map_err(|e| e.to_string())?.len(),
        integrity: "ok",
        excluded: [
            "pending outbox delivery",
            "native transcript files",
            "application configuration",
        ],
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(report)
}

pub fn create(source: &Path, output: &Path) -> Result<BackupReport, String> {
    copy(source, output, false, Duration::from_secs(60))
}
pub fn restore(source: &Path, output: &Path) -> Result<BackupReport, String> {
    copy(source, output, true, Duration::from_secs(60))
}

fn copy(
    source: &Path,
    output: &Path,
    restoring: bool,
    timeout: Duration,
) -> Result<BackupReport, String> {
    if output.try_exists().map_err(|e| e.to_string())? {
        return Err("Destination already exists; choose a new path".into());
    }
    // A dangling symlink or stale SQLite sidecar must not become a destination either.
    if std::fs::symlink_metadata(output).is_ok() {
        return Err("Destination already exists".into());
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = output.as_os_str().to_owned();
        sidecar.push(suffix);
        if std::fs::symlink_metadata(&sidecar).is_ok() {
            return Err("Destination has SQLite sidecars; choose a new path".into());
        }
    }
    let source_conn = read_only(source)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file = tempfile::Builder::new()
        .prefix(".harnesscope-backup-")
        .suffix(".sqlite3")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    {
        let mut destination = Connection::open(file.path()).map_err(|e| e.to_string())?;
        destination
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        {
            let backup = Backup::new(&source_conn, &mut destination).map_err(|e| e.to_string())?;
            let deadline = Instant::now() + timeout;
            loop {
                if Instant::now() >= deadline {
                    return Err("Snapshot timed out; destination was not published. Retry during a quieter period.".into());
                }
                match backup.step(256).map_err(|e| e.to_string())? {
                    StepResult::Done => break,
                    StepResult::More => std::thread::sleep(Duration::from_millis(1)),
                    StepResult::Busy | StepResult::Locked => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    _ => return Err("Unexpected SQLite backup state".into()),
                }
            }
        }
        // A standalone file must not rely on WAL or SHM siblings.
        destination
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(|e| e.to_string())?;
        // Portable snapshots inline immutable objects one at a time. The live
        // archive remains separate, but backup/restore needs only this one file.
        let has_objects: bool = destination
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='evidence_objects')",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if has_objects {
            let hashes: Vec<String> = destination
                .prepare("SELECT hash FROM evidence_objects WHERE inline_json IS NULL")
                .map_err(|e| e.to_string())?
                .query_map([], |r| r.get(0))
                .map_err(|e| e.to_string())?
                .collect::<rusqlite::Result<_>>()
                .map_err(|e| e.to_string())?;
            for hash in hashes {
                if !super::evidence::valid_hash(&hash) {
                    return Err("Invalid evidence hash".into());
                }
                let object = super::evidence::object_dir(source).join(&hash);
                if std::fs::metadata(&object)
                    .map_err(|_| "Missing evidence object")?
                    .len()
                    > crate::capture::MAX_OBJECT as u64
                {
                    return Err("Oversized evidence object".into());
                }
                let text =
                    std::fs::read_to_string(object).map_err(|_| "Cannot read evidence object")?;
                if crate::redact::sha256_digest(&text) != hash {
                    return Err("Corrupt evidence object".into());
                }
                destination
                    .execute(
                        "UPDATE evidence_objects SET inline_json=?2 WHERE hash=?1",
                        rusqlite::params![hash, text],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        verify(file.path())?;
        if restoring {
            super::migrations::run_migrations(&mut destination).map_err(|e| e.to_string())?;
            destination
                .execute("UPDATE telemetry_sources SET enabled=0", [])
                .map_err(|e| e.to_string())?;
        }
        destination.close().map_err(|(_, e)| e.to_string())?;
    }
    let mut report = verify(file.path())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    // Atomic no-clobber publication also covers another process creating output meanwhile.
    file.persist_noclobber(output).map_err(|e| e.to_string())?;
    report.path = output.to_string_lossy().into();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_never_publishes_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.db");
        let _db = super::super::Database::open(&source).unwrap();
        let output = dir.path().join("backup.db");
        assert!(copy(&source, &output, false, Duration::ZERO)
            .unwrap_err()
            .contains("timed out"));
        assert!(!output.exists());
        assert!(!std::fs::read_dir(dir.path()).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".harnesscope-backup-")));
    }
}
