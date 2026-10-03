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

pub fn spawn_background_server(exe_path: &Path, port: u16) -> std::io::Result<()> {
    spawn_background_server_at(exe_path, "127.0.0.1", port)
}

#[cfg(windows)]
fn spawn_background_server_at(exe_path: &Path, host: &str, port: u16) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{CreateProcessW, CREATE_NO_WINDOW, PROCESS_INFORMATION, STARTUPINFOW},
    };
    let exe: Vec<u16> = exe_path.as_os_str().encode_wide().chain(Some(0)).collect();
    // argv[0] is only a label: lpApplicationName is the exact executable path.
    let mut command: Vec<u16> = format!("harnesscope serve --host {host} --port {port}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: zero is a valid initial representation for these Windows structs;
    // all buffers are NUL terminated, live for the call, and command is mutable.
    // Handle inheritance MUST be false: Command::spawn otherwise leaks the
    // caller's output pipes into this long-lived server, blocking captured CLI output.
    unsafe {
        let mut startup: STARTUPINFOW = std::mem::zeroed();
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut process: PROCESS_INFORMATION = std::mem::zeroed();
        if CreateProcessW(
            exe.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut process,
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    Ok(())
}

#[cfg(not(windows))]
fn spawn_background_server_at(exe_path: &Path, host: &str, port: u16) -> std::io::Result<()> {
    Command::new(exe_path)
        .args(["serve", "--host", host, "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn ensure_server_running(server_url: &str) {
    if std::env::var("HARNESSCOPE_AUTOSTART").as_deref() == Ok("0") {
        return;
    }
    let Ok(url) = crate::config::local_server_url(server_url) else {
        return;
    };
    let client = match reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(std::time::Duration::from_millis(200))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let health_url = format!("{}/api/v1/health", server_url.trim_end_matches('/'));
    // Fast path: server is already running
    if client
        .get(&health_url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        return;
    }

    // Inter-process startup lock ensures multiple wrappers starting simultaneously spawn only 1 server
    let port = url.port_or_known_default().unwrap_or(4242);
    let config = crate::config::Config::load();
    if std::fs::create_dir_all(&config.data_dir).is_err() {
        return;
    }
    let lock_path = config.data_dir.join(format!("server-startup-{port}.lock"));
    let mut got_lock = false;

    for _ in 0..3 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => {
                got_lock = true;
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // If lock file is older than 8 seconds, consider it stale from a dead process
                if let Ok(metadata) = std::fs::metadata(&lock_path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = modified.elapsed() {
                            if elapsed > std::time::Duration::from_secs(8) {
                                let _ = std::fs::remove_file(&lock_path);
                                continue;
                            }
                        }
                    }
                }
                break;
            }
            Err(_) => break,
        }
    }

    if got_lock {
        // Double check health in case another process just started it
        if !client
            .get(&health_url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            if let Ok(exe) = std::env::current_exe() {
                let host = url
                    .host_str()
                    .unwrap_or("127.0.0.1")
                    .trim_matches(['[', ']']);
                let _ = spawn_background_server_at(&exe, host, port);
            }
        }
    }

    // Wait for server to become responsive
    let startup_deadline = Instant::now() + std::time::Duration::from_secs(2);
    for _ in 0..30 {
        if Instant::now() >= startup_deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        if client
            .get(&health_url)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
        {
            break;
        }
    }

    if got_lock {
        let _ = std::fs::remove_file(&lock_path);
    }
}

fn queue_events(outbox: Option<&crate::outbox::Outbox>, stream: &str, events: &[IngestEvent]) {
    if let Some(outbox) = outbox {
        if let Err(error) = outbox.enqueue(stream, events) {
            eprintln!(
                "Harnesscope: telemetry could not be queued: {error}. Agent execution continues."
            );
        }
    }
}

fn heartbeat(runtime_id: &str) -> IngestEvent {
    IngestEvent {
        event_id: format!("evt_{}", Uuid::new_v4().simple()),
        timestamp: Utc::now().to_rfc3339(),
        event_type: "runtime.heartbeat".into(),
        source: "wrapper".into(),
        runtime_id: Some(runtime_id.into()),
        session_id: None,
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({"child_running":true}),
        extra: Default::default(),
    }
}

fn parse_generic_args_and_env(runner_name: &str, args: &[String], _cwd: &Path) -> RunnerMetadata {
    let mut meta = RunnerMetadata {
        runner_name: runner_name.to_string(),
        runner_version: "UNKNOWN".to_string(),
        native_session_id: std::env::var("HARNESSCOPE_SESSION_ID")
            .unwrap_or_else(|_| "UNKNOWN".to_string()),
        parent_session_id: std::env::var("HARNESSCOPE_PARENT_SESSION_ID").ok(),
        fork_reason: None,
        model: "UNKNOWN".to_string(),
        reasoning_effort: "UNKNOWN".to_string(),
        selected_agent_role: "UNKNOWN".to_string(),
        prompt_summary: None,
        components: Vec::new(),
        config_snapshots: Vec::new(),
    };

    // Arbitrary agents assign different meanings to -s, -m and -a. Only explicit
    // Harnesscope metadata is portable; never infer it from an unknown CLI.
    let _ = args;
    meta.model = std::env::var("HARNESSCOPE_MODEL").unwrap_or_else(|_| "UNKNOWN".into());

    meta
}

pub fn execute_wrapper_generic(
    runner_name: &str,
    surface: &str,
    command: &str,
    args: &[String],
    server_url: &str,
) -> i32 {
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
    let full_command_line = format!(
        "{} {}",
        command,
        crate::redact::redact_command_args(args).join(" ")
    );

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
            "pid": null,
            "wrapper_pid": std::process::id(),
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
            "parent_session_id": meta.parent_session_id,
            "fork_reason": meta.fork_reason,
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
            "capture_scope": "PROCESS",
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
    for (cfg_type, raw, parsed) in meta
        .config_snapshots
        .iter()
        .filter(|_| std::env::var("HARNESSCOPE_CAPTURE_CONFIG").as_deref() == Ok("1"))
    {
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

    // 2. Spawn child process with inherited stdio & transparent exit code / TTY
    #[cfg(windows)]
    let mut child_command = if binary_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
    {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-File"]).arg(&binary_path);
        command
    } else {
        Command::new(&binary_path)
    };
    #[cfg(not(windows))]
    let mut child_command = Command::new(&binary_path);
    let child_res = child_command
        .args(args)
        .current_dir(&cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    if let Ok(child) = &child_res {
        initial_events[0].payload["pid"] = serde_json::json!(child.id());
    }
    let outbox =
        match crate::outbox::Outbox::open(&crate::config::Config::load().db_path, server_url) {
            Ok(q) => Some(q),
            Err(error) => {
                eprintln!(
                "Harnesscope: durable telemetry unavailable: {error}. Agent execution continues."
            );
                None
            }
        };
    if child_res.is_ok() {
        initial_events.push(heartbeat(&runtime_id));
    }
    queue_events(outbox.as_ref(), &runtime_id, &initial_events);
    // Network I/O and server startup never hold up the child's inherited terminal.
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let worker_queue = outbox.clone().map(|q| q.for_stream(&runtime_id));
    let destination = server_url.to_string();
    let worker = std::thread::spawn(move || {
        ensure_server_running(&destination);
        if let Some(q) = worker_queue {
            loop {
                for _ in 0..10 {
                    match q.deliver_http() {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(error) => {
                            eprintln!("Harnesscope: delivery worker: {error}");
                            break;
                        }
                    }
                }
                match stop_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        // Closing events were committed before this signal.
                        for _ in 0..10 {
                            if !q.deliver_http().unwrap_or(false) {
                                break;
                            }
                        }
                        break;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        }
    });
    let exit_code = match child_res {
        Ok(mut child) => {
            let mut last_heartbeat = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => break child_exit_code(status),
                    Ok(None) => {
                        if last_heartbeat.elapsed() >= std::time::Duration::from_secs(15) {
                            queue_events(outbox.as_ref(), &runtime_id, &[heartbeat(&runtime_id)]);
                            last_heartbeat = Instant::now();
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(error) => {
                        eprintln!("Harnesscope: Error waiting for child: {error}");
                        break 1;
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("Harnesscope: Failed to spawn {binary_path:?}: {error}");
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
        runtime_id: Some(runtime_id.clone()),
        session_id: Some(session_id),
        execution_id: None,
        agent_instance_id: None,
        payload: serde_json::json!({
            "exit_code": exit_code,
            "status": if exit_code == 0 { "COMPLETED" } else { "FAILED" },
        }),
        extra: std::collections::HashMap::new(),
    });

    queue_events(outbox.as_ref(), &runtime_id, &closing_events);
    let _ = stop_tx.send(());
    let _ = worker.join();
    if let Some(q) = outbox {
        if q.has_pending_stream(&runtime_id).unwrap_or(false) {
            eprintln!(
                "Harnesscope: pending telemetry saved on disk; delivery resumes with the server."
            );
        }
    }

    exit_code
}

pub fn execute_wrapper(runner_name: &str, args: &[String], server_url: &str) -> i32 {
    let surface = if runner_name == "opencode" {
        "tui"
    } else {
        "cli"
    };
    execute_wrapper_generic(runner_name, surface, runner_name, args, server_url)
}

fn child_exit_code(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
    }
    #[cfg(not(unix))]
    {
        status
            .code()
            .unwrap_or(if status.success() { 0 } else { 1 })
    }
}
