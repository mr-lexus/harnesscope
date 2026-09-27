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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IngestPayload {
    Batch(Vec<IngestEvent>),
    Single(IngestEvent),
}

impl IngestPayload {
    pub fn into_events(self) -> Vec<IngestEvent> {
        match self {
            IngestPayload::Batch(events) => events,
            IngestPayload::Single(event) => vec![event],
        }
    }
}
