use chrono::Utc;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use uuid::Uuid;

use crate::domain::events::IngestEvent;
use crate::git::capture_git_context;
use crate::runners::codex::{parse_codex_args_and_env, RunnerMetadata};
use crate::runners::copilot::parse_copilot_args_and_env;
use crate::runners::discovery::{discover_runner_binary, find_executable};
use crate::runners::opencode::parse_opencode_args_and_env;

#[cfg(windows)]
pub fn spawn_background_server(exe_path: &Path, port: u16) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new(exe_path)
        .args(["serve", "--port", &port.to_string()])
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
pub fn spawn_background_server(exe_path: &Path, port: u16) -> std::io::Result<()> {
    Command::new(exe_path)
        .args(["serve", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn ensure_server_running(server_url: &str) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let health_url = format!("{}/api/v1/health", server_url.trim_end_matches('/'));
    if client.get(&health_url).send().map(|r| r.status().is_success()).unwrap_or(false) {
        return;
    }

    if let Ok(exe) = std::env::current_exe() {
        let _ = spawn_background_server(&exe, 4242);
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if client.get(&health_url).send().map(|r| r.status().is_success()).unwrap_or(false) {
                break;
            }
        }
    }
}

fn send_telemetry_events(server_url: &str, events: &[IngestEvent]) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let url = format!("{}/api/v1/events", server_url.trim_end_matches('/'));
    let _ = client.post(&url).json(events).send();
}

fn parse_generic_args_and_env(runner_name: &str, args: &[String], _cwd: &Path) -> RunnerMetadata {
    let mut meta = RunnerMetadata {
        runner_name: runner_name.to_string(),
        runner_version: "UNKNOWN".to_string(),
        native_session_id: "UNKNOWN".to_string(),
        model: "UNKNOWN".to_string(),
        reasoning_effort: "UNKNOWN".to_string(),
        selected_agent_role: "UNKNOWN".to_string(),
        prompt_summary: None,
        components: Vec::new(),
        config_snapshots: Vec::new(),
    };

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if (arg == "--model" || arg == "-m") && i + 1 < args.len() {
            meta.model = args[i + 1].clone();
            i += 2;
            continue;
        } else if (arg == "--session" || arg == "-s") && i + 1 < args.len() {
            meta.native_session_id = args[i + 1].clone();
            i += 2;
            continue;
        } else if (arg == "--agent" || arg == "-a") && i + 1 < args.len() {
            meta.selected_agent_role = args[i + 1].clone();
            i += 2;
            continue;
        }
        i += 1;
    }

    meta
}

