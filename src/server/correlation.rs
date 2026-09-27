use chrono::Utc;
use rusqlite::Result;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::events::IngestEvent;
use crate::domain::models::*;
use crate::redact::{redact_secrets, sha256_digest};
use crate::storage::Repository;

pub struct CorrelationEngine {
    repo: Arc<Repository>,
}

impl CorrelationEngine {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    pub fn process_event(&self, event: &IngestEvent) -> Result<String> {
        let timestamp = if event.timestamp.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            event.timestamp.clone()
        };

        match event.event_type.as_str() {
            "runtime.started" => self.handle_runtime_started(event, &timestamp)?,
            "runtime.stopped" => self.handle_runtime_stopped(event, &timestamp)?,
            "session.identified" | "session.started" => self.handle_session_identified(event, &timestamp)?,
            "execution.started" => self.handle_execution_started(event, &timestamp)?,
            "execution.completed" => self.handle_execution_completed(event, &timestamp)?,
            "agent.started" | "subagent.started" => self.handle_agent_started(event, &timestamp)?,
            "component.discovered" | "component.invoked" | "mcp.discovered" | "mcp.invoked" | "skill.discovered" | "plugin.discovered" => {
                self.handle_component_event(event, &timestamp)?
            }
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
        let payload_str = serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string());
        let redacted_payload = redact_secrets(&payload_str);

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
        let runtime_id = event.runtime_id.clone().unwrap_or_else(|| format!("run_{}", Uuid::new_v4().simple()));
        let p = &event.payload;

        let runner_name = p.get("runner_name").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
        let runner_version = p.get("runner_version").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
        let surface = p.get("surface").and_then(|v| v.as_str()).unwrap_or("cli").to_string();
        let pid = p.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
        let hostname = p.get("hostname").and_then(|v| v.as_str()).unwrap_or("localhost").to_string();
        let os = p.get("os").and_then(|v| v.as_str()).unwrap_or(std::env::consts::OS).to_string();
        let cwd = p.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let raw_cmd = p.get("command_line").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let command_line = redact_secrets(&raw_cmd);

        let runtime = RuntimeInstance {
            id: runtime_id.clone(),
            runner_name,
            runner_version,
            surface,
            pid,
            hostname,
            os,
            cwd,
            command_line,
            started_at: timestamp.to_string(),
            ended_at: None,
            exit_code: None,
            status: "RUNNING".to_string(),
        };

