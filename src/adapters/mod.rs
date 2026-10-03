pub mod codex;
pub mod codex_sqlite;

use crate::{
    server::correlation::CorrelationEngine,
    storage::{
        sources::{SourceFile, TelemetrySource},
        Repository,
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const MAX_FILES: usize = 10_000;
const MAX_LINE: u64 = 32 * 1024 * 1024;
const RECORDS_PER_BATCH: usize = 200;
const BATCHES_PER_SCAN: usize = 10;

/// Walk only explicit roots. Do not follow symlinks/junctions into other trees.
fn discover(root: &Path) -> Result<Vec<PathBuf>, String> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    if !root.is_dir() {
        return Err("Source directory is unavailable".into());
    }
    let mut dirs = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).map_err(|_| "Cannot read source directory")? {
            visited += 1;
            if visited > 100_000 {
                return Err(
                    "Source tree exceeds 100,000 entries; choose a smaller directory".into(),
                );
            }
            let entry = entry.map_err(|_| "Cannot read source directory entry")?;
            let kind = entry
                .file_type()
                .map_err(|_| "Cannot inspect source entry")?;
            let path = entry.path();
            if kind.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if fs::symlink_metadata(&path)
                    .map_err(|_| "Cannot inspect source entry")?
                    .file_attributes()
                    & 0x400
                    != 0
                {
                    continue;
                }
            }
            if kind.is_dir() {
                dirs.push(path);
            } else if kind.is_file()
                && (path.extension().is_some_and(|e| e == "jsonl")
                    || entry.file_name().to_string_lossy().ends_with(".jsonl.gz"))
                && entry.file_name().to_string_lossy().starts_with("rollout-")
            {
                files.push(path);
                if files.len() > MAX_FILES {
                    return Err("Source exceeds 10,000 rollouts; choose a smaller directory".into());
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

type Captured = (
    crate::capture::ObservationInput,
    crate::capture::SafeContent,
);
type ParsedBatch = (
    SourceFile,
    Vec<crate::domain::events::IngestEvent>,
    Vec<Captured>,
);
fn parse_batch(source: &TelemetrySource, previous: &SourceFile) -> Result<ParsedBatch, String> {
    let file = File::open(&previous.path).map_err(|_| "Cannot open source file")?;
    let metadata = file.metadata().map_err(|_| "Cannot inspect source file")?;
    if !metadata.is_file() {
        return Err("Source is no longer a regular file".into());
    }
    let size = metadata.len();
    let compressed = previous.path.ends_with(".jsonl.gz");
    if !previous.path.ends_with(".jsonl") && !compressed {
        return Err("Unsupported history format; supported: JSONL and gzip JSONL".into());
    }
    if !compressed && size < previous.byte_offset {
        return Err("Source was truncated; checkpoint retained. Restore the original file or register a separate copy.".into());
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_default();
    if size == previous.file_size
        && modified == previous.modified_stamp
        && !modified.is_empty()
        && matches!(previous.status.as_str(), "READY" | "WAITING")
    {
        return Ok((previous.clone(), Vec::new(), Vec::new()));
    }
    let input: Box<dyn Read> = if compressed {
        Box::new(flate2::read::MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::new(input);
    let mut hash = Sha256::new();
    let mut remaining = previous.byte_offset;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let limit = remaining.min(buffer.len() as u64) as usize;
        reader
            .read_exact(&mut buffer[..limit])
            .map_err(|_| "Cannot verify consumed prefix")?;
        hash.update(&buffer[..limit]);
        remaining -= limit as u64;
    }
    if previous.byte_offset > 0 && hex::encode(hash.clone().finalize()) != previous.prefix_hash {
        return Err("Consumed source content changed; checkpoint retained. Restore the original file or register a separate copy.".into());
    }
    let mut state: codex::CodexState = if previous.byte_offset == 0 {
        Default::default()
    } else {
        serde_json::from_str(&previous.state_json)
            .map_err(|_| "Checkpoint is incompatible with this adapter")?
    };
    let mut next = previous.clone();
    next.file_size = size;
    next.modified_stamp = modified;
    next.status = "BACKLOG".into();
    next.last_error = None;
    let mut events = Vec::new();
    let mut observations = Vec::new();
    for _ in 0..RECORDS_PER_BATCH {
        if next.byte_offset.saturating_sub(previous.byte_offset) >= 8 * 1024 * 1024 {
            break;
        }
        let mut bytes = Vec::new();
        let count = reader
            .by_ref()
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| "Cannot read rollout line")?;
        if count == 0 {
            next.status = "READY".into();
            break;
        }
        if count as u64 > MAX_LINE {
            return Err(format!("Line {} exceeds 32 MiB", next.line_number + 1));
        }
        if bytes.last() != Some(&b'\n') {
            next.status = "WAITING".into();
            break;
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| format!("Line {} is not UTF-8", next.line_number + 1))?;
        let text = if next.byte_offset == 0 {
            text.trim_start_matches('\u{feff}')
        } else {
            text
        };
        if !text.trim().is_empty() {
            let record: serde_json::Value = serde_json::from_str(text).map_err(|_| {
                format!(
                    "Invalid JSON at line {}; no data from this batch was committed",
                    next.line_number + 1
                )
            })?;
            let captured = state.observation(
                &previous.path,
                &next.byte_offset.to_string(),
                &record,
                source.include_content,
            );
            let clean = crate::redact::redact_value(&record);
            let converted = state
                .convert(
                    &clean,
                    source.include_content && captured.1.value().is_some(),
                )
                .map_err(|e| format!("Line {}: {e}", next.line_number + 1))?;
            next.ignored_count += u64::from(converted.ignored);
            events.extend(converted.events);
            observations.push(captured);
        }
        hash.update(&bytes);
        next.byte_offset += count as u64;
        next.line_number += 1;
    }
    if !compressed && next.byte_offset == size {
        next.status = "READY".into();
    }
    next.prefix_hash = hex::encode(hash.finalize());
    next.state_json =
        serde_json::to_string(&state).map_err(|_| "Cannot save adapter checkpoint")?;
    next.events_count += events.len() as u64;
    next.updated_at = chrono::Utc::now().to_rfc3339();
    Ok((next, events, observations))
}

/// One bounded pass. Files are the durable queue: offsets advance only in the
/// same transaction as projections. A crash or concurrent scanner cannot lose data.
pub fn scan_sources(
    repo: &Repository,
    engine: &CorrelationEngine,
    stop: &AtomicBool,
) -> Result<(), String> {
    for source in repo
        .list_sources()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|s| s.enabled)
    {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let result = scan_source(repo, engine, &source, stop);
        repo.finish_source_scan(&source.id, result.as_ref().err().map(String::as_str))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn scan_source(
    repo: &Repository,
    engine: &CorrelationEngine,
    source: &TelemetrySource,
    stop: &AtomicBool,
) -> Result<(), String> {
    if source.adapter == "codex-sqlite/v1" {
        return codex_sqlite::scan(repo, source);
    }
    if source.adapter != "codex-rollout/v1" {
        return Err("Unsupported adapter checkpoint version".into());
    }
    let root = Path::new(&source.path);
    if root.canonicalize().map_err(|_| "Source is unavailable")? != root {
        return Err("Registered source now points to a different location".into());
    }
    let mut paths = discover(root)?;
    let files = repo
        .list_source_files(&source.id)
        .map_err(|e| e.to_string())?;
    let mut checkpoints: HashMap<_, _> = files.into_iter().map(|f| (f.path.clone(), f)).collect();
    // Oldest inspected file first, preventing a busy large file from starving others.
    paths.sort_by_key(|p| {
        checkpoints
            .get(p.to_string_lossy().as_ref())
            .map(|f| f.updated_at.clone())
            .unwrap_or_default()
    });
    let mut failures = 0;
    let mut batches = 0;
    for path in paths {
        if stop.load(Ordering::Relaxed) || batches >= BATCHES_PER_SCAN {
            break;
        }
        if !repo.source_enabled(&source.id).map_err(|e| e.to_string())? {
            break;
        }
        // Recheck canonical containment when a directory tree changes during discovery.
        let canonical = path
            .canonicalize()
            .map_err(|_| "Source file disappeared during scan")?;
        if root.is_dir() && !canonical.starts_with(root) {
            return Err("Source path escaped the registered directory".into());
        }
        let path = path
            .to_str()
            .ok_or("Source path must be Unicode")?
            .to_string();
        let previous = checkpoints.remove(&path).unwrap_or(SourceFile {
            source_id: source.id.clone(),
            path,
            state_json: "{}".into(),
            ..Default::default()
        });
        match parse_batch(source, &previous) {
            Ok((next, events, observations)) => {
                if next.updated_at == previous.updated_at {
                    continue;
                }
                batches += 1;
                if let Err(error) = engine.process_events_with(&events, || {
                    if !repo.source_policy_matches(&source.id, source.include_content)? {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    for (input, safe) in &observations {
                        repo.record_observation(input, safe)?;
                    }
                    repo.save_source_file(&next, previous.byte_offset)
                }) {
                    failures += 1;
                    tracing::warn!(source_id=%source.id,"Source batch rolled back: {error}");
                    let failed = SourceFile {
                        status: "ERROR".into(),
                        last_error: Some(
                            "Event projection failed; batch and checkpoint rolled back".into(),
                        ),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        ..previous.clone()
                    };
                    let _ =
                        repo.transaction(|| repo.save_source_file(&failed, previous.byte_offset));
                }
            }
            Err(error) => {
                failures += 1;
                batches += 1;
                let next = SourceFile {
                    status: "ERROR".into(),
                    last_error: Some(error),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    ..previous.clone()
                };
                let _ = repo.transaction(|| repo.save_source_file(&next, previous.byte_offset));
            }
        }
    }
    // Keep an unresolved error visible even if the file was not visited this pass.
    failures = failures.max(
        repo.source_error_count(&source.id)
            .map_err(|e| e.to_string())?,
    );
    if failures > 0 {
        Err(format!(
            "{failures} file(s) need attention; see file errors. Other files continue importing."
        ))
    } else {
        Ok(())
    }
}