pub fn execute_wrapper_generic(
    runner_name: &str,
    surface: &str,
    command: &str,
    args: &[String],
    server_url: &str,
) -> i32 {
    // Ensure background server is running to collect telemetry
    ensure_server_running(server_url);

    let binary_path = match discover_runner_binary(command).or_else(|| find_executable(command)) {
        Some(path) => path,
        None => {
            eprintln!(
                "Harnesscope Error: Could not find '{}' executable on PATH.\n\
                 Ensure it is installed or specify the full path.",
                command
            );
            return 1;
        }
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let start_instant = Instant::now();
    let start_time = Utc::now().to_rfc3339();

    let meta = match runner_name {
        "codex" => parse_codex_args_and_env(args, &cwd),
        "copilot" => parse_copilot_args_and_env(args, &cwd),
        "opencode" => parse_opencode_args_and_env(args, &cwd),
        _ => parse_generic_args_and_env(runner_name, args, &cwd),
    };

    let runtime_id = format!("run_{}", Uuid::new_v4().simple());
    let execution_id = format!("exec_{}", Uuid::new_v4().simple());
    let session_id = format!("sess_{}", Uuid::new_v4().simple());

    let git_ctx_before = capture_git_context(&cwd);
    let full_command_line = format!("{} {}", command, args.join(" "));

    // 1. Prepare initial events
    let mut initial_events = Vec::new();

    // Event: runtime.started
    initial_events.push(IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: start_time.clone(),
        event_type: "runtime.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(runtime_id.clone()),
        session_id: Some(session_id.clone()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": runner_name,
            "runner_version": meta.runner_version,
            "surface": surface,
            "pid": std::process::id(),
            "hostname": "localhost",
            "os": std::env::consts::OS,
            "cwd": cwd.to_string_lossy(),
            "command_line": full_command_line,
        }),
        extra: std::collections::HashMap::new(),
    });

    // Event: session.identified
    initial_events.push(IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: start_time.clone(),
        event_type: "session.identified".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(runtime_id.clone()),
        session_id: Some(session_id.clone()),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "runner_name": runner_name,
            "native_session_id": meta.native_session_id,
            "title": meta.prompt_summary,
        }),
        extra: std::collections::HashMap::new(),
    });

    // Event: execution.started
    initial_events.push(IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: start_time.clone(),
        event_type: "execution.started".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(runtime_id.clone()),
        session_id: Some(session_id.clone()),
        execution_id: Some(execution_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "native_execution_id": "UNKNOWN",
            "turn_index": 0,
            "prompt_summary": meta.prompt_summary,
            "model": meta.model,
            "reasoning_effort": meta.reasoning_effort,
            "selected_agent_role": meta.selected_agent_role,
            "repo_root": git_ctx_before.as_ref().map(|g| &g.repo_root),
            "worktree_path": git_ctx_before.as_ref().map(|g| &g.worktree_path),
            "branch": git_ctx_before.as_ref().map(|g| &g.branch),
            "head_sha": git_ctx_before.as_ref().map(|g| &g.head_sha),
        }),
        extra: std::collections::HashMap::new(),
    });

    // Event: git.snapshot (BEFORE)
    if let Some(git) = &git_ctx_before {
        initial_events.push(IngestEvent {
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: start_time.clone(),
            event_type: "git.snapshot".to_string(),
            source: "git_observer".to_string(),
            runtime_id: Some(runtime_id.clone()),
            session_id: Some(session_id.clone()),
            execution_id: Some(execution_id.clone()),
            agent_instance_id: None,
            payload: serde_json::json!({
                "snapshot_type": "BEFORE",
                "repo_root": git.repo_root,
                "worktree_path": git.worktree_path,
                "branch": git.branch,
                "head_commit": git.head_sha,
                "is_dirty": git.is_dirty,
                "changed_files": git.changed_files,
                "diff_stat": git.diff_stat,
                "attribution": "OBSERVED",
            }),
            extra: std::collections::HashMap::new(),
        });
    }

    // Discovered components
    for comp in &meta.components {
        initial_events.push(IngestEvent {
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: start_time.clone(),
            event_type: "component.discovered".to_string(),
            source: "config_discovery".to_string(),
            runtime_id: Some(runtime_id.clone()),
            session_id: Some(session_id.clone()),
            execution_id: Some(execution_id.clone()),
            agent_instance_id: None,
            payload: serde_json::json!({
                "component_type": comp.component_type,
                "name": comp.name,
                "version": comp.version,
                "description": comp.description,
                "state": comp.state,
                "invocations_count": 0,
            }),
            extra: std::collections::HashMap::new(),
        });
    }

    // Config snapshots
    for (cfg_type, raw, parsed) in &meta.config_snapshots {
        initial_events.push(IngestEvent {
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: start_time.clone(),
            event_type: "config.snapshot".to_string(),
            source: "config_discovery".to_string(),
            runtime_id: Some(runtime_id.clone()),
            session_id: Some(session_id.clone()),
            execution_id: Some(execution_id.clone()),
            agent_instance_id: None,
            payload: serde_json::json!({
                "config_type": cfg_type,
                "raw_content": raw,
                "parsed_json": parsed,
            }),
            extra: std::collections::HashMap::new(),
        });
    }

    // Send pre-run telemetry asynchronously/non-blocking
    send_telemetry_events(server_url, &initial_events);

    // 2. Spawn child process with inherited stdio & transparent exit code / TTY
    let child_res = Command::new(&binary_path)
        .args(args)
        .current_dir(&cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    let exit_code = match child_res {
        Ok(mut child) => match child.wait() {
            Ok(status) => status.code().unwrap_or(if status.success() { 0 } else { 1 }),
            Err(e) => {
                eprintln!("Harnesscope: Error waiting for child process: {:?}", e);
                1
            }
        },
        Err(e) => {
            eprintln!("Harnesscope: Failed to spawn process {:?}: {:?}", binary_path, e);
            1
        }
    };

    let duration_ms = start_instant.elapsed().as_millis() as i64;
    let end_time = Utc::now().to_rfc3339();

    // 3. Post-run telemetry
    let git_ctx_after = capture_git_context(&cwd);
    let mut closing_events = Vec::new();

    if let Some(git) = &git_ctx_after {
        closing_events.push(IngestEvent {
            event_id: format!("evt_{}", Uuid::new_v4().simple()),
            timestamp: end_time.clone(),
            event_type: "git.snapshot".to_string(),
            source: "git_observer".to_string(),
            runtime_id: Some(runtime_id.clone()),
            session_id: Some(session_id.clone()),
            execution_id: Some(execution_id.clone()),
            agent_instance_id: None,
            payload: serde_json::json!({
                "snapshot_type": "AFTER",
                "repo_root": git.repo_root,
                "worktree_path": git.worktree_path,
                "branch": git.branch,
                "head_commit": git.head_sha,
                "is_dirty": git.is_dirty,
                "changed_files": git.changed_files,
                "diff_stat": git.diff_stat,
                "attribution": "OBSERVED",
            }),
            extra: std::collections::HashMap::new(),
        });
    }

    // Event: execution.completed
    closing_events.push(IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: end_time.clone(),
        event_type: "execution.completed".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(runtime_id.clone()),
        session_id: Some(session_id.clone()),
        execution_id: Some(execution_id.clone()),
        agent_instance_id: None,
        payload: serde_json::json!({
            "ended_at": end_time,
            "duration_ms": duration_ms,
            "status": if exit_code == 0 { "COMPLETED" } else { "FAILED" },
            "exit_code": exit_code,
        }),
        extra: std::collections::HashMap::new(),
    });

    // Event: runtime.stopped
    closing_events.push(IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: end_time,
        event_type: "runtime.stopped".to_string(),
        source: "wrapper".to_string(),
        runtime_id: Some(runtime_id),
        session_id: Some(session_id),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "exit_code": exit_code,
            "status": if exit_code == 0 { "COMPLETED" } else { "FAILED" },
        }),
        extra: std::collections::HashMap::new(),
    });

    send_telemetry_events(server_url, &closing_events);

    exit_code
}

pub fn execute_wrapper(runner_name: &str, args: &[String], server_url: &str) -> i32 {
    let surface = if runner_name == "opencode" { "tui" } else { "cli" };
    execute_wrapper_generic(runner_name, surface, runner_name, args, server_url)
}

