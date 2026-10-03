use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct DiscoveredComponent {
    pub component_type: String, // 'MCP', 'SKILL', 'PLUGIN', 'TOOL'
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub state: String, // 'CONFIGURED', 'DISCOVERED'
}

#[derive(Debug, Clone, Default)]
pub struct RunnerMetadata {
    pub runner_name: String,
    pub runner_version: String,
    pub native_session_id: String,
    pub parent_session_id: Option<String>,
    pub fork_reason: Option<String>,
    pub model: String,
    pub reasoning_effort: String,
    pub selected_agent_role: String,
    pub prompt_summary: Option<String>,
    pub components: Vec<DiscoveredComponent>,
    pub config_snapshots: Vec<(String, String, Option<Value>)>, // (config_type, raw_content, parsed_json)
}

pub fn parse_codex_args_and_env(args: &[String], cwd: &Path) -> RunnerMetadata {
    let mut meta = RunnerMetadata {
        runner_name: "codex".to_string(),
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

    parse_args(args, &mut meta);

    // Discover declared components, not their use or the effective model. Project
    // trust, profiles and managed settings can change which configuration applies.
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home).join(".codex"));
    for path in [
        codex_home.join("config.toml"),
        cwd.join(".codex/config.toml"),
    ] {
        if path.metadata().is_ok_and(|m| m.len() <= 1024 * 1024) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = content.parse::<toml::Value>() {
                    discover_config(&config, &mut meta);
                    meta.config_snapshots.push((
                        "codex".into(),
                        content,
                        serde_json::to_value(config).ok(),
                    ));
                }
            }
        }
    }
    meta.components.sort_by(|a, b| a.name.cmp(&b.name));
    meta.components.dedup_by(|a, b| a.name == b.name);

    meta
}

fn parse_args(args: &[String], meta: &mut RunnerMetadata) {
    let mut i = 0;
    let mut mode = "prompt";
    while i < args.len() {
        let arg = args[i].as_str();
        if matches!(arg, "--model" | "-m") {
            if let Some(value) = args.get(i + 1) {
                meta.model = value.clone();
            }
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--model=") {
            meta.model = value.into();
            i += 1;
            continue;
        }
        if matches!(arg, "--config" | "-c") {
            if let Some(value) = args.get(i + 1) {
                parse_override(value, meta);
            }
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--config=") {
            parse_override(value, meta);
            i += 1;
            continue;
        }
        if matches!(
            arg,
            "--sandbox"
                | "-s"
                | "--ask-for-approval"
                | "-a"
                | "--profile"
                | "-p"
                | "--cd"
                | "-C"
                | "--image"
                | "-i"
                | "--output-last-message"
                | "-o"
                | "--output-schema"
                | "--add-dir"
                | "--enable"
                | "--disable"
        ) {
            i += 2;
            continue;
        }
        if arg == "--" {
            if let Some(prompt) = args.get(i + 1) {
                meta.prompt_summary = Some(prompt.clone());
            }
            break;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        match arg {
            "exec" | "e" if mode == "prompt" && meta.prompt_summary.is_none() => {}
            "resume" | "fork" if meta.prompt_summary.is_none() => {
                mode = arg;
            }
            _ if mode == "resume" => {
                // Names are not stable identity; only UUIDs identify a native session.
                if uuid::Uuid::parse_str(arg).is_ok() {
                    meta.native_session_id = arg.into();
                }
                mode = "prompt";
            }
            _ if mode == "fork" => {
                if uuid::Uuid::parse_str(arg).is_ok() {
                    meta.parent_session_id = Some(arg.into());
                    meta.fork_reason = Some("FORK_ARG".into());
                }
                mode = "prompt";
            }
            _ => {
                meta.prompt_summary = Some(arg.into());
                break;
            }
        }
        i += 1;
    }
}

fn parse_override(value: &str, meta: &mut RunnerMetadata) {
    if let Ok(config) = value.parse::<toml::Value>() {
        if let Some(value) = config.get("model").and_then(toml::Value::as_str) {
            meta.model = value.into();
        }
        if let Some(value) = config
            .get("model_reasoning_effort")
            .and_then(toml::Value::as_str)
        {
            meta.reasoning_effort = value.into();
        }
    }
}

fn discover_config(config: &toml::Value, meta: &mut RunnerMetadata) {
    if let Some(servers) = config.get("mcp_servers").and_then(toml::Value::as_table) {
        for (name, server) in servers {
            if server.get("enabled").and_then(toml::Value::as_bool) == Some(false) {
                continue;
            }
            meta.components.push(DiscoveredComponent {
                component_type: "MCP".into(),
                name: name.clone(),
                state: "CONFIGURED".into(),
                ..Default::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> RunnerMetadata {
        let mut meta = RunnerMetadata {
            native_session_id: "UNKNOWN".into(),
            model: "UNKNOWN".into(),
            ..Default::default()
        };
        parse_args(
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &mut meta,
        );
        meta
    }
    #[test]
    fn sandbox_is_not_session_and_resume_is_not_prompt() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let meta = parse(&[
            "-s",
            "workspace-write",
            "resume",
            id,
            "-m",
            "example-model",
            "Fix tests",
        ]);
        assert_eq!(meta.native_session_id, id);
        assert_eq!(meta.prompt_summary.as_deref(), Some("Fix tests"));
        assert_eq!(meta.model, "example-model");
        assert_eq!(parse(&["resume", "--last"]).native_session_id, "UNKNOWN");
        assert_eq!(parse(&["fork", id]).parent_session_id.as_deref(), Some(id));
        assert_eq!(parse(&["fork", id]).native_session_id, "UNKNOWN");
    }
    #[test]
    fn toml_override_and_components_are_observed_without_claiming_invocation() {
        let mut meta = parse(&[
            "exec",
            "-c",
            "model_reasoning_effort=\"high\"",
            "-m",
            "example",
            "task",
        ]);
        assert_eq!(meta.reasoning_effort, "high");
        let config =
            "[mcp_servers.context7]\ncommand='node'\n[mcp_servers.disabled]\nenabled=false"
                .parse()
                .unwrap();
        discover_config(&config, &mut meta);
        assert_eq!(meta.components.len(), 1);
        assert_eq!(meta.components[0].state, "CONFIGURED");
    }
}
