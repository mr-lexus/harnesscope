//! Sanitized, immutable observations. Projections can be rebuilt independently.
use super::Repository;
use crate::{
    capture::{self, ObservationInput, SafeContent},
    redact::sha256_digest,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub fn object_dir(database: &Path) -> PathBuf {
    let mut path = database.as_os_str().to_owned();
    path.push(".objects");
    PathBuf::from(path)
}
fn failure() -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(
        "Evidence storage unavailable or corrupt; checkpoint retained".into(),
    )
}
pub fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct EvidenceFilter {
    pub after: Option<i64>,
    pub through: Option<i64>,
    pub limit: Option<u32>,
    pub session: Option<String>,
    pub task: Option<String>,
    pub project: Option<String>,
    pub kind: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub q: Option<String>,
    pub category: Option<String>,
}

impl Repository {
    pub fn database_path(&self) -> Result<Option<PathBuf>, String> {
        self.db
            .with_conn(|c| Ok(c.path().filter(|p| !p.is_empty()).map(PathBuf::from)))
            .map_err(|_| "Cannot locate database".into())
    }
    pub fn evidence_dir(&self) -> rusqlite::Result<Option<PathBuf>> {
        self.db.with_conn(|c| {
            Ok(c.path()
                .filter(|p| !p.is_empty())
                .map(|p| object_dir(Path::new(p))))
        })
    }

