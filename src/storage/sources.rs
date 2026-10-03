use super::Repository;
use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySource {
    pub id: String,
    pub adapter: String,
    pub path: String,
    pub enabled: bool,
    pub include_content: bool,
    pub created_at: String,
    pub last_scan_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceFile {
    pub source_id: String,
    pub path: String,
    pub byte_offset: u64,
    pub line_number: u64,
    pub prefix_hash: String,
    #[serde(skip_serializing)]
    pub state_json: String,
    pub events_count: u64,
    pub ignored_count: u64,
    pub status: String,
    pub last_error: Option<String>,
    pub updated_at: String,
    pub file_size: u64,
    pub modified_stamp: String,
}

impl Repository {
    pub fn add_source(
        &self,
        path: &std::path::Path,
        include_content: bool,
    ) -> std::result::Result<TelemetrySource, String> {
        let path = path
            .canonicalize()
            .map_err(|_| "The source path must be an existing file or directory")?;
        if !path.is_file() && !path.is_dir() {
            return Err("Source must be a regular file or directory".into());
        }
        let path = path
            .to_str()
            .ok_or("Source path must be Unicode")?
            .to_string();
        let source = TelemetrySource {
            id: format!("src_{}", crate::redact::sha256_digest(&path)),
            adapter: "codex-rollout/v1".into(),
            path,
            enabled: true,
            include_content,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_scan_at: None,
            last_success_at: None,
            last_error: None,
        };
        self.db.with_conn(|conn| {
            conn.execute("INSERT INTO telemetry_sources(id,adapter,path,enabled,include_content,created_at) VALUES(?1,?2,?3,1,?4,?5) ON CONFLICT(path) DO NOTHING",params![source.id,source.adapter,source.path,source.include_content,source.created_at])?;
            Ok(())
        }).map_err(|e|e.to_string())?;
        let existing = self
            .list_sources()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|s| s.id == source.id)
            .ok_or("Source disappeared")?;
        if existing.include_content != include_content {
            return Err("Source already exists with a different content policy; remove its registration before re-adding it. Stored events are never rewritten.".into());
        }
        Ok(existing)
    }

    pub fn list_sources(&self) -> Result<Vec<TelemetrySource>> {
        self.db.with_conn(|conn| conn.prepare("SELECT id,adapter,path,enabled,include_content,created_at,last_scan_at,last_success_at,last_error FROM telemetry_sources ORDER BY created_at,id")?.query_map([],|r|Ok(TelemetrySource {
            id:r.get(0)?,adapter:r.get(1)?,path:r.get(2)?,enabled:r.get(3)?,include_content:r.get(4)?,created_at:r.get(5)?,last_scan_at:r.get(6)?,last_success_at:r.get(7)?,last_error:r.get(8)?,
        }))?.collect())
    }

    pub fn source_enabled(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|c| {
            c.query_row(
                "SELECT enabled FROM telemetry_sources WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map(|v| v.unwrap_or(false))
        })
    }

    pub fn set_source_enabled(&self, id: &str, enabled: bool) -> Result<bool> {
        self.db.with_conn(|c| {
            c.execute(
                "UPDATE telemetry_sources SET enabled=?2 WHERE id=?1",
                params![id, enabled],
            )
            .map(|n| n > 0)
        })
    }

    pub fn remove_source(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|c| {
            c.execute("DELETE FROM telemetry_sources WHERE id=?1", [id])
                .map(|n| n > 0)
        })
    }

    pub fn list_source_files(&self, id: &str) -> Result<Vec<SourceFile>> {
        self.db.with_conn(|conn| conn.prepare("SELECT source_id,path,byte_offset,line_number,prefix_hash,state_json,events_count,ignored_count,status,last_error,updated_at,file_size,modified_stamp FROM source_files WHERE source_id=?1 ORDER BY updated_at,path")?.query_map([id],|r|Ok(SourceFile {
            source_id:r.get(0)?,path:r.get(1)?,byte_offset:r.get(2)?,line_number:r.get(3)?,prefix_hash:r.get(4)?,state_json:r.get(5)?,events_count:r.get(6)?,ignored_count:r.get(7)?,status:r.get(8)?,last_error:r.get(9)?,updated_at:r.get(10)?,file_size:r.get(11)?,modified_stamp:r.get(12)?,
        }))?.collect())
    }

    /// Count diagnostics without loading every parser checkpoint a second time.
    pub fn source_error_count(&self, id: &str) -> Result<usize> {
        self.db.with_conn(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM source_files WHERE source_id=?1 AND status='ERROR'",
                [id],
                |r| r.get(0),
            )
        })
    }

    /// CAS protects against a second collector that read an older checkpoint.
    pub fn save_source_file(&self, file: &SourceFile, expected_offset: u64) -> Result<()> {
        self.db.with_conn(|conn| {
            let current: Option<u64> = conn.query_row("SELECT byte_offset FROM source_files WHERE source_id=?1 AND path=?2",params![file.source_id,file.path],|r|r.get(0)).optional()?;
            if current.unwrap_or(0)!=expected_offset { return Err(rusqlite::Error::InvalidParameterName("Source checkpoint changed; retry scan".into())); }
            conn.execute("INSERT INTO source_files VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(source_id,path) DO UPDATE SET byte_offset=excluded.byte_offset,line_number=excluded.line_number,prefix_hash=excluded.prefix_hash,state_json=excluded.state_json,events_count=excluded.events_count,ignored_count=excluded.ignored_count,status=excluded.status,last_error=excluded.last_error,updated_at=excluded.updated_at,file_size=excluded.file_size,modified_stamp=excluded.modified_stamp",
                params![file.source_id,file.path,file.byte_offset,file.line_number,file.prefix_hash,file.state_json,file.events_count,file.ignored_count,file.status,file.last_error,file.updated_at,file.file_size,file.modified_stamp])?;
            Ok(())
        })
    }

    pub fn finish_source_scan(&self, id: &str, error: Option<&str>) -> Result<()> {
        self.db.with_conn(|c| {
            c.execute("UPDATE telemetry_sources SET last_scan_at=?2,last_success_at=CASE WHEN ?3 IS NULL THEN ?2 ELSE last_success_at END,last_error=?3 WHERE id=?1",params![id,chrono::Utc::now().to_rfc3339(),error])?;
            Ok(())
        })
    }
}

