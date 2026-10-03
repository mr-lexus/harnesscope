//! Offline-safe hook ingress. Queue files contain sanitized envelopes only.
use crate::{
    capture::{self, ObservationInput},
    config::Config,
    storage::Repository,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub fn queue_dir(db: &Path) -> PathBuf {
    let mut p = db.as_os_str().to_owned();
    p.push(".capture");
    PathBuf::from(p)
}

pub fn hook(config: &Config) -> Result<(), String> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(capture::MAX_OBJECT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read hook input")?;
    let record: Value = if bytes.len() > capture::MAX_OBJECT {
        json!({"hook_event_name":"oversized_hook"})
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"hook_event_name":"invalid_hook_json"}))
    };
    let id = uuid::Uuid::new_v4().to_string();
    let mut input = capture::from_record("codex_hook", "local_hook", &id, &record);
    input.observed_at = Some(chrono::Utc::now().to_rfc3339());
    let safe = if record["hook_event_name"] == "oversized_hook" {
        capture::SafeContent::excluded("object_limit")
    } else if record["hook_event_name"] == "invalid_hook_json" {
        capture::SafeContent::excluded("invalid_hook_json")
    } else {
        capture::sanitize(&record)
    };
    let envelope = json!({"input":input,"content":safe.value(),"disposition":safe.disposition,"reason":safe.reason});
    let dir = queue_dir(&config.db_path);
    std::fs::create_dir_all(&dir).map_err(|_| "Cannot create capture queue")?;
    if fs2::available_space(&dir).map_err(|_| "Cannot inspect queue capacity")?
        < 128 * 1024 * 1024 + bytes.len() as u64
    {
        return Err("Capture queue paused: low disk space".into());
    }
    let mut file =
        tempfile::NamedTempFile::new_in(&dir).map_err(|_| "Cannot open capture queue")?;
    serde_json::to_writer(&mut file, &envelope).map_err(|_| "Cannot write capture queue")?;
    file.flush().map_err(|_| "Cannot flush capture queue")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "Cannot sync capture queue")?;
    file.persist_noclobber(dir.join(format!("{id}.json")))
        .map_err(|_| "Cannot publish capture queue")?;
    #[cfg(unix)]
    std::fs::File::open(&dir)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync queue directory")?;
    Ok(())
}

pub fn drain(repo: &Repository, db: &Path) -> Result<usize, String> {
    let dir = queue_dir(db);
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)
        .map_err(|_| "Cannot read capture queue")?
        .take(100)
    {
        let entry = entry.map_err(|_| "Cannot read queue entry")?;
        if entry.path().extension().is_none_or(|e| e != "json") {
            continue;
        }
        if !entry
            .file_type()
            .map_err(|_| "Cannot inspect queue entry")?
            .is_file()
        {
            continue;
        }
        if entry
            .metadata()
            .map_err(|_| "Cannot inspect queue entry")?
            .len()
            > capture::MAX_OBJECT as u64 + 64 * 1024
        {
            return Err("Invalid capture envelope size".into());
        }
        let value: Value = serde_json::from_reader(
            std::fs::File::open(entry.path()).map_err(|_| "Cannot open queue entry")?,
        )
        .map_err(|_| "Invalid capture envelope")?;
        let input: ObservationInput = serde_json::from_value(value["input"].clone())
            .map_err(|_| "Invalid capture metadata")?;
        let safe = if value["content"].is_null() {
            capture::SafeContent::excluded(capture::exclusion_reason(value["reason"].as_str()))
        } else {
            capture::sanitize(&value["content"])
                .with_prior_redaction(value["disposition"] == "redacted")
        };
        repo.record_observation(&input, &safe)
            .map_err(|_| "Cannot persist capture entry; queued for retry")?;
        std::fs::remove_file(entry.path()).map_err(|_| "Cannot acknowledge capture entry")?;
        count += 1;
    }
    Ok(count)
}

/// Emit config for review. Installation never trusts hooks or replaces existing
/// OTel settings. Use an absolute executable and shell-safe quoting.
pub fn hook_configuration(executable: &Path, database: &Path) -> Result<Value, String> {
    let exe = executable.to_string_lossy();
    let database = std::path::absolute(database).map_err(|_| "Cannot resolve capture database")?;
    let database = database.to_string_lossy();
    if [&exe, &database]
        .iter()
        .any(|p| p.contains(['\n', '\r', '\0']) || (cfg!(windows) && p.contains(['%', '!', '"'])))
    {
        return Err("Executable/database path contains unsupported shell characters".into());
    }
    let command = if cfg!(windows) {
        format!("\"{exe}\" capture hook --database \"{database}\"")
    } else {
        format!(
            "'{}' capture hook --database '{}'",
            exe.replace('\'', "'\\''"),
            database.replace('\'', "'\\''")
        )
    };
    let mut hooks = serde_json::Map::new();
    for event in [
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "PreCompact",
        "PostCompact",
        "SubagentStart",
        "SubagentStop",
        "Stop",
        "Interrupt",
    ] {
        hooks.insert(
            event.into(),
            json!([{"hooks":[{"type":"command","command":command,"async":true,"timeout":3}]}]),
        );
    }
    Ok(
        json!({"description":"Harnesscope sanitized local capture; review and trust in Codex","hooks":hooks}),
    )
}

pub fn import(repo: &Repository, path: &Path) -> Result<Value, String> {
    use std::io::BufRead;
    let mut reader =
        std::io::BufReader::new(std::fs::File::open(path).map_err(|_| "Cannot open package")?);
    let mut count = 0;
    let mut first = true;
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take(capture::MAX_OBJECT as u64 + 128 * 1024)
            .read_until(b'\n', &mut line)
            .map_err(|_| "Cannot read package")?;
        if n == 0 {
            break;
        }
        if line.len() >= capture::MAX_OBJECT + 128 * 1024 {
            return Err("Package record too large".into());
        }
        let record: Value = serde_json::from_slice(&line).map_err(|_| "Invalid package JSON")?;
        if first {
            first = false;
            if record["type"] != "manifest" || record["schema_version"] != 1 {
                return Err("Unsupported package version".into());
            }
            continue;
        }
        if let Some(expected) = record["metadata"]["object_hash"].as_str() {
            if record["content"].is_null()
                || !crate::storage::evidence::valid_hash(expected)
                || crate::redact::sha256_digest(&record["content"].to_string()) != expected
            {
                return Err(
                    "Package object hash mismatch; accepted prefix retained for retry".into(),
                );
            }
        }
        if record["type"] == "workflow" || record["type"] == "task" {
            repo.import_retro_context(&record)
                .map_err(|_| "Cannot import retrospective context")?;
            continue;
        }
        if record["type"] != "observation" {
            return Err("Unsupported package record".into());
        }
        let m = &record["metadata"];
        let input: ObservationInput =
            serde_json::from_value(m.clone()).map_err(|_| "Invalid package observation")?;
        let safe = if record["content"].is_null() {
            capture::SafeContent::excluded(
                if record["metadata"]["reason"] == "metadata_only_policy" {
                    "metadata_only_policy"
                } else {
                    "excluded_in_package"
                },
            )
        } else {
            capture::sanitize(&record["content"])
                .with_prior_redaction(record["metadata"]["disposition"] == "redacted")
        };
        repo.record_observation(&input, &safe)
            .map_err(|_| "Cannot import observation")?;
        count += 1;
    }
    if first {
        return Err("Empty package".into());
    }
    Ok(json!({"imported":count,"schema_version":1}))
}