    /// The caller must supply sanitized content. Publish bytes durably before a
    /// transaction can refer to them; a rollback leaves only a harmless orphan.
    pub fn store_object(&self, safe: &SafeContent) -> rusqlite::Result<Option<String>> {
        let Some(value) = safe.value() else {
            return Ok(None);
        };
        let text = serde_json::to_string(value).map_err(|_| failure())?;
        if text.len() > capture::MAX_OBJECT {
            return Err(failure());
        }
        let hash = sha256_digest(&text);
        let dir = self.evidence_dir()?;
        if let Some(dir) = &dir {
            let parent = dir.parent().unwrap_or(Path::new("."));
            if fs2::available_space(parent).map_err(|_| failure())?
                < text.len() as u64 + 128 * 1024 * 1024
            {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Collection paused: less than 128 MiB free space".into(),
                ));
            }
        }
        let external = text.len() > 64 * 1024 && dir.is_some();
        if external {
            let dir = dir.unwrap();
            std::fs::create_dir_all(&dir).map_err(|_| failure())?;
            let path = dir.join(&hash);
            if path.exists() {
                if std::fs::symlink_metadata(&path)
                    .map_err(|_| failure())?
                    .file_type()
                    .is_symlink()
                    || std::fs::read_to_string(&path).map_err(|_| failure())? != text
                {
                    return Err(failure());
                }
            } else {
                let mut file = tempfile::NamedTempFile::new_in(&dir).map_err(|_| failure())?;
                file.write_all(text.as_bytes()).map_err(|_| failure())?;
                file.as_file().sync_all().map_err(|_| failure())?;
                if file.persist_noclobber(&path).is_err()
                    && std::fs::read_to_string(&path).map_err(|_| failure())? != text
                {
                    return Err(failure());
                }
                #[cfg(unix)]
                std::fs::File::open(&dir)
                    .and_then(|f| f.sync_all())
                    .map_err(|_| failure())?;
            }
        }
        self.db.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO evidence_objects(hash,bytes,inline_json) VALUES(?1,?2,?3)",
                params![hash, text.len(), if external { None } else { Some(&text) }],
            )?;
            Ok(())
        })?;
        Ok(Some(hash))
    }

    pub fn evidence_object(&self, hash: &str) -> rusqlite::Result<Option<Value>> {
        if !valid_hash(hash) {
            return Ok(None);
        }
        let row: Option<(usize, Option<String>)> = self.db.with_conn(|c| {
            c.query_row(
                "SELECT bytes,inline_json FROM evidence_objects WHERE hash=?1",
                [hash],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        })?;
        let Some((bytes, inline)) = row else {
            return Ok(None);
        };
        let text = if let Some(text) = inline {
            text
        } else {
            let path = self.evidence_dir()?.ok_or_else(failure)?.join(hash);
            let meta = std::fs::symlink_metadata(&path).map_err(|_| failure())?;
            if !meta.is_file() || meta.len() as usize != bytes || bytes > capture::MAX_OBJECT {
                return Err(failure());
            }
            std::fs::read_to_string(path).map_err(|_| failure())?
        };
        if text.len() != bytes || sha256_digest(&text) != hash {
            return Err(failure());
        }
        serde_json::from_str(&text).map(Some).map_err(|_| failure())
    }

    pub fn evidence_object_page(
        &self,
        hash: &str,
        offset: usize,
        limit: usize,
    ) -> rusqlite::Result<Value> {
        let Some(object) = self.evidence_object(hash)? else {
            return Ok(json!({"error":"Object not found"}));
        };
        let text = serde_json::to_string_pretty(&object).map_err(|_| failure())?;
        let mut start = offset.min(text.len());
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = start.saturating_add(limit.clamp(1, 32000)).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == start && start < text.len() {
            end = start + text[start..].chars().next().unwrap().len_utf8();
        }
        Ok(
            json!({"hash":hash,"text":&text[start..end],"offset":start,"next":if end<text.len(){Some(end)}else{None},"total_bytes":text.len()}),
        )
    }

    pub fn evidence_diff(&self, before: &str, after: &str) -> rusqlite::Result<Value> {
        let a = self.evidence_object(before)?.ok_or_else(failure)?;
        let b = self.evidence_object(after)?.ok_or_else(failure)?;
        let text = |v: Value| -> String {
            if let Some(s) = v.as_str() {
                s.to_owned()
            } else {
                serde_json::to_string_pretty(&v).unwrap_or_default()
            }
        };
        let a = text(a);
        let b = text(b);
        if a.len() > 128 * 1024 || b.len() > 128 * 1024 {
            return Ok(
                json!({"available":false,"reason":"diff_limit","before":before,"after":after}),
            );
        }
        let a: Vec<_> = a.lines().collect();
        let b: Vec<_> = b.lines().collect();
        let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        let suffix = a[prefix..]
            .iter()
            .rev()
            .zip(b[prefix..].iter().rev())
            .take_while(|(x, y)| x == y)
            .count();
        let removed = &a[prefix..a.len() - suffix];
        let added = &b[prefix..b.len() - suffix];
        Ok(
            json!({"available":true,"same":before==after,"unchanged_prefix_lines":prefix,"unchanged_suffix_lines":suffix,"removed":removed,"added":added,"algorithm":"common_prefix_suffix","before":before,"after":after}),
        )
    }

    pub fn record_observation(
        &self,
        input: &ObservationInput,
        safe: &SafeContent,
    ) -> rusqlite::Result<String> {
        // Re-sanitize metadata at the public persistence boundary, including imports.
        let mut input: ObservationInput = serde_json::from_value(
            capture::sanitize(&serde_json::to_value(input).map_err(|_| failure())?)
                .value()
                .cloned()
                .ok_or_else(failure)?,
        )
        .map_err(|_| failure())?;
        input.project = input.project.map(|p| capture::project_path(&p));
        let policy = if safe.reason.as_deref() == Some("metadata_only_policy") {
            "metadata"
        } else {
            "content"
        };
        let id = sha256_digest(&format!(
            "{}\0{}\0{}\0{}",
            input.channel, input.source, input.position, policy
        ));
        self.transaction(|| {
            let exists:bool=self.db.with_conn(|c|c.query_row("SELECT EXISTS(SELECT 1 FROM observations WHERE id=?1)",[&id],|r|r.get(0)))?;
            if exists{return Ok(id.clone());}
            let hash = self.store_object(safe)?;
            let inserted = self.db.with_conn(|c| c.execute("INSERT OR IGNORE INTO observations(id,channel,source,source_version,position,observed_at,received_at,session_id,turn_id,native_id,kind,project,object_hash,disposition,reason) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![id,input.channel,input.source,input.source_version,input.position,input.observed_at,chrono::Utc::now().to_rfc3339(),input.session_id,input.turn_id,input.native_id,input.kind,input.project,hash,safe.disposition,safe.reason]))?;
            if inserted > 0 {
                self.project_observation(&id, safe.value())?;
                if let Some(value)=safe.value(){self.project_external_task(&id,value)?;}
            }
            Ok(id.clone())
        })
    }

    fn project_observation(&self, id: &str, value: Option<&Value>) -> rusqlite::Result<()> {
        let empty = json!({});
        let record = value.unwrap_or(&empty);
        let payload = record.get("payload").unwrap_or(record);
        let p = payload.get("item").unwrap_or(payload);
        let kind = p["type"]
            .as_str()
            .or(record["type"].as_str())
            .unwrap_or("unknown");
        let category = if kind.contains("call") || kind.contains("tool") {
            "tool"
        } else if kind.contains("compact") {
            "compaction"
        } else if kind.contains("usage") || kind == "token_count" {
            "usage"
        } else if kind == "message" || kind.ends_with("message") {
            "message"
        } else if kind.contains("context")
            || kind == "world_state"
            || p.get("model_context_window").is_some()
            || p.get("context_window").is_some()
        {
            "context"
        } else {
            "lifecycle"
        };
        self.db.with_conn(|c| {
            c.execute("INSERT OR REPLACE INTO evidence_items(observation_id,category,role,tool,call_id,parent_session_id) VALUES(?1,?2,?3,?4,?5,?6)",params![id,category,p["role"].as_str(),p["name"].as_str().or(p["tool_name"].as_str()),p["call_id"].as_str().or(p["tool_use_id"].as_str()),p["forked_from_id"].as_str().or(p["parent_thread_id"].as_str())])?;
            Ok(())
        })
    }

    pub fn evidence_page(&self, f: &EvidenceFilter) -> rusqlite::Result<Value> {
        let project = f.project.as_deref().map(capture::project_path);
        self.db.with_conn(|c| {
            let latest:i64=c.query_row("SELECT COALESCE(MAX(sequence),0) FROM observations",[],|r| r.get(0))?;
            let through = f.through.map(|v|v.clamp(0,latest)).unwrap_or(latest);
            let mut stmt = c.prepare("SELECT o.sequence,o.id,o.channel,o.source,o.position,o.source_version,o.observed_at,o.received_at,o.session_id,o.turn_id,o.native_id,o.kind,o.project,o.object_hash,o.disposition,o.reason,i.category,i.role,i.tool,i.call_id,i.parent_session_id FROM observations o LEFT JOIN evidence_items i ON i.observation_id=o.id WHERE o.sequence>?1 AND o.sequence<=?2 AND (?3 IS NULL OR o.session_id=?3) AND (?4 IS NULL OR o.project=?4) AND (?5 IS NULL OR o.kind=?5) AND (?6 IS NULL OR julianday(o.observed_at)>=julianday(?6)) AND (?7 IS NULL OR julianday(o.observed_at)<=julianday(?7)) AND (?8 IS NULL OR EXISTS(SELECT 1 FROM task_links l WHERE l.task_id=?8 AND l.status='confirmed' AND l.session_id=o.session_id AND (l.turn_id='' OR l.turn_id=o.turn_id))) AND (?9 IS NULL OR instr(lower(o.kind||' '||COALESCE(i.tool,'')||' '||COALESCE(o.session_id,'')),lower(?9))>0) AND (?11 IS NULL OR i.category=?11 OR (?11='context' AND i.category IN ('usage','compaction'))) ORDER BY o.sequence LIMIT ?10")?;
            let items:Vec<Value> = stmt.query_map(params![f.after.unwrap_or(0),through,f.session,project,f.kind,f.since,f.until,f.task,f.q,f.limit.unwrap_or(50).clamp(1,200)+1,f.category],|r| Ok(json!({
                "sequence":r.get::<_,i64>(0)?,"id":r.get::<_,String>(1)?,"channel":r.get::<_,String>(2)?,"source":r.get::<_,String>(3)?,"position":r.get::<_,String>(4)?,"source_version":r.get::<_,Option<String>>(5)?,"observed_at":r.get::<_,Option<String>>(6)?,"received_at":r.get::<_,String>(7)?,"session_id":r.get::<_,Option<String>>(8)?,"turn_id":r.get::<_,Option<String>>(9)?,"native_id":r.get::<_,Option<String>>(10)?,"kind":r.get::<_,String>(11)?,"project":r.get::<_,Option<String>>(12)?,"object_hash":r.get::<_,Option<String>>(13)?,"disposition":r.get::<_,String>(14)?,"reason":r.get::<_,Option<String>>(15)?,"category":r.get::<_,Option<String>>(16)?,"role":r.get::<_,Option<String>>(17)?,"tool":r.get::<_,Option<String>>(18)?,"call_id":r.get::<_,Option<String>>(19)?,"parent_session_id":r.get::<_,Option<String>>(20)?,"certainty":"observed","schema_version":1
            })))?.collect::<rusqlite::Result<_>>()?;
            let limit=f.limit.unwrap_or(50).clamp(1,200) as usize;
            let more=items.len()>limit;
            let items:Vec<_>=items.into_iter().take(limit).collect();
            let next=if more {items.last().map(|v|v["sequence"].clone())} else {None};
            Ok(json!({"items":items,"next":next,"through":through,"schema_version":1}))
        })
    }

    pub fn evidence_coverage(&self) -> rusqlite::Result<Value> {
        let archive = self.evidence_dir()?;
        let free_bytes = archive
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|p| fs2::available_space(p).ok());
        self.db.with_conn(|c| {
            let versions:Vec<Value>=c.prepare("SELECT channel,source_version,COUNT(*) FROM observations WHERE source_version IS NOT NULL GROUP BY channel,source_version")?.query_map([],|r|Ok(json!({"channel":r.get::<_,String>(0)?,"source_version":r.get::<_,String>(1)?,"observations":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<_>>()?;
            let channels:Vec<Value>=c.prepare("SELECT channel,disposition,reason,COUNT(*),MAX(received_at) FROM observations GROUP BY channel,disposition,reason")?.query_map([],|r|Ok(json!({"channel":r.get::<_,String>(0)?,"disposition":r.get::<_,String>(1)?,"reason":r.get::<_,Option<String>>(2)?,"count":r.get::<_,i64>(3)?,"last_received":r.get::<_,String>(4)?})))?.collect::<rusqlite::Result<_>>()?;
            let bytes:i64=c.query_row("SELECT COALESCE(SUM(bytes),0) FROM evidence_objects",[],|r|r.get(0))?;
            let errors:i64=c.query_row("SELECT COUNT(*) FROM source_files WHERE status='ERROR'",[],|r|r.get(0))?;
            Ok(json!({"schema_version":1,"channels":channels,"observed_versions":versions,"archive_path":archive,"free_bytes":free_bytes,"object_bytes":bytes,"source_errors":errors,"model_request":"not_exposed","role_token_counts":"unavailable","current_context":"not_inferred_from_cumulative_usage","historical_workflow":"only_observed_versions","macos_gui_acceptance":"pending_real_device","secret_detection":"conservative_best_effort","automatic_retention":false}))
        })
    }

    pub fn context_page(&self, filter: &EvidenceFilter) -> rusqlite::Result<Value> {
        let mut f = filter.clone();
        f.category = Some("context".into());
        let mut page = self.evidence_page(&f)?;
        for item in page["items"].as_array_mut().unwrap() {
            let record = item["object_hash"]
                .as_str()
                .map(|hash| self.evidence_object(hash))
                .transpose()?
                .flatten()
                .unwrap_or(Value::Null);
            let p = record.get("payload").unwrap_or(&record);
            let window = p
                .get("model_context_window")
                .or(p.get("context_window"))
                .or(p.pointer("/info/model_context_window"))
                .filter(|v| v.is_number());
            item["context"] = json!({"window_limit":{"value":window,"certainty":if window.is_some(){"observed"}else{"unavailable"}},"request_usage":p.get("usage"),"turn_usage":p.get("turn_token_usage"),"thread_cumulative_usage":p.get("thread_token_usage").or(p.pointer("/info/total_token_usage")),"active_context_tokens":{"value":null,"certainty":"unavailable"},"exact_model_request":false});
        }
        Ok(page)
    }

    pub fn reindex_evidence(&self) -> rusqlite::Result<u64> {
        let mut f = EvidenceFilter {
            limit: Some(100),
            ..Default::default()
        };
        let mut count = 0;
        loop {
            let page = self.evidence_page(&f)?;
            f.through = page["through"].as_i64();
            for item in page["items"].as_array().unwrap() {
                let value = item["object_hash"]
                    .as_str()
                    .map(|h| self.evidence_object(h))
                    .transpose()?
                    .flatten();
                self.project_observation(item["id"].as_str().unwrap(), value.as_ref())?;
                if let Some(value) = &value {
                    self.project_external_task(item["id"].as_str().unwrap(), value)?;
                }
                count += 1;
            }
            f.after = page["next"].as_i64();
            if f.after.is_none() {
                break;
            }
        }
        Ok(count)
    }
}
