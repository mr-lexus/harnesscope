use chrono::{Duration, Utc};
use rusqlite::Result;
use uuid::Uuid;

use crate::domain::models::*;
use crate::storage::Repository;

pub fn seed_demo_data(repo: &Repository) -> Result<String> {
    let base_time = Utc::now() - Duration::hours(2);

    // 1. Session 1: Codex CLI with RESUME across runtime restart!
    // Native session: "codex-thread-8841"
    let sess1_id = "sess_codex_demo_01".to_string();
    let sess1 = Session {
        id: sess1_id.clone(),
        runner_name: "codex".to_string(),
        native_session_id: "codex-thread-8841".to_string(),
        title: Some("Implement JWT auth middleware".to_string()),
        started_at: (base_time).to_rfc3339(),
        ended_at: None,
        status: "ACTIVE".to_string(),
        created_at: (base_time).to_rfc3339(),
        parent_session_id: None,
        fork_reason: None,
        forked_at: None,
    };
    repo.save_session(&sess1)?;

    // Session 1 Fork: developer forked off Turn 2 to test PASETO tokens
    let sess1_fork_id = "sess_codex_demo_01_paseto".to_string();
    let sess1_fork = Session {
        id: sess1_fork_id.clone(),
        runner_name: "codex".to_string(),
        native_session_id: "codex-thread-8841-fork-paseto".to_string(),
        title: Some("Experiment: PASETO tokens instead of JWT".to_string()),
        started_at: (base_time + Duration::minutes(26)).to_rfc3339(),
        ended_at: None,
        status: "ACTIVE".to_string(),
        created_at: (base_time + Duration::minutes(26)).to_rfc3339(),
        parent_session_id: Some(sess1_id.clone()),
        fork_reason: Some("MANUAL_BRANCH".to_string()),
        forked_at: Some((base_time + Duration::minutes(26)).to_rfc3339()),
    };
    repo.save_session(&sess1_fork)?;

    // Runtime 1A (Codex process 1)
    let run1a_id = "run_codex_proc_101".to_string();
    let run1a = RuntimeInstance {
        id: run1a_id.clone(),
        runner_name: "codex".to_string(),
        runner_version: "0.14.2".to_string(),
        surface: "cli".to_string(),
        pid: Some(14208),
        hostname: "dev-workstation".to_string(),
        os: std::env::consts::OS.to_string(),
        cwd: "C:/projects/auth-service".to_string(),
        command_line: "codex --model gpt-4o --reasoning-effort high --session codex-thread-8841".to_string(),
        started_at: (base_time).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(25)).to_rfc3339()),
        exit_code: Some(0),
        status: "COMPLETED".to_string(),
    };
    repo.save_runtime_instance(&run1a)?;

    let bind1a = RuntimeSessionBinding {
        id: format!("bind_{}_{}", run1a_id, sess1_id),
        runtime_id: run1a_id.clone(),
        session_id: sess1_id.clone(),
        bound_at: (base_time).to_rfc3339(),
        unbound_at: Some((base_time + Duration::minutes(25)).to_rfc3339()),
        reason: "START".to_string(),
    };
    repo.save_runtime_session_binding(&bind1a)?;

    // Execution 1: Turn 1 in Runtime 1A
    let exec1_id = "exec_codex_turn_01".to_string();
    let exec1 = Execution {
        id: exec1_id.clone(),
        session_id: sess1_id.clone(),
        runtime_id: run1a_id.clone(),
        native_execution_id: "req_cdx_01_aaa".to_string(),
        turn_index: 0,
        prompt_summary: Some("Scaffold JWT token generation and validation tests".to_string()),
        model: "gpt-4o".to_string(),
        reasoning_effort: "high".to_string(),
        selected_agent_role: "coder".to_string(),
        started_at: (base_time + Duration::seconds(5)).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(10)).to_rfc3339()),
        duration_ms: Some(595000),
        status: "COMPLETED".to_string(),
        exit_code: Some(0),
        error_message: None,
        repo_root: Some("C:/projects/auth-service".to_string()),
        worktree_path: Some("C:/projects/auth-service".to_string()),
        branch: Some("feature/jwt-auth".to_string()),
        head_sha: Some("d3a84b29c112".to_string()),
        git_attribution: "OBSERVED".to_string(),
    };
    repo.save_execution(&exec1)?;

    // Execution 2: Turn 2 in Runtime 1A
    let exec2_id = "exec_codex_turn_02".to_string();
    let exec2 = Execution {
        id: exec2_id.clone(),
        session_id: sess1_id.clone(),
        runtime_id: run1a_id.clone(),
        native_execution_id: "req_cdx_01_bbb".to_string(),
        turn_index: 1,
        prompt_summary: Some("Fix expiration claim boundary check".to_string()),
        model: "gpt-4o".to_string(),
        reasoning_effort: "high".to_string(),
        selected_agent_role: "coder".to_string(),
        started_at: (base_time + Duration::minutes(12)).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(24)).to_rfc3339()),
        duration_ms: Some(720000),
        status: "COMPLETED".to_string(),
        exit_code: Some(0),
        error_message: None,
        repo_root: Some("C:/projects/auth-service".to_string()),
        worktree_path: Some("C:/projects/auth-service".to_string()),
        branch: Some("feature/jwt-auth".to_string()),
        head_sha: Some("9f110c4d5e21".to_string()),
        git_attribution: "OBSERVED".to_string(),
    };
    repo.save_execution(&exec2)?;

    // Now Runtime 1B resumes Session 1 after Runtime 1A has stopped!
    let run1b_id = "run_codex_proc_102".to_string();
    let run1b = RuntimeInstance {
        id: run1b_id.clone(),
        runner_name: "codex".to_string(),
        runner_version: "0.14.2".to_string(),
        surface: "cli".to_string(),
        pid: Some(19840),
        hostname: "dev-workstation".to_string(),
        os: std::env::consts::OS.to_string(),
        cwd: "C:/projects/auth-service".to_string(),
        command_line: "codex --resume --session codex-thread-8841".to_string(),
        started_at: (base_time + Duration::minutes(40)).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(55)).to_rfc3339()),
        exit_code: Some(0),
        status: "COMPLETED".to_string(),
    };
    repo.save_runtime_instance(&run1b)?;

    let bind1b = RuntimeSessionBinding {
        id: format!("bind_{}_{}", run1b_id, sess1_id),
        runtime_id: run1b_id.clone(),
        session_id: sess1_id.clone(),
        bound_at: (base_time + Duration::minutes(40)).to_rfc3339(),
        unbound_at: Some((base_time + Duration::minutes(55)).to_rfc3339()),
        reason: "RESUME".to_string(),
    };
    repo.save_runtime_session_binding(&bind1b)?;

    // Execution 3: Turn 3 in Runtime 1B under the SAME session!
    let exec3_id = "exec_codex_turn_03".to_string();
    let exec3 = Execution {
        id: exec3_id.clone(),
        session_id: sess1_id.clone(),
        runtime_id: run1b_id.clone(),
        native_execution_id: "req_cdx_01_ccc".to_string(),
        turn_index: 2,
        prompt_summary: Some("Add refresh token endpoint and documentation".to_string()),
        model: "gpt-4o".to_string(),
        reasoning_effort: "medium".to_string(),
        selected_agent_role: "architect".to_string(),
        started_at: (base_time + Duration::minutes(42)).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(54)).to_rfc3339()),
        duration_ms: Some(720000),
        status: "COMPLETED".to_string(),
        exit_code: Some(0),
        error_message: None,
        repo_root: Some("C:/projects/auth-service".to_string()),
        worktree_path: Some("C:/projects/auth-service".to_string()),
        branch: Some("feature/jwt-auth".to_string()),
        head_sha: Some("3e82bb71aa90".to_string()),
        git_attribution: "OBSERVED".to_string(),
    };
    repo.save_execution(&exec3)?;

    // 2. Parallel Executions Example: OpenCode CLI and GitHub Copilot CLI in the SAME WORKTREE simultaneously!
    // -> Git attribution MUST BE "AMBIGUOUS"
    let parallel_start = base_time + Duration::minutes(60);
    let parallel_end = base_time + Duration::minutes(75);

    // Session 2: OpenCode
    let sess2_id = "sess_opencode_demo_02".to_string();
    let sess2 = Session {
        id: sess2_id.clone(),
        runner_name: "opencode".to_string(),
        native_session_id: "oc_sess_90021_alpha".to_string(),
        title: Some("Refactor database connection pool".to_string()),
        started_at: parallel_start.to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        status: "COMPLETED".to_string(),
        created_at: parallel_start.to_rfc3339(),
        parent_session_id: None,
        fork_reason: None,
        forked_at: None,
    };
    repo.save_session(&sess2)?;

    let run2_id = "run_opencode_proc_201".to_string();
    let run2 = RuntimeInstance {
        id: run2_id.clone(),
        runner_name: "opencode".to_string(),
        runner_version: "1.1.0".to_string(),
        surface: "tui".to_string(),
        pid: Some(22410),
        hostname: "dev-workstation".to_string(),
        os: std::env::consts::OS.to_string(),
        cwd: "C:/projects/shared-repo".to_string(),
        command_line: "opencode --agent coder --model claude-3-5-sonnet".to_string(),
        started_at: parallel_start.to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        exit_code: Some(0),
        status: "COMPLETED".to_string(),
    };
    repo.save_runtime_instance(&run2)?;

    let bind2 = RuntimeSessionBinding {
        id: format!("bind_{}_{}", run2_id, sess2_id),
        runtime_id: run2_id.clone(),
        session_id: sess2_id.clone(),
        bound_at: parallel_start.to_rfc3339(),
        unbound_at: Some(parallel_end.to_rfc3339()),
        reason: "START".to_string(),
    };
    repo.save_runtime_session_binding(&bind2)?;

    let exec4_id = "exec_opencode_parallel_04".to_string();
    let exec4 = Execution {
        id: exec4_id.clone(),
        session_id: sess2_id.clone(),
        runtime_id: run2_id.clone(),
        native_execution_id: "oc_turn_442".to_string(),
        turn_index: 0,
        prompt_summary: Some("Optimize pool retry configuration".to_string()),
        model: "claude-3-5-sonnet".to_string(),
        reasoning_effort: "none".to_string(),
        selected_agent_role: "coder".to_string(),
        started_at: parallel_start.to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        duration_ms: Some(900000),
        status: "COMPLETED".to_string(),
        exit_code: Some(0),
        error_message: None,
        repo_root: Some("C:/projects/shared-repo".to_string()),
        worktree_path: Some("C:/projects/shared-repo".to_string()),
        branch: Some("main".to_string()),
        head_sha: Some("7c83f12401ba".to_string()),
        git_attribution: "AMBIGUOUS".to_string(), // parallel in same worktree!
    };
    repo.save_execution(&exec4)?;

    // Session 3: Copilot CLI concurrently running in the SAME worktree "C:/projects/shared-repo"!
    let sess3_id = "sess_copilot_demo_03".to_string();
    let sess3 = Session {
        id: sess3_id.clone(),
        runner_name: "copilot".to_string(),
        native_session_id: "gh_copilot_turn_71".to_string(),
        title: Some("Generate unit tests for metrics exporter".to_string()),
        started_at: (parallel_start + Duration::minutes(2)).to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        status: "COMPLETED".to_string(),
        created_at: (parallel_start + Duration::minutes(2)).to_rfc3339(),
        parent_session_id: None,
        fork_reason: None,
        forked_at: None,
    };
    repo.save_session(&sess3)?;

    let run3_id = "run_copilot_proc_301".to_string();
    let run3 = RuntimeInstance {
        id: run3_id.clone(),
        runner_name: "copilot".to_string(),
        runner_version: "0.5.4".to_string(),
        surface: "cli".to_string(),
        pid: Some(25112),
        hostname: "dev-workstation".to_string(),
        os: std::env::consts::OS.to_string(),
        cwd: "C:/projects/shared-repo".to_string(),
        command_line: "copilot --model gpt-4o".to_string(),
        started_at: (parallel_start + Duration::minutes(2)).to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        exit_code: Some(0),
        status: "COMPLETED".to_string(),
    };
    repo.save_runtime_instance(&run3)?;

    let bind3 = RuntimeSessionBinding {
        id: format!("bind_{}_{}", run3_id, sess3_id),
        runtime_id: run3_id.clone(),
        session_id: sess3_id.clone(),
        bound_at: (parallel_start + Duration::minutes(2)).to_rfc3339(),
        unbound_at: Some(parallel_end.to_rfc3339()),
        reason: "START".to_string(),
    };
    repo.save_runtime_session_binding(&bind3)?;

    let exec5_id = "exec_copilot_parallel_05".to_string();
    let exec5 = Execution {
        id: exec5_id.clone(),
        session_id: sess3_id.clone(),
        runtime_id: run3_id.clone(),
        native_execution_id: "gh_exec_71_01".to_string(),
        turn_index: 0,
        prompt_summary: Some("Write unit test suite for Prometheus exporter".to_string()),
        model: "gpt-4o".to_string(),
        reasoning_effort: "UNKNOWN".to_string(),
        selected_agent_role: "test-writer".to_string(),
        started_at: (parallel_start + Duration::minutes(2)).to_rfc3339(),
        ended_at: Some(parallel_end.to_rfc3339()),
        duration_ms: Some(780000),
        status: "COMPLETED".to_string(),
        exit_code: Some(0),
        error_message: None,
        repo_root: Some("C:/projects/shared-repo".to_string()),
        worktree_path: Some("C:/projects/shared-repo".to_string()),
        branch: Some("main".to_string()),
        head_sha: Some("7c83f12401ba".to_string()),
        git_attribution: "AMBIGUOUS".to_string(), // parallel in same worktree!
    };
    repo.save_execution(&exec5)?;

    // 3. Session 4: OpenCode with UNKNOWN native session, UNKNOWN model, Failed execution
    let sess4_start = base_time + Duration::minutes(80);
    let sess4_id = "sess_opencode_unknown_04".to_string();
    let sess4 = Session {
        id: sess4_id.clone(),
        runner_name: "opencode".to_string(),
        native_session_id: "UNKNOWN".to_string(),
        title: Some("Run exploratory benchmark".to_string()),
        started_at: sess4_start.to_rfc3339(),
        ended_at: Some((sess4_start + Duration::minutes(5)).to_rfc3339()),
        status: "COMPLETED".to_string(),
        created_at: sess4_start.to_rfc3339(),
        parent_session_id: None,
        fork_reason: None,
        forked_at: None,
    };
    repo.save_session(&sess4)?;

    // Demo Session Conflict: Concurrent overlap between OpenCode and Copilot sessions
    let demo_conflict = SessionConflict {
        id: "conf_demo_shared_wt_01".to_string(),
        session_id: sess2_id.clone(),
        conflicting_session_id: Some(sess3_id.clone()),
        execution_id: Some(exec4_id.clone()),
        conflict_type: "WORKTREE_OVERLAP".to_string(),
        severity: "WARNING".to_string(),
        detected_at: parallel_start.to_rfc3339(),
        resolved_at: None,
        details_json: Some(r#"{"message": "Two independent agent sessions concurrently modified C:/projects/shared-repo"}"#.to_string()),
    };
    repo.save_session_conflict(&demo_conflict)?;

    let run4_id = "run_opencode_proc_401".to_string();
    let run4 = RuntimeInstance {
        id: run4_id.clone(),
        runner_name: "opencode".to_string(),
        runner_version: "UNKNOWN".to_string(),
        surface: "cli".to_string(),
        pid: Some(31200),
        hostname: "dev-workstation".to_string(),
        os: std::env::consts::OS.to_string(),
        cwd: "C:/projects/benchmark".to_string(),
        command_line: "opencode run benchmark".to_string(),
        started_at: sess4_start.to_rfc3339(),
        ended_at: Some((sess4_start + Duration::minutes(5)).to_rfc3339()),
        exit_code: Some(1),
        status: "FAILED".to_string(),
    };
    repo.save_runtime_instance(&run4)?;

    let bind4 = RuntimeSessionBinding {
        id: format!("bind_{}_{}", run4_id, sess4_id),
        runtime_id: run4_id.clone(),
        session_id: sess4_id.clone(),
        bound_at: sess4_start.to_rfc3339(),
        unbound_at: Some((sess4_start + Duration::minutes(5)).to_rfc3339()),
        reason: "START".to_string(),
    };
    repo.save_runtime_session_binding(&bind4)?;

    let exec6_id = "exec_failed_06".to_string();
    let exec6 = Execution {
        id: exec6_id.clone(),
        session_id: sess4_id.clone(),
        runtime_id: run4_id.clone(),
        native_execution_id: "UNKNOWN".to_string(),
        turn_index: 0,
        prompt_summary: Some("Run load test benchmark against localhost".to_string()),
        model: "UNKNOWN".to_string(),
        reasoning_effort: "UNKNOWN".to_string(),
        selected_agent_role: "UNKNOWN".to_string(),
        started_at: sess4_start.to_rfc3339(),
        ended_at: Some((sess4_start + Duration::minutes(5)).to_rfc3339()),
        duration_ms: Some(300000),
        status: "FAILED".to_string(),
        exit_code: Some(1),
        error_message: Some("Connection refused at port 8080".to_string()),
        repo_root: None,
        worktree_path: None,
        branch: None,
        head_sha: None,
        git_attribution: "UNKNOWN".to_string(),
    };
    repo.save_execution(&exec6)?;

    // Components & Execution Components
    let comp_mcp_fs = Component {
        id: "comp_mcp_filesystem".to_string(),
        component_type: "MCP".to_string(),
        name: "filesystem".to_string(),
        version: Some("1.2.0".to_string()),
        description: Some("Model Context Protocol local file access server".to_string()),
    };
    let comp_mcp_git = Component {
        id: "comp_mcp_git".to_string(),
        component_type: "MCP".to_string(),
        name: "git".to_string(),
        version: Some("0.4.1".to_string()),
        description: Some("Model Context Protocol git operations server".to_string()),
    };
    let comp_mcp_pg = Component {
        id: "comp_mcp_postgres".to_string(),
        component_type: "MCP".to_string(),
        name: "postgres".to_string(),
        version: Some("0.2.0".to_string()),
        description: Some("Database inspection and query execution".to_string()),
    };
    let comp_skill_review = Component {
        id: "comp_skill_code_review".to_string(),
        component_type: "SKILL".to_string(),
        name: "code-review".to_string(),
        version: Some("1.0".to_string()),
        description: Some("Automated diff review skill".to_string()),
    };
    let comp_plugin_omo = Component {
        id: "comp_plugin_omo_presets".to_string(),
        component_type: "PLUGIN".to_string(),
        name: "oh-my-opencode-presets".to_string(),
        version: Some("2.3.0".to_string()),
        description: Some("Curated prompts and tool presets for OpenCode".to_string()),
    };

    let id_fs = repo.save_component(&comp_mcp_fs)?;
    let id_git = repo.save_component(&comp_mcp_git)?;
    let id_pg = repo.save_component(&comp_mcp_pg)?;
    let id_skill = repo.save_component(&comp_skill_review)?;
    let id_omo = repo.save_component(&comp_plugin_omo)?;

    // Execution 1 components:
    // Filesystem: INVOKED (4 calls)
    // Git: INVOKED (2 calls)
    // Postgres: CONFIGURED (0 calls)
    repo.save_execution_component(&ExecutionComponent {
        execution_id: exec1_id.clone(),
        component_id: id_fs.clone(),
        state: "INVOKED".to_string(),
        invocations_count: 4,
        details_json: Some(r#"{"read_files": 3, "write_files": 1}"#.to_string()),
        component_type: None, component_name: None, component_version: None,
    })?;
    repo.save_execution_component(&ExecutionComponent {
        execution_id: exec1_id.clone(),
        component_id: id_git.clone(),
        state: "INVOKED".to_string(),
        invocations_count: 2,
        details_json: Some(r#"{"status_check": true}"#.to_string()),
        component_type: None, component_name: None, component_version: None,
    })?;
    repo.save_execution_component(&ExecutionComponent {
        execution_id: exec1_id.clone(),
        component_id: id_pg.clone(),
        state: "CONFIGURED".to_string(),
        invocations_count: 0,
        details_json: None,
        component_type: None, component_name: None, component_version: None,
    })?;

    // Execution 4 (OpenCode) components:
    repo.save_execution_component(&ExecutionComponent {
        execution_id: exec4_id.clone(),
        component_id: id_omo.clone(),
        state: "LOADED".to_string(),
        invocations_count: 0,
        details_json: None,
        component_type: None, component_name: None, component_version: None,
    })?;
    repo.save_execution_component(&ExecutionComponent {
        execution_id: exec4_id.clone(),
        component_id: id_skill.clone(),
        state: "INVOKED".to_string(),
        invocations_count: 1,
        details_json: None,
        component_type: None, component_name: None, component_version: None,
    })?;

    // Subagent instances for Execution 1
    let subagent1 = AgentInstance {
        id: "agent_sub_reviewer_01".to_string(),
        execution_id: exec1_id.clone(),
        parent_agent_id: None,
        agent_role: "reviewer".to_string(),
        agent_name: "test-reviewer".to_string(),
        model: "gpt-4o-mini".to_string(),
        started_at: (base_time + Duration::minutes(7)).to_rfc3339(),
        ended_at: Some((base_time + Duration::minutes(9)).to_rfc3339()),
        status: "COMPLETED".to_string(),
    };
    repo.save_agent_instance(&subagent1)?;

    // Git Snapshots for Execution 1: BEFORE & AFTER
    let snap_before = GitSnapshot {
        id: format!("snap_before_{}", exec1_id),
        execution_id: exec1_id.clone(),
        snapshot_type: "BEFORE".to_string(),
        captured_at: (base_time + Duration::seconds(6)).to_rfc3339(),
        repo_root: "C:/projects/auth-service".to_string(),
        worktree_path: "C:/projects/auth-service".to_string(),
        branch: "feature/jwt-auth".to_string(),
        head_commit: "d3a84b29c112".to_string(),
        is_dirty: false,
        changed_files_count: 0,
        diff_stat: None,
        changed_files_json: Some("[]".to_string()),
        attribution: "OBSERVED".to_string(),
    };
    repo.save_git_snapshot(&snap_before)?;

    let snap_after = GitSnapshot {
        id: format!("snap_after_{}", exec1_id),
        execution_id: exec1_id.clone(),
        snapshot_type: "AFTER".to_string(),
        captured_at: (base_time + Duration::minutes(10)).to_rfc3339(),
        repo_root: "C:/projects/auth-service".to_string(),
        worktree_path: "C:/projects/auth-service".to_string(),
        branch: "feature/jwt-auth".to_string(),
        head_commit: "d3a84b29c112".to_string(),
        is_dirty: true,
        changed_files_count: 3,
        diff_stat: Some("3 files changed, 142 insertions(+), 8 deletions(-)".to_string()),
        changed_files_json: Some(r#"[
            {"status": "M", "path": "src/auth/jwt.rs"},
            {"status": "A", "path": "tests/jwt_validation_test.rs"},
            {"status": "M", "path": "Cargo.toml"}
        ]"#.to_string()),
        attribution: "OBSERVED".to_string(),
    };
    repo.save_git_snapshot(&snap_after)?;

    // Timeline events for Execution 1
    let events = vec![
        Event {
            id: None,
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: (base_time + Duration::seconds(5)).to_rfc3339(),
            runtime_id: Some(run1a_id.clone()),
            session_id: Some(sess1_id.clone()),
            execution_id: Some(exec1_id.clone()),
            agent_instance_id: None,
            event_type: "execution.started".to_string(),
            source: "wrapper".to_string(),
            payload_json: r#"{"turn_index": 0, "model": "gpt-4o", "reasoning_effort": "high"}"#.to_string(),
        },
        Event {
            id: None,
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: (base_time + Duration::minutes(2)).to_rfc3339(),
            runtime_id: Some(run1a_id.clone()),
            session_id: Some(sess1_id.clone()),
            execution_id: Some(exec1_id.clone()),
            agent_instance_id: None,
            event_type: "mcp.invoked".to_string(),
            source: "runner_event".to_string(),
            payload_json: r#"{"tool": "filesystem.read_file", "path": "src/auth/mod.rs"}"#.to_string(),
        },
        Event {
            id: None,
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: (base_time + Duration::minutes(7)).to_rfc3339(),
            runtime_id: Some(run1a_id.clone()),
            session_id: Some(sess1_id.clone()),
            execution_id: Some(exec1_id.clone()),
            agent_instance_id: Some(subagent1.id.clone()),
            event_type: "subagent.started".to_string(),
            source: "runner_event".to_string(),
            payload_json: r#"{"role": "reviewer", "task": "verify boundary conditions"}"#.to_string(),
        },
        Event {
            id: None,
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: (base_time + Duration::minutes(10)).to_rfc3339(),
            runtime_id: Some(run1a_id.clone()),
            session_id: Some(sess1_id.clone()),
            execution_id: Some(exec1_id.clone()),
            agent_instance_id: None,
            event_type: "execution.completed".to_string(),
            source: "wrapper".to_string(),
            payload_json: r#"{"status": "COMPLETED", "exit_code": 0, "duration_ms": 595000}"#.to_string(),
        },
    ];

    for ev in events {
        repo.save_event(&ev)?;
    }

    Ok("Demo data successfully seeded: 4 sessions, 5 runtimes, 6 executions, components, and git snapshots.".to_string())
}