impl Repository {
    /// Small aggregate for the global collection indicator; never reads parser state or payloads.
    pub fn collection_status(&self) -> Result<serde_json::Value> {
        let sources = self.list_sources()?;
        let enabled: Vec<_> = sources.iter().filter(|s| s.enabled).collect();
        let now = chrono::Utc::now();
        let stale = enabled.iter().any(|s| {
            s.last_scan_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .is_some_and(|t| now.signed_duration_since(t).num_seconds() > 30)
        });
        let starting = enabled.iter().any(|s| s.last_scan_at.is_none());
        self.db.with_conn(|c| {
            let (files,backlog,errors,events): (u64,u64,u64,u64) = c.query_row("SELECT COUNT(*),COALESCE(SUM(f.status='BACKLOG'),0),COALESCE(SUM(f.status='ERROR'),0),COALESCE(SUM(f.events_count),0) FROM source_files f JOIN telemetry_sources s ON s.id=f.source_id WHERE s.enabled=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
            let status=if sources.is_empty() {"NOT_CONNECTED"} else if enabled.is_empty() {"PAUSED"} else if errors>0 || enabled.iter().any(|s|s.last_error.is_some()) {"NEEDS_ATTENTION"} else if stale {"OVERDUE"} else if starting {"STARTING"} else if backlog>0 {"CATCHING_UP"} else {"COLLECTING"};
            let database=c.path().and_then(|p| std::path::Path::new(p).canonicalize().ok());
            Ok(serde_json::json!({"status":status,"connected_sources":sources.len(),"enabled_sources":enabled.len(),"files_inspected":files,"backlog_files":backlog,"error_files":errors,"converted_events":events,
                "last_scan_at":enabled.iter().filter_map(|s|s.last_scan_at.as_ref()).max(),"database_path":database,"version":env!("CARGO_PKG_VERSION") }))
        })
    }
}
