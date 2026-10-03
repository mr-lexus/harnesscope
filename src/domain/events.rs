use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

fn generate_event_id() -> String {
    format!("evt_{}", Uuid::new_v4().simple())
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestEvent {
    #[serde(default = "generate_event_id")]
    pub event_id: String,

    #[serde(default = "current_timestamp")]
    pub timestamp: String,

    pub event_type: String,
    pub source: String,

    pub runtime_id: Option<String>,
    pub session_id: Option<String>,
    pub execution_id: Option<String>,
    pub agent_instance_id: Option<String>,

    #[serde(default)]
    pub payload: serde_json::Value,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl IngestEvent {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("event_id", &self.event_id),
            ("event_type", &self.event_type),
            ("source", &self.source),
        ] {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(format!("{name} must contain 1–256 bytes"));
            }
        }
        if chrono::DateTime::parse_from_rfc3339(&self.timestamp).is_err() {
            return Err("timestamp must be RFC3339".into());
        }
        if !self.payload.is_object() {
            return Err("payload must be a JSON object".into());
        }
        for id in [
            &self.runtime_id,
            &self.session_id,
            &self.execution_id,
            &self.agent_instance_id,
        ]
        .into_iter()
        .flatten()
        {
            if id.trim().is_empty() || id.len() > 256 {
                return Err("Entity IDs must contain 1–256 bytes".into());
            }
        }
        if let Some(scope) = self.payload.get("capture_scope") {
            if !matches!(scope.as_str(), Some("PROCESS" | "TURN" | "UNKNOWN")) {
                return Err("capture_scope must be PROCESS, TURN or UNKNOWN".into());
            }
        }
        if self
            .payload
            .get("duration_ms")
            .is_some_and(|v| v.as_i64().is_none_or(|n| n < 0))
        {
            return Err("duration_ms must be a non-negative integer".into());
        }
        if matches!(
            self.event_type.as_str(),
            "usage.observed" | "execution.context"
        ) && self.execution_id.is_none()
        {
            return Err("Execution evidence requires execution_id".into());
        }
        if self.event_type == "usage.observed" {
            let usage: crate::storage::usage::TokenUsage =
                serde_json::from_value(self.payload.clone())
                    .map_err(|_| "Invalid token counters")?;
            if !usage.valid() {
                return Err("Invalid token counters".into());
            }
        }
        if self.event_type == "execution.completed" && self.execution_id.is_none() {
            return Err("execution.completed requires execution_id".into());
        }
        if matches!(
            self.event_type.as_str(),
            "runtime.stopped" | "runtime.heartbeat"
        ) && self.runtime_id.is_none()
        {
            return Err("runtime.stopped requires runtime_id".into());
        }
        if matches!(
            self.event_type.as_str(),
            "runtime.stopped" | "execution.completed"
        ) && self.payload.get("status").is_some_and(|v| {
            !matches!(
                v.as_str(),
                Some("COMPLETED" | "FAILED" | "CANCELLED" | "UNKNOWN")
            )
        }) {
            return Err("Completion status must be COMPLETED, FAILED, CANCELLED or UNKNOWN".into());
        }
        if self.event_type == "runtime.started" && self.runtime_id.is_none() {
            return Err("runtime.started requires runtime_id".into());
        }
        if self.event_type == "execution.started" && self.execution_id.is_none() {
            return Err("execution.started requires execution_id".into());
        }
        if matches!(
            self.event_type.as_str(),
            "session.identified" | "session.started"
        ) && self.session_id.is_none()
        {
            return Err("Session events require session_id".into());
        }
        if self
            .payload
            .get("invocations_count")
            .is_some_and(|v| v.as_i64().is_none_or(|n| n < 0 || n > i32::MAX as i64))
        {
            return Err("invocations_count must be a non-negative 32-bit integer".into());
        }
        if self
            .payload
            .get("exit_code")
            .is_some_and(|v| !v.is_null() && v.as_i64().is_none_or(|n| i32::try_from(n).is_err()))
        {
            return Err("exit_code must be null or a signed 32-bit integer".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IngestPayload {
    Batch(Vec<IngestEvent>),
    Single(Box<IngestEvent>),
}

impl IngestPayload {
    pub fn into_events(self) -> Vec<IngestEvent> {
        match self {
            IngestPayload::Batch(events) => events,
            IngestPayload::Single(event) => vec![*event],
        }
    }
}
