use rusqlite::{params, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

use crate::domain::models::*;
use crate::storage::db::Database;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutionFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub period: Option<String>,
    pub runner: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub branch: Option<String>,
    pub session: Option<String>,
    pub component: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionFilter {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub runner: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWithStats {
    pub id: String,
    pub runner_name: String,
    pub native_session_id: String,
    pub title: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub created_at: String,
    pub execution_count: i64,
    pub runtime_count: i64,
    pub resume_count: i64,
    pub last_active_at: String,
    pub parent_session_id: Option<String>,
    pub fork_reason: Option<String>,
    pub forked_at: Option<String>,
    pub conflicts_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCount {
    pub key: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStat {
    pub name: String,
    pub configured: i64,
    pub invoked: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSummary {
    pub total_executions: i64,
    pub total_sessions: i64,
    pub total_runtimes: i64,
    pub total_conflicts: i64,
    pub by_runner: Vec<KeyCount>,
    pub by_model: Vec<KeyCount>,
    pub by_reasoning: Vec<KeyCount>,
    pub by_agent: Vec<KeyCount>,
    pub by_status: Vec<KeyCount>,
    pub mcp_stats: Vec<McpStat>,
    pub skill_stats: Vec<KeyCount>,
    pub plugin_stats: Vec<KeyCount>,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    pub status: String,
    pub version: String,
    pub db_connected: bool,
    pub active_executions: i64,
    pub active_runtimes: i64,
    pub total_executions: i64,
    pub total_sessions: i64,
}

pub struct Repository {
    db: Database,
}

impl Repository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn save_runtime_instance(&self, r: &RuntimeInstance) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO runtime_instances (
                    id, runner_name, runner_version, surface, pid, hostname, os, cwd,
                    command_line, started_at, ended_at, exit_code, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(id) DO UPDATE SET
                    runner_version = excluded.runner_version,
                    surface = excluded.surface,
                    ended_at = excluded.ended_at,
                    exit_code = excluded.exit_code,
                    status = excluded.status
                "#,
                params![
                    r.id, r.runner_name, r.runner_version, r.surface, r.pid, r.hostname,
                    r.os, r.cwd, r.command_line, r.started_at, r.ended_at, r.exit_code, r.status
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_runtime_stopped(&self, runtime_id: &str, ended_at: &str, exit_code: Option<i32>, status: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE runtime_instances
                SET ended_at = ?2, exit_code = ?3, status = ?4
                WHERE id = ?1
                "#,
                params![runtime_id, ended_at, exit_code, status],
            )?;
            Ok(())
        })
    }

    pub fn find_runtime_by_id(&self, id: &str) -> Result<Option<RuntimeInstance>> {
        self.db.with_conn(|conn| {
            let res = conn.query_row(
                r#"
                SELECT id, runner_name, runner_version, surface, pid, hostname, os, cwd,
                       command_line, started_at, ended_at, exit_code, status
                FROM runtime_instances WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(RuntimeInstance {
                        id: row.get(0)?,
                        runner_name: row.get(1)?,
                        runner_version: row.get(2)?,
                        surface: row.get(3)?,
                        pid: row.get(4)?,
                        hostname: row.get(5)?,
                        os: row.get(6)?,
                        cwd: row.get(7)?,
                        command_line: row.get(8)?,
                        started_at: row.get(9)?,
                        ended_at: row.get(10)?,
                        exit_code: row.get(11)?,
                        status: row.get(12)?,
                    })
                },
            ).optional()?;
            Ok(res)
        })
    }

    pub fn find_session_by_native_id(&self, runner_name: &str, native_id: &str) -> Result<Option<Session>> {
        if native_id == "UNKNOWN" || native_id.trim().is_empty() {
            return Ok(None);
        }
        self.db.with_conn(|conn| {
            let res = conn.query_row(
                r#"
                SELECT id, runner_name, native_session_id, title, started_at, ended_at, status, created_at,
                       parent_session_id, fork_reason, forked_at
                FROM sessions
                WHERE runner_name = ?1 AND native_session_id = ?2
                ORDER BY created_at DESC LIMIT 1
                "#,
                params![runner_name, native_id],
                |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        runner_name: row.get(1)?,
                        native_session_id: row.get(2)?,
                        title: row.get(3)?,
                        started_at: row.get(4)?,
                        ended_at: row.get(5)?,
                        status: row.get(6)?,
                        created_at: row.get(7)?,
                        parent_session_id: row.get(8)?,
                        fork_reason: row.get(9)?,
                        forked_at: row.get(10)?,
                    })
                },
            ).optional()?;
            Ok(res)
        })
    }

    pub fn find_session_by_id(&self, id: &str) -> Result<Option<Session>> {
        self.db.with_conn(|conn| {
            let res = conn.query_row(
                r#"
                SELECT id, runner_name, native_session_id, title, started_at, ended_at, status, created_at,
                       parent_session_id, fork_reason, forked_at
                FROM sessions WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(Session {
                        id: row.get(0)?,
                        runner_name: row.get(1)?,
                        native_session_id: row.get(2)?,
                        title: row.get(3)?,
                        started_at: row.get(4)?,
                        ended_at: row.get(5)?,
                        status: row.get(6)?,
                        created_at: row.get(7)?,
                        parent_session_id: row.get(8)?,
                        fork_reason: row.get(9)?,
                        forked_at: row.get(10)?,
                    })
                },
            ).optional()?;
            Ok(res)
        })
    }

    pub fn save_session(&self, s: &Session) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO sessions (
                    id, runner_name, native_session_id, title, started_at, ended_at,
                    status, created_at, parent_session_id, fork_reason, forked_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ON CONFLICT(id) DO UPDATE SET
                    title = COALESCE(excluded.title, sessions.title),
                    ended_at = excluded.ended_at,
                    status = excluded.status,
                    parent_session_id = COALESCE(excluded.parent_session_id, sessions.parent_session_id),
                    fork_reason = COALESCE(excluded.fork_reason, sessions.fork_reason),
                    forked_at = COALESCE(excluded.forked_at, sessions.forked_at)
                "#,
                params![
                    s.id, s.runner_name, s.native_session_id, s.title, s.started_at, s.ended_at,
                    s.status, s.created_at, s.parent_session_id, s.fork_reason, s.forked_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_session_status(&self, id: &str, status: &str, ended_at: Option<&str>) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE sessions
                SET status = ?2, ended_at = COALESCE(?3, ended_at)
                WHERE id = ?1
                "#,
                params![id, status, ended_at],
            )?;
            Ok(())
        })
    }

    pub fn save_runtime_session_binding(&self, b: &RuntimeSessionBinding) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO runtime_session_bindings (id, runtime_id, session_id, bound_at, unbound_at, reason)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    unbound_at = excluded.unbound_at,
                    reason = excluded.reason
                "#,
                params![b.id, b.runtime_id, b.session_id, b.bound_at, b.unbound_at, b.reason],
            )?;
            Ok(())
        })
    }

    pub fn list_bindings_for_session(&self, session_id: &str) -> Result<Vec<RuntimeSessionBinding>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, runtime_id, session_id, bound_at, unbound_at, reason
                FROM runtime_session_bindings
                WHERE session_id = ?1
                ORDER BY bound_at ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(RuntimeSessionBinding {
                    id: row.get(0)?,
                    runtime_id: row.get(1)?,
                    session_id: row.get(2)?,
                    bound_at: row.get(3)?,
                    unbound_at: row.get(4)?,
                    reason: row.get(5)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn save_session_conflict(&self, sc: &SessionConflict) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO session_conflicts (
                    id, session_id, conflicting_session_id, execution_id, conflict_type,
                    severity, detected_at, resolved_at, details_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(id) DO UPDATE SET
                    resolved_at = excluded.resolved_at,
                    details_json = excluded.details_json
                "#,
                params![
                    sc.id, sc.session_id, sc.conflicting_session_id, sc.execution_id,
                    sc.conflict_type, sc.severity, sc.detected_at, sc.resolved_at, sc.details_json
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_conflicts_for_session(&self, session_id: &str) -> Result<Vec<SessionConflict>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, session_id, conflicting_session_id, execution_id, conflict_type,
                       severity, detected_at, resolved_at, details_json
                FROM session_conflicts
                WHERE session_id = ?1 OR conflicting_session_id = ?1
                ORDER BY detected_at DESC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(SessionConflict {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    conflicting_session_id: row.get(2)?,
                    execution_id: row.get(3)?,
                    conflict_type: row.get(4)?,
                    severity: row.get(5)?,
                    detected_at: row.get(6)?,
                    resolved_at: row.get(7)?,
                    details_json: row.get(8)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn list_all_conflicts(&self) -> Result<Vec<SessionConflict>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, session_id, conflicting_session_id, execution_id, conflict_type,
                       severity, detected_at, resolved_at, details_json
                FROM session_conflicts
                ORDER BY detected_at DESC
                LIMIT 100
                "#,
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(SessionConflict {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    conflicting_session_id: row.get(2)?,
                    execution_id: row.get(3)?,
                    conflict_type: row.get(4)?,
                    severity: row.get(5)?,
                    detected_at: row.get(6)?,
                    resolved_at: row.get(7)?,
                    details_json: row.get(8)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn list_child_forks(&self, parent_session_id: &str) -> Result<Vec<Session>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, runner_name, native_session_id, title, started_at, ended_at, status, created_at,
                       parent_session_id, fork_reason, forked_at
                FROM sessions
                WHERE parent_session_id = ?1
                ORDER BY created_at ASC
                "#,
            )?;
            let rows = stmt.query_map(params![parent_session_id], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    runner_name: row.get(1)?,
                    native_session_id: row.get(2)?,
                    title: row.get(3)?,
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    parent_session_id: row.get(8)?,
                    fork_reason: row.get(9)?,
                    forked_at: row.get(10)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn find_active_execution_in_session(&self, session_id: &str, exclude_exec_id: &str) -> Result<Option<Execution>> {
        self.db.with_conn(|conn| {
            let res = conn.query_row(
                r#"
                SELECT id, session_id, runtime_id, native_execution_id, turn_index,
                       prompt_summary, model, reasoning_effort, selected_agent_role,
                       started_at, ended_at, duration_ms, status, exit_code, error_message,
                       repo_root, worktree_path, branch, head_sha, git_attribution
                FROM executions
                WHERE session_id = ?1 AND status = 'RUNNING' AND id != ?2
                LIMIT 1
                "#,
                params![session_id, exclude_exec_id],
                |row| {
                    Ok(Execution {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        runtime_id: row.get(2)?,
                        native_execution_id: row.get(3)?,
                        turn_index: row.get(4)?,
                        prompt_summary: row.get(5)?,
                        model: row.get(6)?,
                        reasoning_effort: row.get(7)?,
                        selected_agent_role: row.get(8)?,
                        started_at: row.get(9)?,
                        ended_at: row.get(10)?,
                        duration_ms: row.get(11)?,
                        status: row.get(12)?,
                        exit_code: row.get(13)?,
                        error_message: row.get(14)?,
                        repo_root: row.get(15)?,
                        worktree_path: row.get(16)?,
                        branch: row.get(17)?,
                        head_sha: row.get(18)?,
                        git_attribution: row.get(19)?,
                    })
                },
            ).optional()?;
            Ok(res)
        })
    }

    pub fn save_execution(&self, e: &Execution) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO executions (
                    id, session_id, runtime_id, native_execution_id, turn_index,
                    prompt_summary, model, reasoning_effort, selected_agent_role,
                    started_at, ended_at, duration_ms, status, exit_code, error_message,
                    repo_root, worktree_path, branch, head_sha, git_attribution
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
                ON CONFLICT(id) DO UPDATE SET
                    prompt_summary = COALESCE(excluded.prompt_summary, executions.prompt_summary),
                    model = CASE WHEN excluded.model != 'UNKNOWN' THEN excluded.model ELSE executions.model END,
                    reasoning_effort = CASE WHEN excluded.reasoning_effort != 'UNKNOWN' THEN excluded.reasoning_effort ELSE executions.reasoning_effort END,
                    selected_agent_role = CASE WHEN excluded.selected_agent_role != 'UNKNOWN' THEN excluded.selected_agent_role ELSE executions.selected_agent_role END,
                    ended_at = excluded.ended_at,
                    duration_ms = excluded.duration_ms,
                    status = excluded.status,
                    exit_code = excluded.exit_code,
                    error_message = excluded.error_message,
                    repo_root = COALESCE(excluded.repo_root, executions.repo_root),
                    worktree_path = COALESCE(excluded.worktree_path, executions.worktree_path),
                    branch = COALESCE(excluded.branch, executions.branch),
                    head_sha = COALESCE(excluded.head_sha, executions.head_sha),
                    git_attribution = excluded.git_attribution
                "#,
                params![
                    e.id, e.session_id, e.runtime_id, e.native_execution_id, e.turn_index,
                    e.prompt_summary, e.model, e.reasoning_effort, e.selected_agent_role,
                    e.started_at, e.ended_at, e.duration_ms, e.status, e.exit_code, e.error_message,
                    e.repo_root, e.worktree_path, e.branch, e.head_sha, e.git_attribution
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_execution_completed(
        &self,
        id: &str,
        ended_at: &str,
        duration_ms: i64,
        status: &str,
        exit_code: Option<i32>,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE executions
                SET ended_at = ?2, duration_ms = ?3, status = ?4, exit_code = ?5, error_message = ?6
                WHERE id = ?1
                "#,
                params![id, ended_at, duration_ms, status, exit_code, error_message],
            )?;
            Ok(())
        })
    }

    pub fn find_execution_by_id(&self, id: &str) -> Result<Option<Execution>> {
        self.db.with_conn(|conn| {
            let res = conn.query_row(
                r#"
                SELECT id, session_id, runtime_id, native_execution_id, turn_index,
                       prompt_summary, model, reasoning_effort, selected_agent_role,
                       started_at, ended_at, duration_ms, status, exit_code, error_message,
                       repo_root, worktree_path, branch, head_sha, git_attribution
                FROM executions WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(Execution {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        runtime_id: row.get(2)?,
                        native_execution_id: row.get(3)?,
                        turn_index: row.get(4)?,
                        prompt_summary: row.get(5)?,
                        model: row.get(6)?,
                        reasoning_effort: row.get(7)?,
                        selected_agent_role: row.get(8)?,
                        started_at: row.get(9)?,
                        ended_at: row.get(10)?,
                        duration_ms: row.get(11)?,
                        status: row.get(12)?,
                        exit_code: row.get(13)?,
                        error_message: row.get(14)?,
                        repo_root: row.get(15)?,
                        worktree_path: row.get(16)?,
                        branch: row.get(17)?,
                        head_sha: row.get(18)?,
                        git_attribution: row.get(19)?,
                    })
                },
            ).optional()?;
            Ok(res)
        })
    }

    pub fn count_active_executions_in_worktree(&self, worktree_path: &str, exclude_exec_id: &str) -> Result<usize> {
        self.db.with_conn(|conn| {
            let count: i64 = conn.query_row(
                r#"
                SELECT COUNT(*) FROM executions
                WHERE worktree_path = ?1 AND status = 'RUNNING' AND id != ?2
                "#,
                params![worktree_path, exclude_exec_id],
                |row| row.get(0),
            )?;
            Ok(count as usize)
        })
    }

    pub fn update_execution_git_attribution(&self, id: &str, attribution: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "UPDATE executions SET git_attribution = ?2 WHERE id = ?1",
                params![id, attribution],
            )?;
            Ok(())
        })
    }

    pub fn save_agent_instance(&self, a: &AgentInstance) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO agent_instances (id, execution_id, parent_agent_id, agent_role, agent_name, model, started_at, ended_at, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(id) DO UPDATE SET
                    ended_at = excluded.ended_at,
                    status = excluded.status
                "#,
                params![a.id, a.execution_id, a.parent_agent_id, a.agent_role, a.agent_name, a.model, a.started_at, a.ended_at, a.status],
            )?;
            Ok(())
        })
    }

    pub fn list_agent_instances_for_execution(&self, execution_id: &str) -> Result<Vec<AgentInstance>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, execution_id, parent_agent_id, agent_role, agent_name, model, started_at, ended_at, status
                FROM agent_instances WHERE execution_id = ?1
                ORDER BY started_at ASC
                "#,
            )?;
            let rows = stmt.query_map(params![execution_id], |row| {
                Ok(AgentInstance {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    parent_agent_id: row.get(2)?,
                    agent_role: row.get(3)?,
                    agent_name: row.get(4)?,
                    model: row.get(5)?,
                    started_at: row.get(6)?,
                    ended_at: row.get(7)?,
                    status: row.get(8)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn save_component(&self, c: &Component) -> Result<String> {
        self.db.with_conn(|conn| {
            let version_str = c.version.as_deref().unwrap_or("UNKNOWN");
            let existing: Option<String> = conn.query_row(
                "SELECT id FROM components WHERE component_type = ?1 AND name = ?2 AND version = ?3",
                params![c.component_type, c.name, version_str],
                |row| row.get(0),
            ).optional()?;

            if let Some(id) = existing {
                Ok(id)
            } else {
                conn.execute(
                    r#"
                    INSERT INTO components (id, component_type, name, version, description)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![c.id, c.component_type, c.name, version_str, c.description],
                )?;
                Ok(c.id.clone())
            }
        })
    }

    pub fn save_execution_component(&self, ec: &ExecutionComponent) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO execution_components (execution_id, component_id, state, invocations_count, details_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(execution_id, component_id) DO UPDATE SET
                    state = CASE
                        WHEN excluded.state = 'INVOKED' THEN 'INVOKED'
                        WHEN excluded.state = 'LOADED' AND execution_components.state != 'INVOKED' THEN 'LOADED'
                        WHEN excluded.state = 'DISCOVERED' AND execution_components.state NOT IN ('INVOKED', 'LOADED') THEN 'DISCOVERED'
                        WHEN excluded.state = 'CONFIGURED' AND execution_components.state NOT IN ('INVOKED', 'LOADED', 'DISCOVERED') THEN 'CONFIGURED'
                        ELSE execution_components.state
                    END,
                    invocations_count = execution_components.invocations_count + excluded.invocations_count,
                    details_json = COALESCE(excluded.details_json, execution_components.details_json)
                "#,
                params![ec.execution_id, ec.component_id, ec.state, ec.invocations_count, ec.details_json],
            )?;
            Ok(())
        })
    }

    pub fn list_components_for_execution(&self, execution_id: &str) -> Result<Vec<ExecutionComponent>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT ec.execution_id, ec.component_id, ec.state, ec.invocations_count, ec.details_json,
                       c.component_type, c.name, c.version
                FROM execution_components ec
                JOIN components c ON ec.component_id = c.id
                WHERE ec.execution_id = ?1
                ORDER BY c.component_type ASC, c.name ASC
                "#,
            )?;
            let rows = stmt.query_map(params![execution_id], |row| {
                Ok(ExecutionComponent {
                    execution_id: row.get(0)?,
                    component_id: row.get(1)?,
                    state: row.get(2)?,
                    invocations_count: row.get(3)?,
                    details_json: row.get(4)?,
                    component_type: Some(row.get(5)?),
                    component_name: Some(row.get(6)?),
                    component_version: Some(row.get(7)?),
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn save_git_snapshot(&self, s: &GitSnapshot) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO git_snapshots (
                    id, execution_id, snapshot_type, captured_at, repo_root, worktree_path,
                    branch, head_commit, is_dirty, changed_files_count, diff_stat, changed_files_json, attribution
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(id) DO UPDATE SET
                    diff_stat = excluded.diff_stat,
                    changed_files_json = excluded.changed_files_json,
                    attribution = excluded.attribution
                "#,
                params![
                    s.id, s.execution_id, s.snapshot_type, s.captured_at, s.repo_root, s.worktree_path,
                    s.branch, s.head_commit, s.is_dirty as i32, s.changed_files_count, s.diff_stat,
                    s.changed_files_json, s.attribution
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_git_snapshots_for_execution(&self, execution_id: &str) -> Result<Vec<GitSnapshot>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, execution_id, snapshot_type, captured_at, repo_root, worktree_path,
                       branch, head_commit, is_dirty, changed_files_count, diff_stat, changed_files_json, attribution
                FROM git_snapshots WHERE execution_id = ?1
                ORDER BY captured_at ASC
                "#,
            )?;
            let rows = stmt.query_map(params![execution_id], |row| {
                let is_dirty_int: i32 = row.get(8)?;
                Ok(GitSnapshot {
                    id: row.get(0)?,
                    execution_id: row.get(1)?,
                    snapshot_type: row.get(2)?,
                    captured_at: row.get(3)?,
                    repo_root: row.get(4)?,
                    worktree_path: row.get(5)?,
                    branch: row.get(6)?,
                    head_commit: row.get(7)?,
                    is_dirty: is_dirty_int != 0,
                    changed_files_count: row.get(9)?,
                    diff_stat: row.get(10)?,
                    changed_files_json: row.get(11)?,
                    attribution: row.get(12)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn save_config_snapshot(&self, c: &ConfigSnapshot) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT OR IGNORE INTO config_snapshots (
                    id, content_sha256, config_type, file_path, raw_content_redacted, parsed_json, captured_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![c.id, c.content_sha256, c.config_type, c.file_path, c.raw_content_redacted, c.parsed_json, c.captured_at],
            )?;
            Ok(())
        })
    }

    pub fn save_event(&self, e: &Event) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO events (event_id, timestamp, runtime_id, session_id, execution_id, agent_instance_id, event_type, source, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    e.event_id, e.timestamp, e.runtime_id, e.session_id, e.execution_id,
                    e.agent_instance_id, e.event_type, e.source, e.payload_json
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_events_for_execution(&self, execution_id: &str) -> Result<Vec<Event>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, event_id, timestamp, runtime_id, session_id, execution_id, agent_instance_id, event_type, source, payload_json
                FROM events WHERE execution_id = ?1
                ORDER BY timestamp ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map(params![execution_id], |row| {
                Ok(Event {
                    id: Some(row.get(0)?),
                    event_id: row.get(1)?,
                    timestamp: row.get(2)?,
                    runtime_id: row.get(3)?,
                    session_id: row.get(4)?,
                    execution_id: row.get(5)?,
                    agent_instance_id: row.get(6)?,
                    event_type: row.get(7)?,
                    source: row.get(8)?,
                    payload_json: row.get(9)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn list_events_for_session(&self, session_id: &str) -> Result<Vec<Event>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, event_id, timestamp, runtime_id, session_id, execution_id, agent_instance_id, event_type, source, payload_json
                FROM events WHERE session_id = ?1
                ORDER BY timestamp ASC, id ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(Event {
                    id: Some(row.get(0)?),
                    event_id: row.get(1)?,
                    timestamp: row.get(2)?,
                    runtime_id: row.get(3)?,
                    session_id: row.get(4)?,
                    execution_id: row.get(5)?,
                    agent_instance_id: row.get(6)?,
                    event_type: row.get(7)?,
                    source: row.get(8)?,
                    payload_json: row.get(9)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn list_executions(&self, filter: &ExecutionFilter) -> Result<(Vec<Execution>, usize)> {
        self.db.with_conn(|conn| {
            let mut conditions: Vec<String> = Vec::new();
            let mut query_params: Vec<rusqlite::types::Value> = Vec::new();

            if let Some(period) = &filter.period {
                let modifier = match period.as_str() {
                    "1h" => Some("-1 hours"),
                    "24h" => Some("-24 hours"),
                    "7d" => Some("-7 days"),
                    "30d" => Some("-30 days"),
                    _ => None,
                };
                if let Some(m) = modifier {
                    conditions.push(format!("e.started_at >= datetime('now', '{}')", m));
                }
            }

            if let Some(runner) = &filter.runner {
                if !runner.is_empty() && runner != "ALL" {
                    conditions.push("r.runner_name = ?".to_string());
                    query_params.push(runner.clone().into());
                }
            }

            if let Some(model) = &filter.model {
                if !model.is_empty() && model != "ALL" {
                    conditions.push("e.model = ?".to_string());
                    query_params.push(model.clone().into());
                }
            }

            if let Some(agent) = &filter.agent {
                if !agent.is_empty() && agent != "ALL" {
                    conditions.push("e.selected_agent_role = ?".to_string());
                    query_params.push(agent.clone().into());
                }
            }

            if let Some(branch) = &filter.branch {
                if !branch.is_empty() && branch != "ALL" {
                    conditions.push("e.branch = ?".to_string());
                    query_params.push(branch.clone().into());
                }
            }

            if let Some(session) = &filter.session {
                if !session.is_empty() {
                    conditions.push("(e.session_id = ? OR s.native_session_id = ?)".to_string());
                    query_params.push(session.clone().into());
                    query_params.push(session.clone().into());
                }
            }

            if let Some(status) = &filter.status {
                if !status.is_empty() && status != "ALL" {
                    conditions.push("e.status = ?".to_string());
                    query_params.push(status.clone().into());
                }
            }

            if let Some(component) = &filter.component {
                if !component.is_empty() && component != "ALL" {
                    conditions.push(
                        "EXISTS (SELECT 1 FROM execution_components ec JOIN components c ON ec.component_id = c.id WHERE ec.execution_id = e.id AND c.name = ?)".to_string()
                    );
                    query_params.push(component.clone().into());
                }
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let count_sql = format!(
                r#"
                SELECT COUNT(*)
                FROM executions e
                JOIN runtime_instances r ON e.runtime_id = r.id
                JOIN sessions s ON e.session_id = s.id
                {}
                "#,
                where_clause
            );

            let mut count_stmt = conn.prepare(&count_sql)?;
            let total: i64 = count_stmt.query_row(
                rusqlite::params_from_iter(query_params.iter()),
                |row| row.get(0),
            )?;

            let page = filter.page.unwrap_or(1).max(1);
            let page_size = filter.page_size.unwrap_or(20).clamp(1, 100);
            let offset = (page - 1) * page_size;

            let query_sql = format!(
                r#"
                SELECT e.id, e.session_id, e.runtime_id, e.native_execution_id, e.turn_index,
                       e.prompt_summary, e.model, e.reasoning_effort, e.selected_agent_role,
                       e.started_at, e.ended_at, e.duration_ms, e.status, e.exit_code, e.error_message,
                       e.repo_root, e.worktree_path, e.branch, e.head_sha, e.git_attribution
                FROM executions e
                JOIN runtime_instances r ON e.runtime_id = r.id
                JOIN sessions s ON e.session_id = s.id
                {}
                ORDER BY e.started_at DESC
                LIMIT {} OFFSET {}
                "#,
                where_clause, page_size, offset
            );

            let mut stmt = conn.prepare(&query_sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(query_params.iter()),
                |row| {
                    Ok(Execution {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        runtime_id: row.get(2)?,
                        native_execution_id: row.get(3)?,
                        turn_index: row.get(4)?,
                        prompt_summary: row.get(5)?,
                        model: row.get(6)?,
                        reasoning_effort: row.get(7)?,
                        selected_agent_role: row.get(8)?,
                        started_at: row.get(9)?,
                        ended_at: row.get(10)?,
                        duration_ms: row.get(11)?,
                        status: row.get(12)?,
                        exit_code: row.get(13)?,
                        error_message: row.get(14)?,
                        repo_root: row.get(15)?,
                        worktree_path: row.get(16)?,
                        branch: row.get(17)?,
                        head_sha: row.get(18)?,
                        git_attribution: row.get(19)?,
                    })
                },
            )?;

            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }

            Ok((items, total as usize))
        })
    }

    pub fn list_sessions(&self, filter: &SessionFilter) -> Result<(Vec<SessionWithStats>, usize)> {
        self.db.with_conn(|conn| {
            let mut conditions: Vec<String> = Vec::new();
            let mut query_params: Vec<rusqlite::types::Value> = Vec::new();

            if let Some(runner) = &filter.runner {
                if !runner.is_empty() && runner != "ALL" {
                    conditions.push("s.runner_name = ?".to_string());
                    query_params.push(runner.clone().into());
                }
            }

            if let Some(status) = &filter.status {
                if !status.is_empty() && status != "ALL" {
                    conditions.push("s.status = ?".to_string());
                    query_params.push(status.clone().into());
                }
            }

            let where_clause = if conditions.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let count_sql = format!("SELECT COUNT(*) FROM sessions s {}", where_clause);
            let mut count_stmt = conn.prepare(&count_sql)?;
            let total: i64 = count_stmt.query_row(
                rusqlite::params_from_iter(query_params.iter()),
                |row| row.get(0),
            )?;

            let page = filter.page.unwrap_or(1).max(1);
            let page_size = filter.page_size.unwrap_or(20).clamp(1, 100);
            let offset = (page - 1) * page_size;

            let query_sql = format!(
                r#"
                SELECT s.id, s.runner_name, s.native_session_id, s.title, s.started_at, s.ended_at, s.status, s.created_at,
                       COUNT(DISTINCT e.id) as exec_count,
                       COUNT(DISTINCT b.runtime_id) as runtime_count,
                       SUM(CASE WHEN b.reason = 'RESUME' THEN 1 ELSE 0 END) as resume_count,
                       COALESCE(MAX(e.started_at), s.started_at) as last_active_at,
                       s.parent_session_id,
                       s.fork_reason,
                       s.forked_at,
                       COUNT(DISTINCT sc.id) as conflicts_count
                FROM sessions s
                LEFT JOIN executions e ON s.id = e.session_id
                LEFT JOIN runtime_session_bindings b ON s.id = b.session_id
                LEFT JOIN session_conflicts sc ON (s.id = sc.session_id OR s.id = sc.conflicting_session_id)
                {}
                GROUP BY s.id
                ORDER BY last_active_at DESC
                LIMIT {} OFFSET {}
                "#,
                where_clause, page_size, offset
            );

            let mut stmt = conn.prepare(&query_sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(query_params.iter()),
                |row| {
                    Ok(SessionWithStats {
                        id: row.get(0)?,
                        runner_name: row.get(1)?,
                        native_session_id: row.get(2)?,
                        title: row.get(3)?,
                        started_at: row.get(4)?,
                        ended_at: row.get(5)?,
                        status: row.get(6)?,
                        created_at: row.get(7)?,
                        execution_count: row.get(8)?,
                        runtime_count: row.get(9)?,
                        resume_count: row.get(10)?,
                        last_active_at: row.get(11)?,
                        parent_session_id: row.get(12)?,
                        fork_reason: row.get(13)?,
                        forked_at: row.get(14)?,
                        conflicts_count: row.get(15)?,
                    })
                },
            )?;

            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }

            Ok((items, total as usize))
        })
    }

    pub fn list_executions_for_session(&self, session_id: &str) -> Result<Vec<Execution>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, session_id, runtime_id, native_execution_id, turn_index,
                       prompt_summary, model, reasoning_effort, selected_agent_role,
                       started_at, ended_at, duration_ms, status, exit_code, error_message,
                       repo_root, worktree_path, branch, head_sha, git_attribution
                FROM executions WHERE session_id = ?1
                ORDER BY started_at ASC
                "#,
            )?;
            let rows = stmt.query_map(params![session_id], |row| {
                Ok(Execution {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    runtime_id: row.get(2)?,
                    native_execution_id: row.get(3)?,
                    turn_index: row.get(4)?,
                    prompt_summary: row.get(5)?,
                    model: row.get(6)?,
                    reasoning_effort: row.get(7)?,
                    selected_agent_role: row.get(8)?,
                    started_at: row.get(9)?,
                    ended_at: row.get(10)?,
                    duration_ms: row.get(11)?,
                    status: row.get(12)?,
                    exit_code: row.get(13)?,
                    error_message: row.get(14)?,
                    repo_root: row.get(15)?,
                    worktree_path: row.get(16)?,
                    branch: row.get(17)?,
                    head_sha: row.get(18)?,
                    git_attribution: row.get(19)?,
                })
            })?;
            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }

    pub fn get_stats(&self) -> Result<StatsSummary> {
        self.db.with_conn(|conn| {
            let total_executions: i64 = conn.query_row("SELECT COUNT(*) FROM executions", [], |r| r.get(0))?;
            let total_sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
            let total_runtimes: i64 = conn.query_row("SELECT COUNT(*) FROM runtime_instances", [], |r| r.get(0))?;
            let total_conflicts: i64 = conn.query_row("SELECT COUNT(*) FROM session_conflicts", [], |r| r.get(0)).unwrap_or(0);

            // by_runner
            let mut stmt = conn.prepare(
                "SELECT r.runner_name, COUNT(e.id) FROM executions e JOIN runtime_instances r ON e.runtime_id = r.id GROUP BY r.runner_name ORDER BY COUNT(e.id) DESC"
            )?;
            let by_runner = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // by_model
            let mut stmt = conn.prepare(
                "SELECT model, COUNT(*) FROM executions GROUP BY model ORDER BY COUNT(*) DESC"
            )?;
            let by_model = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // by_reasoning
            let mut stmt = conn.prepare(
                "SELECT reasoning_effort, COUNT(*) FROM executions GROUP BY reasoning_effort ORDER BY COUNT(*) DESC"
            )?;
            let by_reasoning = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // by_agent
            let mut stmt = conn.prepare(
                "SELECT selected_agent_role, COUNT(*) FROM executions GROUP BY selected_agent_role ORDER BY COUNT(*) DESC"
            )?;
            let by_agent = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // by_status
            let mut stmt = conn.prepare(
                "SELECT status, COUNT(*) FROM executions GROUP BY status ORDER BY COUNT(*) DESC"
            )?;
            let by_status = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // mcp_stats: configured vs invoked
            let mut stmt = conn.prepare(
                r#"
                SELECT c.name,
                       SUM(CASE WHEN ec.state = 'CONFIGURED' OR ec.state = 'DISCOVERED' THEN 1 ELSE 0 END) as configured_count,
                       SUM(CASE WHEN ec.state = 'INVOKED' OR ec.invocations_count > 0 THEN 1 ELSE 0 END) as invoked_count
                FROM components c
                JOIN execution_components ec ON c.id = ec.component_id
                WHERE c.component_type = 'MCP'
                GROUP BY c.name
                ORDER BY configured_count DESC
                "#
            )?;
            let mcp_stats = stmt.query_map([], |row| {
                Ok(McpStat {
                    name: row.get(0)?,
                    configured: row.get(1)?,
                    invoked: row.get(2)?,
                })
            })?.collect::<Result<Vec<_>>>()?;

            // skill_stats
            let mut stmt = conn.prepare(
                r#"
                SELECT c.name, COUNT(ec.execution_id)
                FROM components c
                JOIN execution_components ec ON c.id = ec.component_id
                WHERE c.component_type = 'SKILL'
                GROUP BY c.name
                ORDER BY COUNT(ec.execution_id) DESC
                "#
            )?;
            let skill_stats = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // plugin_stats
            let mut stmt = conn.prepare(
                r#"
                SELECT c.name, COUNT(ec.execution_id)
                FROM components c
                JOIN execution_components ec ON c.id = ec.component_id
                WHERE c.component_type = 'PLUGIN'
                GROUP BY c.name
                ORDER BY COUNT(ec.execution_id) DESC
                "#
            )?;
            let plugin_stats = stmt.query_map([], |row| Ok(KeyCount { key: row.get(0)?, count: row.get(1)? }))?.collect::<Result<Vec<_>>>()?;

            // avg_duration_ms
            let avg_duration_ms: f64 = conn.query_row(
                "SELECT COALESCE(AVG(duration_ms), 0.0) FROM executions WHERE duration_ms IS NOT NULL AND duration_ms > 0",
                [],
                |row| row.get(0),
            )?;

            Ok(StatsSummary {
                total_executions,
                total_sessions,
                total_runtimes,
                total_conflicts,
                by_runner,
                by_model,
                by_reasoning,
                by_agent,
                by_status,
                mcp_stats,
                skill_stats,
                plugin_stats,
                avg_duration_ms,
            })
        })
    }

    pub fn get_health_metrics(&self) -> Result<HealthMetrics> {
        self.db.with_conn(|conn| {
            let active_executions: i64 = conn.query_row("SELECT COUNT(*) FROM executions WHERE status = 'RUNNING'", [], |r| r.get(0))?;
            let active_runtimes: i64 = conn.query_row("SELECT COUNT(*) FROM runtime_instances WHERE status = 'RUNNING'", [], |r| r.get(0))?;
            let total_executions: i64 = conn.query_row("SELECT COUNT(*) FROM executions", [], |r| r.get(0))?;
            let total_sessions: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;

            Ok(HealthMetrics {
                status: "ok".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                db_connected: true,
                active_executions,
                active_runtimes,
                total_executions,
                total_sessions,
            })
        })
    }
}