        self.repo.save_runtime_instance(&runtime)?;
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_runtime_stopped(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        if let Some(runtime_id) = &event.runtime_id {
            let p = &event.payload;
            let exit_code = p.get("exit_code").and_then(|v| v.as_i64()).map(|v| v as i32);
            let status = if let Some(code) = exit_code {
                if code == 0 { "COMPLETED" } else { "FAILED" }
            } else {
                p.get("status").and_then(|v| v.as_str()).unwrap_or("COMPLETED")
            };

            self.repo.update_runtime_stopped(runtime_id, timestamp, exit_code, status)?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_session_identified(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        let runner_name = p.get("runner_name").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
        let native_id = p.get("native_session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .trim();

        let title = p.get("title").and_then(|v| v.as_str()).map(|v| v.to_string());

        let raw_parent = p.get("parent_session_id")
            .or_else(|| p.get("fork_from"))
            .and_then(|v| v.as_str());

        let parent_session_id = if let Some(parent_str) = raw_parent {
            if let Ok(Some(parent_by_native)) = self.repo.find_session_by_native_id(runner_name, parent_str) {
                Some(parent_by_native.id)
            } else if let Ok(Some(parent_by_id)) = self.repo.find_session_by_id(parent_str) {
                Some(parent_by_id.id)
            } else {
                Some(parent_str.to_string())
            }
        } else {
            None
        };

        let fork_reason = p.get("fork_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| if parent_session_id.is_some() { Some("BRANCH".to_string()) } else { None });

        let forked_at = if parent_session_id.is_some() { Some(timestamp.to_string()) } else { None };

        // Check if session with native_id exists for this runner
        let (session_id, is_resume) = if native_id != "UNKNOWN" && !native_id.is_empty() {
            if let Some(existing) = self.repo.find_session_by_native_id(runner_name, native_id)? {
                (existing.id, true)
            } else {
                let new_id = event.session_id.clone().unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));
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
            let new_id = event.session_id.clone().unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));
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
            let reason = if is_resume { "RESUME" } else { "START" };
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

        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_execution_started(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        let execution_id = event.execution_id.clone().unwrap_or_else(|| format!("exec_{}", Uuid::new_v4().simple()));
        let session_id = event.session_id.clone().unwrap_or_else(|| "sess_unknown".to_string());
        let runtime_id = event.runtime_id.clone().unwrap_or_else(|| "run_unknown".to_string());

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

        let native_execution_id = p.get("native_execution_id")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        let turn_index = p.get("turn_index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let prompt_summary = p.get("prompt_summary")
            .and_then(|v| v.as_str())
            .map(|s| redact_secrets(&s.chars().take(200).collect::<String>()));

        let model = p.get("model").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
        let reasoning_effort = p.get("reasoning_effort").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
        let selected_agent_role = p.get("selected_agent_role").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();

        let repo_root = p.get("repo_root").and_then(|v| v.as_str()).map(|s| s.to_string());
        let worktree_path = p.get("worktree_path").and_then(|v| v.as_str()).map(|s| s.to_string());
        let branch = p.get("branch").and_then(|v| v.as_str()).map(|s| s.to_string());
        let head_sha = p.get("head_sha").and_then(|v| v.as_str()).map(|s| s.to_string());

        // Git attribution check:
        // If another execution is currently RUNNING in the same worktree -> AMBIGUOUS!
        let git_attribution = if let Some(wt) = &worktree_path {
            let active_count = self.repo.count_active_executions_in_worktree(wt, &execution_id)?;
            if active_count > 0 {
                // If there's already an active execution in this worktree, both are ambiguous!
                "AMBIGUOUS".to_string()
            } else {
                "OBSERVED".to_string()
            }
        } else {
            "UNKNOWN".to_string()
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
        };

        self.repo.save_execution(&execution)?;

        // Session-level conflict detection:
        // 1. Concurrent Session Access (two separate runtimes actively running in the same session):
        if let Ok(Some(active_exec)) = self.repo.find_active_execution_in_session(&session_id, &execution_id) {
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
                let _ = self.repo.save_session_conflict(&conflict);
            }
        }

        // 2. Fork Divergence (parent session and child fork running concurrently):
        if let Ok(Some(current_session)) = self.repo.find_session_by_id(&session_id) {
            if let Some(parent_id) = &current_session.parent_session_id {
                if let Ok(Some(parent_exec)) = self.repo.find_active_execution_in_session(parent_id, &execution_id) {
                    let conflict_id = format!("conf_fork_{}", Uuid::new_v4().simple());
                    let conflict = SessionConflict {
                        id: conflict_id,
                        session_id: session_id.clone(),
                        conflicting_session_id: Some(parent_id.clone()),
                        execution_id: Some(execution_id.clone()),
                        conflict_type: "FORK_DIVERGENCE".to_string(),
                        severity: "WARNING".to_string(),
                        detected_at: timestamp.to_string(),
                        resolved_at: None,
                        details_json: Some(serde_json::json!({
                            "parent_session_id": parent_id,
                            "forked_session_id": session_id,
                            "parent_execution_id": parent_exec.id,
                            "message": "Parent session and forked session are running concurrent turns simultaneously"
                        }).to_string()),
                    };
                    let _ = self.repo.save_session_conflict(&conflict);
                }
            }
        }

        // Also create a default main agent instance if role is known
        let main_agent_id = format!("agent_{}", Uuid::new_v4().simple());
        let main_agent = AgentInstance {
            id: main_agent_id,
            execution_id: execution_id.clone(),
            parent_agent_id: None,
            agent_role: if selected_agent_role != "UNKNOWN" { selected_agent_role } else { "main".to_string() },
            agent_name: "main".to_string(),
            model,
            started_at: timestamp.to_string(),
            ended_at: None,
            status: "RUNNING".to_string(),
        };
        let _ = self.repo.save_agent_instance(&main_agent);

        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_execution_completed(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        if let Some(execution_id) = &event.execution_id {
            let p = &event.payload;
            let duration_ms = p.get("duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            let exit_code = p.get("exit_code").and_then(|v| v.as_i64()).map(|v| v as i32);
            let status = p.get("status").and_then(|v| v.as_str()).unwrap_or(
                if exit_code.unwrap_or(0) == 0 { "COMPLETED" } else { "FAILED" }
            );
            let error_message = p.get("error_message").and_then(|v| v.as_str()).map(|s| redact_secrets(s));

            self.repo.update_execution_completed(
                execution_id,
                timestamp,
                duration_ms,
                status,
                exit_code,
                error_message.as_deref(),
            )?;
        }
        self.record_raw_event(event, timestamp)?;
        Ok(())
    }

    fn handle_agent_started(&self, event: &IngestEvent, timestamp: &str) -> Result<()> {
        let p = &event.payload;
        if let Some(execution_id) = &event.execution_id {
            let agent_id = event.agent_instance_id.clone().unwrap_or_else(|| format!("agent_{}", Uuid::new_v4().simple()));
            let parent_agent_id = p.get("parent_agent_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let agent_role = p.get("agent_role").and_then(|v| v.as_str()).unwrap_or("subagent").to_string();
            let agent_name = p.get("agent_name").and_then(|v| v.as_str()).unwrap_or("subagent").to_string();
            let model = p.get("model").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();

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
            let comp_type = p.get("component_type")
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
            let version = p.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
            let description = p.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());

            let comp_id = format!("comp_{}_{}_{}", comp_type.to_lowercase(), name, version.as_deref().unwrap_or(""));
            let comp = Component {
                id: comp_id.clone(),
                component_type: comp_type.to_string(),
                name: name.to_string(),
                version: version.clone(),
                description,
            };

            let resolved_id = self.repo.save_component(&comp)?;

            let state = p.get("state")
                .and_then(|v| v.as_str())
                .unwrap_or(if event.event_type.ends_with(".invoked") { "INVOKED" } else { "DISCOVERED" });

            let invocations = p.get("invocations_count").and_then(|v| v.as_i64()).unwrap_or(
                if state == "INVOKED" { 1 } else { 0 }
            ) as i32;

            let details_json = p.get("details").map(|v| redact_secrets(&serde_json::to_string(v).unwrap_or_default()));

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
            let snapshot_type = p.get("snapshot_type").and_then(|v| v.as_str()).unwrap_or("BEFORE");
            let repo_root = p.get("repo_root").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
            let worktree_path = p.get("worktree_path").and_then(|v| v.as_str()).unwrap_or(repo_root);
            let branch = p.get("branch").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
            let head_commit = p.get("head_commit").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
            let is_dirty = p.get("is_dirty").and_then(|v| v.as_bool()).unwrap_or(false);
            let diff_stat = p.get("diff_stat").and_then(|v| v.as_str()).map(|s| s.to_string());
            let attribution = p.get("attribution").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");

            let changed_files_val = p.get("changed_files");
            let changed_files_count = changed_files_val
                .and_then(|v| v.as_array())
                .map(|a| a.len() as i32)
                .unwrap_or(0);
            let changed_files_json = changed_files_val.map(|v| serde_json::to_string(v).unwrap_or_default());

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
        let config_type = p.get("config_type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let file_path = p.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string());
        let raw_content = p.get("raw_content").and_then(|v| v.as_str()).unwrap_or("");
        let redacted_raw = redact_secrets(raw_content);
        let sha256 = sha256_digest(&redacted_raw);

        let parsed_json = p.get("parsed_json").map(|v| redact_secrets(&serde_json::to_string(v).unwrap_or_default()));

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
