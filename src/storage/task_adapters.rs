//! Local declarative mappings for observed task-reading MCP calls. No MCP client,
//! credentials, executable plugins, or provider-specific assumptions live here.
use super::Repository;
use crate::{capture, redact::sha256_digest};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TaskAdapter {
    pub schema_version: u32,
    pub id: String,
    pub namespace: String,
    pub tool: String,
    pub task_id: TaskIdField,
    pub title_pointer: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TaskIdField {
    pub source: FieldSource,
    pub pointer: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FieldSource {
    Arguments,
    Result,
}
type MatchedCall = (String, String, String, String, String, Option<String>);

fn invalid() -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName("Invalid task adapter; see docs/TASK_ADAPTERS.md".into())
}
fn pointer_valid(s: &str) -> bool {
    if s.len() > 512 || (!s.is_empty() && !s.starts_with('/')) {
        return false;
    }
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c.is_control() || (c == '~' && !matches!(chars.next(), Some('0' | '1'))) {
            return false;
        }
    }
    true
}
impl TaskAdapter {
    fn validate(&self) -> rusqlite::Result<()> {
        let identifier = |s: &str| {
            !s.is_empty()
                && s.len() <= 80
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        };
        if self.schema_version != 1
            || !identifier(&self.id)
            || !identifier(&self.namespace)
            || self.tool.is_empty()
            || self.tool.len() > 256
            || self
                .tool
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
            || !pointer_valid(&self.task_id.pointer)
            || self
                .title_pointer
                .as_deref()
                .is_some_and(|p| !pointer_valid(p))
        {
            return Err(invalid());
        }
        let value = serde_json::to_value(self).map_err(|_| invalid())?;
        if capture::sanitize(&value).value() != Some(&value) {
            return Err(invalid());
        }
        Ok(())
    }
}
fn payload(record: &Value) -> &Value {
    let p = record.get("payload").unwrap_or(record);
    p.get("item").unwrap_or(p)
}
fn decode(value: &Value) -> Value {
    value
        .as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| value.clone())
}
fn identity(value: &Value) -> Option<String> {
    let id = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
        _ => return None,
    };
    // Never merge different redacted identifiers into one apparent task.
    if id.trim().is_empty()
        || id.len() > 500
        || id.chars().any(char::is_control)
        || id.contains("[REDACTED")
        || id.contains("[excluded")
        || capture::metadata(&id) != id
    {
        return None;
    }
    Some(id)
}

