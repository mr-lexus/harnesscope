//! Conservative adapter for Codex rollout JSONL. No prompt/response replay,
//! no process-liveness claims, and no inferred costs or anonymous turn IDs.
use crate::{
    domain::events::IngestEvent,
    redact::{redact_secrets, sha256_digest},
    storage::usage::TokenUsage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexState {
    session: Option<String>,
    cwd: String,
    active: Option<Turn>,
    #[serde(default)]
    last_completed: Option<String>,
    pending_context: Option<Value>,
    last_total: Option<TokenUsage>,
    turn_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Turn {
    id: String,
    usage: TokenUsage,
    #[serde(default)]
    native_usage: bool,
}

pub struct Converted {
    pub events: Vec<IngestEvent>,
    pub ignored: bool,
}

pub fn session_id(native: &str) -> String {
    format!("codex_session_{}", sha256_digest(native))
}
pub fn execution_id(session: &str, turn: &str) -> String {
    format!(
        "codex_turn_{}",
        sha256_digest(&format!("{session}\0{turn}"))
    )
}

impl CodexState {
    fn event(
        &self,
        timestamp: &str,
        kind: &str,
        discriminator: &str,
        turn: Option<&str>,
        payload: Value,
    ) -> IngestEvent {
        let native = self
            .session
            .as_deref()
            .expect("session checked before conversion");
        IngestEvent {
            event_id: format!(
                "codex_evt_{}",
                sha256_digest(&format!("{native}\0{kind}\0{discriminator}"))
            ),
            timestamp: timestamp.into(),
            event_type: kind.into(),
            source: "codex-rollout/v1".into(),
            runtime_id: Some(format!("codex_transcript_{}", sha256_digest(native))),
            session_id: Some(session_id(native)),
            execution_id: turn.map(|t| execution_id(native, t)),
            agent_instance_id: None,
            payload,
            extra: Default::default(),
        }
    }

    pub fn convert(&mut self, record: &Value, include_content: bool) -> Result<Converted, String> {
        let timestamp = record["timestamp"]
            .as_str()
            .ok_or("Record has no timestamp")?;
        chrono::DateTime::parse_from_rfc3339(timestamp)
            .map_err(|_| "Record timestamp is not RFC3339")?;
        let kind = record["type"].as_str().ok_or("Record has no type")?;
        let p = &record["payload"];
        let record_id = || sha256_digest(&record.to_string());
        let mut events = Vec::new();
        let mut ignored = false;
        if kind == "session_meta" {
            // session_id can name the *root* in modern rollouts; id names this thread.
            let native = p["id"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 256)
                .ok_or("session_meta requires thread id")?;
            if self.session.as_deref().is_some_and(|s| s != native) {
                return Err("Thread identity changed inside one rollout".into());
            }
            if self.session.is_some() {
                // Repeated metadata is not a new session. In particular it must
                // not reset the cumulative usage baseline or an active turn.
                return Ok(Converted { events, ignored });
            }
            self.session = Some(native.into());
            self.cwd = p["cwd"].as_str().unwrap_or("").chars().take(4096).collect();
            // A fork/paginated prefix can inherit earlier cumulative usage. Do not
            // charge inherited tokens to its first observed turn.
            self.last_total = if p["forked_from_id"].is_string()
                || !p["history_base"].is_null()
                || p["parent_thread_id"].is_string()
            {
                None
            } else {
                Some(TokenUsage::default())
            };
            events.push(self.event(timestamp,"runtime.started","meta",None,json!({
                "runner_name":"codex","runner_version":p["cli_version"].as_str().unwrap_or("UNKNOWN"),
                "surface":"transcript","cwd":self.cwd,"hostname":"UNKNOWN","os":"UNKNOWN"
            })));
            events.push(self.event(
                timestamp,
                "session.identified",
                "meta",
                None,
                json!({
                    "runner_name":"codex","native_session_id":native,
                    "parent_session_id":p["forked_from_id"].as_str(),
                    "fork_reason":p["forked_from_id"].as_str().map(|_|"native_rollout"),
                    "history_base_present":!p["history_base"].is_null()
                }),
            ));
            return Ok(Converted { events, ignored });
        }
        if self.session.is_none() {
            return Err("First record must be session_meta with a thread id".into());
        }
        match kind {
            "turn_context" => {
                let turn = p["turn_id"]
                    .as_str()
                    .or(self.active.as_ref().map(|a| a.id.as_str()));
                // Only retain selected metadata: never instructions, sandbox config or content.
                let context = crate::redact::redact_value(
                    &json!({"turn_id":turn,"model":p["model"].as_str(),"reasoning_effort":p["effort"].as_str(),"cwd":p["cwd"].as_str()}),
                );
                if let Some(active) = &self.active {
                    if turn == Some(active.id.as_str()) {
                        events.push(self.event(
                            timestamp,
                            "execution.context",
                            &record_id(),
                            Some(&active.id),
                            context,
                        ));
                    } else {
                        self.pending_context = Some(context);
                    }
                } else {
                    self.pending_context = Some(context);
                }
            }
            "event_msg" => match p["type"].as_str().unwrap_or("") {
                "task_started" | "turn_started" => {
                    let id = p["turn_id"]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 256)
                        .ok_or(
                            "task_started requires turn_id; this rollout version is unsupported",
                        )?;
                    if self.active.as_ref().is_none_or(|a| a.id != id) {
                        self.turn_index = self.turn_index.checked_add(1).ok_or("Too many turns")?;
                        self.active = Some(Turn {
                            id: id.into(),
                            usage: TokenUsage::default(),
                            native_usage: false,
                        });
                    }
                    let mut payload = json!({"capture_scope":"TURN","native_execution_id":id,"turn_index":self.turn_index,"boundary_evidence":"native_turn_started"});
                    if let Some(context) = self.pending_context.take() {
                        if context["turn_id"].as_str() == Some(id) {
                            for field in ["model", "reasoning_effort"] {
                                if context[field].is_string() {
                                    payload[field] = context[field].clone();
                                }
                            }
                        }
                    }
                    events.push(self.event(timestamp, "execution.started", id, Some(id), payload));
                }
                "task_complete" | "turn_complete" | "turn_aborted" => {
                    let id = p["turn_id"]
                        .as_str()
                        .or(self.active.as_ref().map(|a| a.id.as_str()));
                    if let Some(id) = id {
                        // A completion without an observed start must not invent a turn.
                        if self.active.as_ref().is_some_and(|a| a.id == id) {
                            let status = if p["type"] == "turn_aborted" {
                                "CANCELLED"
                            } else if !p["error"].is_null() {
                                "FAILED"
                            } else {
                                "COMPLETED"
                            };
                            let mut payload = json!({"status":status,"boundary_evidence":"native_terminal_event"});
                            if let Some(ms) = p["duration_ms"].as_i64().filter(|n| *n >= 0) {
                                payload["duration_ms"] = json!(ms);
                            }
                            if include_content {
                                if let Some(message) = p["error"]["message"].as_str() {
                                    payload["error_message"] = json!(redact_secrets(message)
                                        .chars()
                                        .take(1000)
                                        .collect::<String>());
                                }
                            }
                            events.push(self.event(
                                timestamp,
                                "execution.completed",
                                id,
                                Some(id),
                                payload,
                            ));
                            self.last_completed = Some(id.to_string());
                            self.active = None;
                        } else {
                            ignored = true;
                        }
                    } else {
                        ignored = true;
                    }
                }
                "user_message" => {
                    if include_content {
                        if let Some(active) = &self.active {
                            if let Some(message) = p["message"].as_str() {
                                let summary = redact_secrets(message)
                                    .chars()
                                    .take(200)
                                    .collect::<String>();
                                events.push(self.event(
                                    timestamp,
                                    "execution.context",
                                    &record_id(),
                                    Some(&active.id),
                                    json!({"prompt_summary":summary}),
                                ));
                            }
                        }
                    }
                }
                "token_count" => {
                    if let Some(total) = p
                        .pointer("/info/total_token_usage")
                        .filter(|v| !v.is_null())
                    {
                        let total: TokenUsage = serde_json::from_value(total.clone())
                            .map_err(|_| "Unsupported token usage shape")?;
                        if !total.valid() {
                            return Err("Invalid cumulative token counters".into());
                        }
                        if self.active.as_ref().is_some_and(|a| a.native_usage) {
                            // Legacy totals can reset at compaction and are not the same
                            // counter as thread_token_usage. Native per-turn totals win.
                            self.last_total = Some(total);
                            return Ok(Converted { events, ignored });
                        }
                        let delta = self
                            .last_total
                            .as_ref()
                            .and_then(|old| total.checked_delta(old));
                        if let (Some(delta), Some(active)) = (delta, &mut self.active) {
                            active.usage = active
                                .usage
                                .checked_add(&delta)
                                .ok_or("Token counter overflow")?;
                            let id = active.id.clone();
                            let usage = serde_json::to_value(&active.usage).unwrap();
                            events.push(self.event(
                                timestamp,
                                "usage.observed",
                                &record_id(),
                                Some(&id),
                                usage,
                            ));
                        } else {
                            ignored = true;
                        }
                        self.last_total = Some(total);
                    }
                }
                // These do not prove turn termination or tool invocation.
                "agent_message"
                | "agent_reasoning"
                | "user_message_delta"
                | "error"
                | "warning"
                | "item_completed"
                | "thread_settings_applied" => {}
                _ => {
                    ignored = true;
                }
            },
            "token_usage_record" => {
                // New Codex records carry cumulative per-turn counters. Never add them
                // to the legacy token_count stream: both describe the same usage.
                let turn = p["turn_id"].as_str();
                let known = turn.is_some_and(|id| {
                    self.active.as_ref().is_some_and(|a| a.id == id)
                        || self.last_completed.as_deref() == Some(id)
                });
                let same_thread = p["thread_id"]
                    .as_str()
                    .is_none_or(|id| self.session.as_deref() == Some(id));
                if known && same_thread {
                    let usage: TokenUsage = serde_json::from_value(p["turn_token_usage"].clone())
                        .map_err(|_| "Unsupported turn token usage shape")?;
                    if !usage.valid() {
                        return Err("Invalid turn token counters".into());
                    }
                    if let Some(active) = &mut self.active {
                        if Some(active.id.as_str()) == turn {
                            active.usage = usage.clone();
                            active.native_usage = true;
                        }
                    }
                    events.push(self.event(
                        timestamp,
                        "usage.observed",
                        &record_id(),
                        turn,
                        serde_json::to_value(usage).unwrap(),
                    ));
                } else {
                    ignored = true;
                }
            }
            "response_item" => {
                let item = p.get("item").filter(|v| v.is_object()).unwrap_or(p);
                if matches!(
                    item["type"].as_str(),
                    Some("function_call" | "custom_tool_call" | "local_shell_call")
                ) {
                    if let Some(active) = &self.active {
                        if let Some(call) = item["call_id"].as_str().or(item["id"].as_str()) {
                            let name = item["name"].as_str().unwrap_or("local_shell");
                            let name: String = redact_secrets(name).chars().take(256).collect();
                            events.push(self.event(timestamp,"component.invoked",&format!("{}:{call}",active.id),Some(&active.id),json!({
                                "name":name,"component_type":if name.starts_with("mcp__") {"MCP"} else {"TOOL"},
                                "invocations_count":1,"details":{"call_id":call,"evidence":"native_tool_call"}
                            })));
                        } else {
                            ignored = true;
                        }
                    } else {
                        ignored = true;
                    }
                }
                // Raw instructions, reasoning, tool arguments and results are deliberately omitted.
            }
            "compacted" | "world_state" => {}
            _ => {
                ignored = true;
            }
        }
        Ok(Converted { events, ignored })
    }
}
