//! The only boundary from untrusted source content into the evidence archive.
//! No raw payloads or parser errors containing input may be written to disk.
use crate::redact::{redact_value, sha256_digest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_OBJECT: usize = 32 * 1024 * 1024;
pub const SANITIZER_VERSION: u32 = 1;
pub fn exclusion_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("metadata_only_policy") => "metadata_only_policy",
        Some("secret_file_or_opaque_content") => "secret_file_or_opaque_content",
        Some("secret_related_tool_call") => "secret_related_tool_call",
        Some("unpaired_tool_output") => "unpaired_tool_output",
        Some("object_limit") => "object_limit",
        Some("invalid_hook_json") => "invalid_hook_json",
        _ => "excluded_before_import",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationInput {
    pub channel: String,
    pub source: String,
    pub position: String,
    pub source_version: Option<String>,
    pub observed_at: Option<String>,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub native_id: Option<String>,
    pub kind: String,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeContent {
    value: Option<Value>,
    pub disposition: String,
    pub reason: Option<String>,
}

impl SafeContent {
    pub fn with_prior_redaction(mut self, redacted: bool) -> Self {
        if redacted && self.disposition == "retained" {
            self.disposition = "redacted".into();
        }
        self
    }
    pub fn excluded(reason: &str) -> Self {
        Self {
            value: None,
            disposition: "excluded".into(),
            reason: Some(reason.into()),
        }
    }
    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

pub fn sensitive_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase().replace('\\', "/");
    lower
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '"' | '\''
                        | ':'
                        | ','
                        | ';'
                        | '='
                        | '|'
                        | '&'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                        | '?'
                        | '*'
                )
        })
        .any(|part| {
            part.split('/').any(|name| {
                name == ".env"
                    || name.starts_with(".env.")
                    || matches!(
                        name,
                        "auth.json"
                            | "credentials"
                            | "credentials.json"
                            | "id_rsa"
                            | "id_ed25519"
                            | ".aws"
                            | ".ssh"
                    )
                    || name.ends_with(".pem")
                    || name.ends_with(".p12")
                    || name.ends_with(".key")
            })
        })
}

fn unsafe_content(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(k, v)| {
            sensitive_path(k)
                || matches!(
                    k.to_ascii_lowercase().as_str(),
                    "encrypted_content"
                        | "bytesvalue"
                        | "image_url"
                        | "audio"
                        | "env"
                        | "environment_variables"
                )
                || unsafe_content(v)
        }),
        Value::Array(values) => values.iter().any(unsafe_content),
        Value::String(text) => {
            sensitive_path(text)
                || text.to_ascii_lowercase().contains("printenv")
                || (text.to_ascii_lowercase().contains("get-childitem")
                    && text.to_ascii_lowercase().contains("env:"))
                || text.trim() == "env"
                || text.contains("-----BEGIN ")
                || text.contains("data:")
                || text.contains('\0')
        }
        _ => false,
    }
}

/// Deliberately conservative: opaque binary/encrypted payloads and secret-file
/// reads retain an exclusion record rather than an unverifiable partial scrub.
pub fn sanitize(value: &Value) -> SafeContent {
    if unsafe_content(value) {
        return SafeContent::excluded("secret_file_or_opaque_content");
    }
    let clean = redact_value(&sanitize_otlp(value));
    let disposition = if &clean == value {
        "retained"
    } else {
        "redacted"
    };
    SafeContent {
        value: Some(clean),
        disposition: disposition.into(),
        reason: None,
    }
}

fn sanitize_otlp(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut map = map.clone();
            if let Some(key) = map.get("key").and_then(Value::as_str) {
                let probe = redact_value(&serde_json::json!({key:"sentinel"}));
                if probe[key] != "sentinel" {
                    map.insert(
                        "value".into(),
                        serde_json::json!({"stringValue":"[REDACTED_SECRET]"}),
                    );
                }
            }
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, sanitize_otlp(&v)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_otlp).collect()),
        _ => value.clone(),
    }
}

/// Metadata is untrusted too. Never let a token become an ID, path or filename.
pub fn metadata(text: &str) -> String {
    let safe = sanitize(&Value::String(text.into()));
    match safe.value() {
        Some(Value::String(s)) if s == text && s.len() <= 4096 => s.clone(),
        _ => format!("excluded:{}", sha256_digest("unsafe metadata")),
    }
}

/// Normalize Windows extended-length spelling without consulting today's FS.
/// Historical project identity must not depend on whether the directory exists.
pub fn project_path(text: &str) -> String {
    let text = text
        .strip_prefix("\\\\?\\UNC\\")
        .map(|p| format!("\\\\{p}"))
        .unwrap_or_else(|| text.strip_prefix("\\\\?\\").unwrap_or(text).to_owned());
    let windows = text.as_bytes().get(1) == Some(&b':') || text.starts_with("\\\\");
    let text = if windows {
        text.replace('\\', "/")
    } else {
        text
    };
    if text.len() > 3 {
        text.trim_end_matches('/').to_owned()
    } else {
        text
    }
}

pub fn from_record(
    channel: &str,
    source: &str,
    position: &str,
    record: &Value,
) -> ObservationInput {
    let p = record.get("payload").unwrap_or(record);
    let string = |keys: &[&str]| keys.iter().find_map(|k| p[*k].as_str().map(metadata));
    ObservationInput {
        channel: channel.into(),
        source: metadata(source),
        position: metadata(position),
        source_version: string(&["cli_version", "version"]),
        observed_at: record["timestamp"].as_str().map(metadata),
        session_id: string(&["thread_id", "session_id"]),
        turn_id: string(&["turn_id"]),
        native_id: string(&["id", "item_id", "call_id"]),
        kind: metadata(&if record["type"] == "event_msg" {
            format!("event_msg/{}", p["type"].as_str().unwrap_or("unknown"))
        } else {
            record["type"]
                .as_str()
                .or(record["hook_event_name"].as_str())
                .unwrap_or("unknown")
                .to_owned()
        }),
        project: string(&["cwd", "project"]),
    }
}