impl Repository {
    pub fn register_task_adapter(&self, adapter: &TaskAdapter) -> rusqlite::Result<()> {
        adapter.validate()?;
        let config = serde_json::to_string(adapter).map_err(|_| invalid())?;
        self.transaction(|| self.db.with_conn(|c| {
            let old: Option<String> = c.query_row("SELECT config_json FROM task_adapters WHERE id=?1", [&adapter.id], |r| r.get(0)).optional()?;
            if old.as_deref().is_some_and(|s| s != config) { return Err(invalid()); }
            c.execute("INSERT INTO task_adapters(id,tool,config_json,enabled) VALUES(?1,?2,?3,1) ON CONFLICT(id) DO UPDATE SET enabled=1", params![adapter.id,adapter.tool,config])?;
            Ok(())
        }))
    }
    pub fn task_adapters(&self) -> rusqlite::Result<Value> {
        self.db.with_conn(|c| {
            let items: Vec<Value> = c.prepare("SELECT config_json,enabled FROM task_adapters ORDER BY id")?.query_map([], |r| {
                let config: String = r.get(0)?;
                Ok(json!({"adapter":serde_json::from_str::<Value>(&config).map_err(|_| invalid())?,"enabled":r.get::<_,bool>(1)?}))
            })?.collect::<rusqlite::Result<_>>()?;
            Ok(json!({"schema_version":1,"items":items}))
        })
    }
    pub fn disable_task_adapter(&self, id: &str) -> rusqlite::Result<bool> {
        self.db.with_conn(|c| {
            c.execute(
                "UPDATE task_adapters SET enabled=0 WHERE id=?1 AND enabled=1",
                [id],
            )
            .map(|n| n > 0)
        })
    }
    pub fn project_external_task(&self, observation: &str, record: &Value) -> rusqlite::Result<()> {
        let item = payload(record);
        let Some(call) = item["call_id"].as_str().or(item["tool_use_id"].as_str()) else {
            return Ok(());
        };
        let Some(raw) = item.get("output").or(item.get("tool_response")) else {
            return Ok(());
        };
        let row: Option<MatchedCall> = self.db.with_conn(|c| c.query_row(
            "SELECT previous.object_hash,current.session_id,current.object_hash,COALESCE(current.observed_at,current.received_at),a.config_json,current.project FROM observations current JOIN observations previous ON previous.id=(SELECT o.id FROM observations o JOIN evidence_items e ON e.observation_id=o.id WHERE o.session_id=current.session_id AND o.sequence<current.sequence AND e.call_id=?2 AND e.tool IS NOT NULL AND (o.turn_id IS NULL OR current.turn_id IS NULL OR o.turn_id=current.turn_id) ORDER BY o.sequence DESC LIMIT 1) JOIN evidence_items i ON i.observation_id=previous.id JOIN task_adapters a ON a.tool=i.tool AND a.enabled=1 WHERE current.id=?1 AND previous.object_hash IS NOT NULL",
            params![observation,call], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional())?;
        let Some((call_hash, session, hash, time, config, project)) = row else {
            return Ok(());
        };
        let adapter: TaskAdapter = serde_json::from_str(&config).map_err(|_| invalid())?;
        let Some(invocation) = self.evidence_object(&call_hash)? else {
            return Ok(());
        };
        let envelope = decode(raw);
        if envelope["isError"] == true
            || envelope.get("error").is_some_and(|v| !v.is_null())
            || item["is_error"] == true
        {
            return Ok(());
        }
        // Standard MCP structured results or one JSON/text content block.
        let result = if let Some(s) = envelope.get("structuredContent") {
            s.clone()
        } else if let Some(blocks) = envelope["content"].as_array().filter(|a| a.len() == 1) {
            blocks[0]
                .get("text")
                .map(decode)
                .unwrap_or_else(|| envelope.clone())
        } else {
            envelope
        };
        let args = decode(
            payload(&invocation)
                .get("arguments")
                .or(payload(&invocation).get("tool_input"))
                .unwrap_or(&Value::Null),
        );
        let source = match adapter.task_id.source {
            FieldSource::Arguments => &args,
            FieldSource::Result => &result,
        };
        let Some(native) = source.pointer(&adapter.task_id.pointer).and_then(identity) else {
            return Ok(());
        };
        let task = format!("mcp:{}:{}", adapter.namespace, native);
        let title = adapter
            .title_pointer
            .as_deref()
            .and_then(|p| result.pointer(p))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty() && s.len() <= 500)
            .map(capture::metadata)
            .unwrap_or_else(|| task.clone());
        let version = sha256_digest(&format!("{task}\0{observation}"));
        self.transaction(|| self.db.with_conn(|c| {
            c.execute("INSERT OR IGNORE INTO retro_tasks(id,title,project,created_at) VALUES(?1,?2,?3,?4)",params![task,title,project,time])?;
            c.execute("INSERT OR IGNORE INTO task_links(task_id,session_id,status,evidence_id) VALUES(?1,?2,'confirmed',?3)",params![task,session,observation])?;
            c.execute("INSERT OR IGNORE INTO external_task_versions(id,task_id,observation_id,object_hash,observed_at) VALUES(?1,?2,?3,?4,?5)",params![version,task,observation,hash,time])?;
            Ok(())
        }))
    }
}
