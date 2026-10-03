use chrono::Utc;
use rusqlite::Result;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::domain::events::IngestEvent;
use crate::domain::models::*;
use crate::redact::{redact_secrets, redact_value, sha256_digest};
use crate::storage::Repository;

pub struct CorrelationEngine {
    repo: Arc<Repository>,
    process_lock: Mutex<()>,
}

impl CorrelationEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self {
            repo,
            process_lock: Mutex::new(()),
        }
    }

    pub fn process_event(&self, event: &IngestEvent) -> Result<String> {
        self.process_events_with(std::slice::from_ref(event), || Ok(()))?;
        Ok(event.event_id.clone())
    }

    /// Source checkpoints commit together with every projected event in a batch.
    pub fn process_events_with(
        &self,
        events: &[IngestEvent],
        checkpoint: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        for event in events {
            event
                .validate()
                .map_err(rusqlite::Error::InvalidParameterName)?;
        }
        let _guard = self.process_lock.lock().unwrap_or_else(|p| p.into_inner());
        self.repo.transaction(|| {
            for event in events {
                if self.repo.event_exists(&event.event_id)? {
                    continue;
                }
                let mut safe = event.clone();
                safe.payload = redact_value(&safe.payload);
                if let Some(id) = &safe.session_id {
                    safe.session_id = Some(self.repo.canonical_session_id(id)?);
                }
                if let Some(id) = &safe.execution_id {
                    if let Some(execution) = self.repo.find_execution_by_id(id)? {
                        if safe
                            .runtime_id
                            .as_ref()
                            .is_some_and(|id| id != &execution.runtime_id)
                            || safe
                                .session_id
                                .as_ref()
                                .is_some_and(|id| id != &execution.session_id)
                        {
                            return Err(rusqlite::Error::InvalidParameterName(
                                "Execution identity does not match its runtime/session".into(),
                            ));
                        }
                        safe.runtime_id = Some(execution.runtime_id);
                        safe.session_id = Some(execution.session_id);
                    }
                }
                self.process_event_inner(&safe)?;
            }
            checkpoint()
        })
    }

    fn process_event_inner(&self, event: &IngestEvent) -> Result<String> {
        let timestamp = if event.timestamp.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            chrono::DateTime::parse_from_rfc3339(&event.timestamp)
                .map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid RFC3339 timestamp".into())
                })?
                .with_timezone(&Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        };

        match event.event_type.as_str() {
            "runtime.heartbeat" => {
                self.repo
                    .observe_runtime(event.runtime_id.as_deref().unwrap(), &timestamp)?;
                // Latest observation is sufficient: do not grow the raw timeline with heartbeats.
            }
            "execution.context" => self.handle_execution_context(event, &timestamp)?,
            "usage.observed" => {
                self.repo.save_usage(event, &timestamp)?;
                self.record_raw_event(event, &timestamp)?;
            }
            "runtime.started" => self.handle_runtime_started(event, &timestamp)?,
            "runtime.stopped" => self.handle_runtime_stopped(event, &timestamp)?,
            "session.identified" | "session.started" => {
                self.handle_session_identified(event, &timestamp)?
            }
            "execution.started" => self.handle_execution_started(event, &timestamp)?,
            "execution.completed" => self.handle_execution_completed(event, &timestamp)?,
            "agent.started" | "subagent.started" => self.handle_agent_started(event, &timestamp)?,
            "component.discovered"
            | "component.invoked"
            | "mcp.discovered"
            | "mcp.invoked"
            | "skill.discovered"
            | "skill.invoked"
            | "plugin.discovered"
            | "plugin.invoked" => self.handle_component_event(event, &timestamp)?,
            "git.snapshot" => self.handle_git_snapshot(event, &timestamp)?,
            "config.snapshot" => self.handle_config_snapshot(event, &timestamp)?,
            _ => {
                // Generic / unknown event: record in events table safely
                self.record_raw_event(event, &timestamp)?;
            }
        }

        Ok(event.event_id.clone())
    }

    fn record_raw_event(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let redacted_payload = serde_json::to_string(&redact_value(&event.payload))
            .unwrap_or_else(|_| "{}".to_string());

        let e = Event {
            id: None,
            event_id: event.event_id.clone(),
            timestamp: timestamp.to_string(),
            runtime_id: event.runtime_id.clone(),
            session_id: event.session_id.clone(),
            execution_id: event.execution_id.clone(),
            agent_instance_id: event.agent_instance_id.clone(),
            event_type: event.event_type.clone(),
            source: event.source.clone(),
            payload_json: redacted_payload,
        };
        self.repo.save_event(&e)
    }

    fn handle_runtime_started(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let runtime_id = event
            .runtime_id
            .clone()
            .unwrap_or_else(|| format!("run_{}", Uuid::new_v4().simple()));
        if self
            .repo
            .find_runtime_by_id(&runtime_id)?
            .is_some_and(|r| r.ended_at.is_some())
        {
            return self.record_raw_event(event, timestamp);
        }
        let p = &event.payload;

        let runner_name = p
            .get("runner_name")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let runner_version = p
            .get("runner_version")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let surface = p
            .get("surface")
            .and_then(|v| v.as_str())
            .unwrap_or("cli")
            .to_string();
        let pid = p.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
        let hostname = p
            .get("hostname")
            .and_then(|v| v.as_str())
            .unwrap_or("localhost")
            .to_string();
        let os = p
            .get("os")
            .and_then(|v| v.as_str())
            .unwrap_or(std::env::consts::OS)
            .to_string();
        let cwd = p
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let raw_cmd = p
            .get("command_line")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let command_line = redact_secrets(&raw_cmd);

        let runtime = RuntimeInstance {
            id: runtime_id.clone(),
            runner_name,
            runner_version,
            surface: surface.clone(),
            pid,
            hostname,
            os,
            cwd,
            command_line,
            started_at: timestamp.to_string(),
            ended_at: None,
            exit_code: None,
            status: if surface == "transcript" {
                "UNKNOWN"
            } else {
                "RUNNING"
            }
            .to_string(),
        };

        self.repo.save_runtime_instance(&runtime)?;
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_runtime_stopped(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        if let Some(runtime_id) = &event.runtime_id {
            if self.repo.find_runtime_by_id(runtime_id)?.is_none() {
                return Err(rusqlite::Error::InvalidParameterName(
                    "Runtime must be started before stopping".into(),
                ));
            }
            let p = &event.payload;
            let exit_code = p
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let status = if let Some(code) = exit_code {
                if code == 0 {
                    "COMPLETED"
                } else {
                    "FAILED"
                }
            } else {
                p.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
            };

            self.repo
                .update_runtime_stopped(runtime_id, timestamp, exit_code, status)?;
            self.repo.db.with_conn(|conn| {
                conn.execute("UPDATE executions SET status='UNKNOWN',ended_at=?2 WHERE runtime_id=?1 AND status='RUNNING'",rusqlite::params![runtime_id,timestamp])?;
                conn.execute("UPDATE agent_instances SET status='UNKNOWN',ended_at=?2 WHERE status='RUNNING' AND execution_id IN (SELECT id FROM executions WHERE runtime_id=?1)",rusqlite::params![runtime_id,timestamp])?;
                conn.execute("UPDATE runtime_session_bindings SET unbound_at=?2 WHERE runtime_id=?1 AND unbound_at IS NULL",rusqlite::params![runtime_id,timestamp])?;
                Ok(())
            })?;
            self.repo.resolve_finished_conflicts(timestamp)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_session_identified(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        let runner_name = p
            .get("runner_name")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        let native_id = p
            .get("native_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .trim();

        let title = p
            .get("title")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let raw_parent = p
            .get("parent_session_id")
            .or_else(|| p.get("fork_from"))
            .and_then(|v| v.as_str());

        let parent_session_id = if let Some(parent_str) = raw_parent {
            if let Some(parent_by_native) = self
                .repo
                .find_session_by_native_id(runner_name, parent_str)?
            {
                Some(parent_by_native.id)
            } else if let Some(parent_by_id) = self.repo.find_session_by_id(parent_str)? {
                Some(parent_by_id.id)
            } else {
                None // The native parent may not have been imported; preserve the claim in the event.
            }
        } else {
            None
        };

        let fork_reason = p
            .get("fork_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                if parent_session_id.is_some() {
                    Some("BRANCH".to_string())
                } else {
                    None
                }
            });

        let forked_at = if parent_session_id.is_some() {
            Some(timestamp.to_string())
        } else {
            None
        };

        // Check if session with native_id exists for this runner
        let (session_id, is_resume) = if native_id != "UNKNOWN" && !native_id.is_empty() {
            if let Some(existing) = self
                .repo
                .find_session_by_native_id(runner_name, native_id)?
            {
                (existing.id, true)
            } else {
                let new_id = event
                    .session_id
                    .clone()
                    .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));
                let s = Session {
                    id: new_id.clone(),
                    runner_name: runner_name.to_string(),
                    native_session_id: native_id.to_string(),
                    title: title.clone(),
                    started_at: timestamp.to_string(),
                    ended_at: None,
                    status: "ACTIVE".to_string(),
                    created_at: timestamp.to_string(),
                    parent_session_id,
                    fork_reason,
                    forked_at,
                };
                self.repo.save_session(&s)?;
                (new_id, false)
            }
        } else {
            let new_id = event
                .session_id
                .clone()
                .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));
            let s = Session {
                id: new_id.clone(),
                runner_name: runner_name.to_string(),
                native_session_id: "UNKNOWN".to_string(),
                title: title.clone(),
                started_at: timestamp.to_string(),
                ended_at: None,
                status: "ACTIVE".to_string(),
                created_at: timestamp.to_string(),
                parent_session_id,
                fork_reason,
                forked_at,
            };
            self.repo.save_session(&s)?;
            (new_id, false)
        };

        if let Some(alias) = &event.session_id {
            self.repo.save_session_alias(alias, &session_id)?;
        }

        // If runtime_id is provided, ensure runtime exists before binding
        if let Some(runtime_id) = &event.runtime_id {
            if self.repo.find_runtime_by_id(runtime_id)?.is_none() {
                let stub_runtime = RuntimeInstance {
                    id: runtime_id.clone(),
                    runner_name: runner_name.to_string(),
                    runner_version: "UNKNOWN".to_string(),
                    surface: "cli".to_string(),
                    pid: None,
                    hostname: "localhost".to_string(),
                    os: std::env::consts::OS.to_string(),
                    cwd: "".to_string(),
                    command_line: "".to_string(),
                    started_at: timestamp.to_string(),
                    ended_at: None,
                    exit_code: None,
                    status: "RUNNING".to_string(),
                };
                self.repo.save_runtime_instance(&stub_runtime)?;
            }

            let binding_id = format!("bind_{}_{}", runtime_id, session_id);
            let reason = if self
                .repo
                .find_runtime_by_id(runtime_id)?
                .is_some_and(|r| r.surface == "transcript")
            {
                "IMPORT"
            } else if is_resume {
                "RESUME"
            } else {
                "START"
            };
            let binding = RuntimeSessionBinding {
                id: binding_id,
                runtime_id: runtime_id.clone(),
                session_id: session_id.clone(),
                bound_at: timestamp.to_string(),
                unbound_at: None,
                reason: reason.to_string(),
            };
            self.repo.save_runtime_session_binding(&binding)?;
        }

        let mut canonical_event = event.clone();
        canonical_event.session_id = Some(session_id);
        self.record_raw_event(&canonical_event, timestamp)?;
        Ok(())
    }

    fn handle_execution_started(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        let execution_id = event
            .execution_id
            .clone()
            .unwrap_or_else(|| format!("exec_{}", Uuid::new_v4().simple()));
        if self.repo.find_execution_by_id(&execution_id)?.is_some() {
            return self.record_raw_event(event, timestamp);
        }
        let session_id = event
            .session_id
            .clone()
            .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));
        let runtime_id = event
            .runtime_id
            .clone()
            .unwrap_or_else(|| format!("run_{}", Uuid::new_v4().simple()));

        // Ensure runtime exists (FK constraint safeguard)
        if self.repo.find_runtime_by_id(&runtime_id)?.is_none() {
            let stub_runtime = RuntimeInstance {
                id: runtime_id.clone(),
                runner_name: "UNKNOWN".to_string(),
                runner_version: "UNKNOWN".to_string(),
                surface: "cli".to_string(),
                pid: None,
                hostname: "localhost".to_string(),
                os: std::env::consts::OS.to_string(),
                cwd: "".to_string(),
                command_line: "".to_string(),
                started_at: timestamp.to_string(),
                ended_at: None,
                exit_code: None,
                status: "RUNNING".to_string(),
            };
            self.repo.save_runtime_instance(&stub_runtime)?;
        }

        // Ensure session exists (FK constraint safeguard)
        if self.repo.find_session_by_id(&session_id)?.is_none() {
            let stub_session = Session {
                id: session_id.clone(),
                runner_name: "UNKNOWN".to_string(),
                native_session_id: "UNKNOWN".to_string(),
                title: None,
                started_at: timestamp.to_string(),
                ended_at: None,
                status: "ACTIVE".to_string(),
                created_at: timestamp.to_string(),
                parent_session_id: None,
                fork_reason: None,
                forked_at: None,
            };
            self.repo.save_session(&stub_session)?;
        }

        let native_execution_id = p
            .get("native_execution_id")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        let turn_index = p.get("turn_index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let prompt_summary = p
            .get("prompt_summary")
            .and_then(|v| v.as_str())
            .map(|s| redact_secrets(&s.chars().take(200).collect::<String>()));

        let model = p
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let reasoning_effort = p
            .get("reasoning_effort")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        let selected_agent_role = p
            .get("selected_agent_role")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        let repo_root = p
            .get("repo_root")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let worktree_path = p
            .get("worktree_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let branch = p
            .get("branch")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let head_sha = p
            .get("head_sha")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let live_observation = self
            .repo
            .find_runtime_by_id(&runtime_id)?
            .is_none_or(|r| r.surface != "transcript");

        // If another execution is active in this worktree, neither run can claim the Git changes alone.
        let active_worktree_executions =
            if let Some(wt) = worktree_path.as_deref().filter(|_| live_observation) {
                self.repo
                    .list_active_execution_refs_in_worktree(wt, &execution_id)?
            } else {
                Vec::new()
            };
        let git_attribution = if worktree_path.is_none() {
            "UNKNOWN".to_string()
        } else if active_worktree_executions.is_empty() {
            "OBSERVED".to_string()
        } else {
            "AMBIGUOUS".to_string()
        };

        let execution = Execution {
            id: execution_id.clone(),
            session_id: session_id.clone(),
            runtime_id: runtime_id.clone(),
            native_execution_id,
            turn_index,
            prompt_summary,
            model: model.clone(),
            reasoning_effort,
            selected_agent_role: selected_agent_role.clone(),
            started_at: timestamp.to_string(),
            ended_at: None,
            duration_ms: None,
            status: "RUNNING".to_string(),
            exit_code: None,
            error_message: None,
            repo_root,
            worktree_path,
            branch,
            head_sha,
            git_attribution,
            capture_scope: p
                .get("capture_scope")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string(),
        };

        self.repo.save_execution(&execution)?;

        for (active_execution_id, active_session_id) in &active_worktree_executions {
            self.repo
                .update_execution_git_attribution(active_execution_id, "AMBIGUOUS")?;

            if active_session_id != &session_id {
                let conflict_id = format!(
                    "conf_worktree_{}",
                    sha256_digest(&format!("{}:{}", execution_id, active_execution_id))
                );
                let conflict = SessionConflict {
                    id: conflict_id,
                    session_id: session_id.clone(),
                    conflicting_session_id: Some(active_session_id.clone()),
                    execution_id: Some(execution_id.clone()),
                    conflict_type: "WORKTREE_OVERLAP".to_string(),
                    severity: "WARNING".to_string(),
                    detected_at: timestamp.to_string(),
                    resolved_at: None,
                    details_json: Some(serde_json::json!({
                        "active_execution_id": active_execution_id,
                        "active_session_id": active_session_id,
                        "message": "Independent sessions have active executions in the same worktree; Git attribution is ambiguous"
                    }).to_string()),
                };
                self.repo.save_session_conflict(&conflict)?;
            }
        }

        // Session-level conflict detection:
        // 1. Concurrent Session Access (two separate runtimes actively running in the same session):
        if live_observation {
            if let Some(active_exec) = self
                .repo
                .find_active_execution_in_session(&session_id, &execution_id)?
            {
                if active_exec.runtime_id != runtime_id {
                    let conflict_id = format!("conf_sess_{}", Uuid::new_v4().simple());
                    let conflict = SessionConflict {
                    id: conflict_id,
                    session_id: session_id.clone(),
                    conflicting_session_id: None,
                    execution_id: Some(execution_id.clone()),
                    conflict_type: "CONCURRENT_SESSION_ACCESS".to_string(),
                    severity: "CRITICAL".to_string(),
                    detected_at: timestamp.to_string(),
                    resolved_at: None,
                    details_json: Some(serde_json::json!({
                        "runtime_a": active_exec.runtime_id,
                        "runtime_b": runtime_id,
                        "active_execution_id": active_exec.id,
                        "message": "Multiple separate agent runtimes are concurrently executing within the same logical session"
                    }).to_string()),
                };
                    self.repo.save_session_conflict(&conflict)?;
                }
            }

            // 2. Fork Divergence (any ancestor or descendant session running at the same time).
            for (active_execution_id, active_session_id) in self
                .repo
                .list_active_execution_refs_in_fork_lineage(&session_id, &execution_id)?
            {
                let conflict_id = format!(
                    "conf_fork_{}",
                    sha256_digest(&format!("{}:{}", execution_id, active_execution_id))
                );
                let conflict = SessionConflict {
                id: conflict_id,
                session_id: session_id.clone(),
                conflicting_session_id: Some(active_session_id.clone()),
                execution_id: Some(execution_id.clone()),
                conflict_type: "FORK_DIVERGENCE".to_string(),
                severity: "WARNING".to_string(),
                detected_at: timestamp.to_string(),
                resolved_at: None,
                details_json: Some(serde_json::json!({
                    "active_session_id": active_session_id,
                    "active_execution_id": active_execution_id,
                    "message": "Related sessions in the same fork lineage are running concurrent turns"
                }).to_string()),
            };
                self.repo.save_session_conflict(&conflict)?;
            }
        }

        // Also create a default main agent instance if role is known
        let main_agent_id = format!("agent_main_{}", execution_id);
        let main_agent = AgentInstance {
            id: main_agent_id,
            execution_id: execution_id.clone(),
            parent_agent_id: None,
            agent_role: if selected_agent_role != "UNKNOWN" {
                selected_agent_role
            } else {
                "main".to_string()
            },
            agent_name: "main".to_string(),
            model,
            started_at: timestamp.to_string(),
            ended_at: None,
            status: "RUNNING".to_string(),
        };
        self.repo.save_agent_instance(&main_agent)?;

        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_execution_completed(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        if let Some(execution_id) = &event.execution_id {
            let p = &event.payload;
            let existing = self
                .repo
                .find_execution_by_id(execution_id)?
                .ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName(
                        "Execution must be started before completion".into(),
                    )
                })?;
            if existing.status != "RUNNING" {
                return self.record_raw_event(event, timestamp);
            }
            let duration_ms = p
                .get("duration_ms")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| {
                    let start = chrono::DateTime::parse_from_rfc3339(&existing.started_at).ok();
                    let end = chrono::DateTime::parse_from_rfc3339(timestamp).ok();
                    start
                        .zip(end)
                        .map(|(a, b)| (b - a).num_milliseconds())
                        .unwrap_or(0)
                })
                .max(0);
            let exit_code = p
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let status = p
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or(match exit_code {
                    Some(0) => "COMPLETED",
                    Some(_) => "FAILED",
                    None => "UNKNOWN",
                });
            let error_message = p
                .get("error_message")
                .and_then(|v| v.as_str())
                .map(redact_secrets);

            self.repo.update_execution_completed(
                execution_id,
                timestamp,
                duration_ms,
                status,
                exit_code,
                error_message.as_deref(),
            )?;
            self.repo.db.with_conn(|conn| {
                conn.execute("UPDATE agent_instances SET ended_at = ?2, status = ?3 WHERE execution_id = ?1 AND status = 'RUNNING'", rusqlite::params![execution_id, timestamp, status])?;
                Ok(())
            })?;
            self.repo.resolve_finished_conflicts(timestamp)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_execution_context(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let id = event.execution_id.as_deref().ok_or_else(|| {
            rusqlite::Error::InvalidParameterName("execution.context requires execution_id".into())
        })?;
        let p = &event.payload;
        self.repo.db.with_conn(|conn| {
            let updated = conn.execute("UPDATE executions SET model=COALESCE(?2,model), reasoning_effort=COALESCE(?3,reasoning_effort), prompt_summary=COALESCE(?4,prompt_summary) WHERE id=?1", rusqlite::params![id,p["model"].as_str(),p["reasoning_effort"].as_str(),p["prompt_summary"].as_str()])?;
            if updated == 0 { return Err(rusqlite::Error::QueryReturnedNoRows); }
            conn.execute("UPDATE agent_instances SET model=COALESCE(?2,model) WHERE execution_id=?1 AND agent_role='main'",rusqlite::params![id,p["model"].as_str()])?;
            Ok(())
        })?;
        self.record_raw_event(event, timestamp)
    }

    fn handle_agent_started(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        if let Some(execution_id) = &event.execution_id {
            let agent_id = event
                .agent_instance_id
                .clone()
                .unwrap_or_else(|| format!("agent_{}", Uuid::new_v4().simple()));
            let parent_agent_id = p
                .get("parent_agent_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let agent_role = p
                .get("agent_role")
                .and_then(|v| v.as_str())
                .unwrap_or("subagent")
                .to_string();
            let agent_name = p
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("subagent")
                .to_string();
            let model = p
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();

            let agent = AgentInstance {
                id: agent_id,
                execution_id: execution_id.clone(),
                parent_agent_id,
                agent_role,
                agent_name,
                model,
                started_at: timestamp.to_string(),
                ended_at: None,
                status: "RUNNING".to_string(),
            };
            self.repo.save_agent_instance(&agent)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_component_event(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        if let Some(execution_id) = &event.execution_id {
            let comp_type = p
                .get("component_type")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    if event.event_type.starts_with("mcp") {
                        Some("MCP")
                    } else if event.event_type.starts_with("skill") {
                        Some("SKILL")
                    } else if event.event_type.starts_with("plugin") {
                        Some("PLUGIN")
                    } else {
                        Some("TOOL")
                    }
                })
                .unwrap_or("TOOL");

            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let version = p
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let description = p
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let comp_id = format!(
                "comp_{}_{}_{}",
                comp_type.to_lowercase(),
                name,
                version.as_deref().unwrap_or("")
            );
            let comp = Component {
                id: comp_id.clone(),
                component_type: comp_type.to_string(),
                name: name.to_string(),
                version: version.clone(),
                description,
            };

            let resolved_id = self.repo.save_component(&comp)?;

            let state = p.get("state").and_then(|v| v.as_str()).unwrap_or(
                if event.event_type.ends_with(".invoked") {
                    "INVOKED"
                } else {
                    "DISCOVERED"
                },
            );

            let invocations =
                p.get("invocations_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(if state == "INVOKED" { 1 } else { 0 }) as i32;

            let details_json = p
                .get("details")
                .map(|v| redact_secrets(&serde_json::to_string(v).unwrap_or_default()));

            let exec_comp = ExecutionComponent {
                execution_id: execution_id.clone(),
                component_id: resolved_id,
                state: state.to_string(),
                invocations_count: invocations,
                details_json,
                component_type: None,
                component_name: None,
                component_version: None,
            };

            self.repo.save_execution_component(&exec_comp)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_git_snapshot(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        if let Some(execution_id) = &event.execution_id {
            let snapshot_type = p
                .get("snapshot_type")
                .and_then(|v| v.as_str())
                .unwrap_or("BEFORE");
            let repo_root = p
                .get("repo_root")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN");
            let worktree_path = p
                .get("worktree_path")
                .and_then(|v| v.as_str())
                .unwrap_or(repo_root);
            let branch = p
                .get("branch")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN");
            let head_commit = p
                .get("head_commit")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN");
            let is_dirty = p.get("is_dirty").and_then(|v| v.as_bool()).unwrap_or(false);
            let diff_stat = p
                .get("diff_stat")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let execution = self.repo.find_execution_by_id(execution_id)?;
            let attribution = execution
                .as_ref()
                .map(|e| e.git_attribution.as_str())
                .unwrap_or("UNKNOWN");

            let changed_files_val = p.get("changed_files");
            let changed_files_count = changed_files_val
                .and_then(|v| v.as_array())
                .map(|a| a.len() as i32)
                .unwrap_or(0);
            let changed_files_json =
                changed_files_val.map(|v| serde_json::to_string(v).unwrap_or_default());

            let snapshot_id = format!("git_{}_{}", execution_id, snapshot_type.to_lowercase());
            let snapshot = GitSnapshot {
                id: snapshot_id,
                execution_id: execution_id.clone(),
                snapshot_type: snapshot_type.to_string(),
                captured_at: timestamp.to_string(),
                repo_root: repo_root.to_string(),
                worktree_path: worktree_path.to_string(),
                branch: branch.to_string(),
                head_commit: head_commit.to_string(),
                is_dirty,
                changed_files_count,
                diff_stat,
                changed_files_json,
                attribution: attribution.to_string(),
            };

            self.repo.save_git_snapshot(&snapshot)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_config_snapshot(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        let config_type = p
            .get("config_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let file_path = p
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let raw_content = p.get("raw_content").and_then(|v| v.as_str()).unwrap_or("");
        let redacted_raw = redact_secrets(raw_content);
        let sha256 = sha256_digest(&redacted_raw);

        let parsed_json = p
            .get("parsed_json")
            .map(|v| redact_secrets(&serde_json::to_string(v).unwrap_or_default()));

        let snapshot = ConfigSnapshot {
            id: format!("cfg_{}", &sha256[..16]),
            content_sha256: sha256,
            config_type: config_type.to_string(),
            file_path,
            raw_content_redacted: redacted_raw,
            parsed_json,
            captured_at: timestamp.to_string(),
        };

        self.repo.save_config_snapshot(&snapshot)?;
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }
}
