use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInstance {
    pub id: String,
    pub runner_name: String,
    pub runner_version: String,
    pub surface: String,
    pub pid: Option<u32>,
    pub hostname: String,
    pub os: String,
    pub cwd: String,
    pub command_line: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub runner_name: String,
    pub native_session_id: String,
    pub title: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub created_at: String,
    pub parent_session_id: Option<String>,
    pub fork_reason: Option<String>,
    pub forked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConflict {
    pub id: String,
    pub session_id: String,
    pub conflicting_session_id: Option<String>,
    pub execution_id: Option<String>,
    pub conflict_type: String,
    pub severity: String,
    pub detected_at: String,
    pub resolved_at: Option<String>,
    pub details_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSessionBinding {
    pub id: String,
    pub runtime_id: String,
    pub session_id: String,
    pub bound_at: String,
    pub unbound_at: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub id: String,
    pub session_id: String,
    pub runtime_id: String,
    pub native_execution_id: String,
    pub turn_index: i32,
    pub prompt_summary: Option<String>,
    pub model: String,
    pub reasoning_effort: String,
    pub selected_agent_role: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    pub repo_root: Option<String>,
    pub worktree_path: Option<String>,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub git_attribution: String,
    pub capture_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstance {
    pub id: String,
    pub execution_id: String,
    pub parent_agent_id: Option<String>,
    pub agent_role: String,
    pub agent_name: String,
    pub model: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub component_type: String, // 'MCP', 'SKILL', 'PLUGIN', 'TOOL', 'CONFIG'
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionComponent {
    pub execution_id: String,
    pub component_id: String,
    pub state: String, // 'CONFIGURED', 'DISCOVERED', 'LOADED', 'INVOKED', 'OBSERVED', 'UNKNOWN'
    pub invocations_count: i32,
    pub details_json: Option<String>,
    // Optional joined fields for API convenience
    pub component_type: Option<String>,
    pub component_name: Option<String>,
    pub component_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSnapshot {
    pub id: String,
    pub execution_id: String,
    pub snapshot_type: String, // 'BEFORE', 'AFTER'
    pub captured_at: String,
    pub repo_root: String,
    pub worktree_path: String,
    pub branch: String,
    pub head_commit: String,
    pub is_dirty: bool,
    pub changed_files_count: i32,
    pub diff_stat: Option<String>,
    pub changed_files_json: Option<String>,
    pub attribution: String, // 'OBSERVED', 'CORRELATED', 'AMBIGUOUS', 'UNKNOWN'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub id: String,
    pub content_sha256: String,
    pub config_type: String,
    pub file_path: Option<String>,
    pub raw_content_redacted: String,
    pub parsed_json: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Option<i64>,
    pub event_id: String,
    pub timestamp: String,
    pub runtime_id: Option<String>,
    pub session_id: Option<String>,
    pub execution_id: Option<String>,
    pub agent_instance_id: Option<String>,
    pub event_type: String,
    pub source: String,
    pub payload_json: String,
}
