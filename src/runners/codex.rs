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
        native_session_id: std::env::var("HARNESSCOPE_SESSION_ID").unwrap_or_else(|_| "UNKNOWN".to_string()),
        parent_session_id: std::env::var("HARNESSCOPE_PARENT_SESSION_ID").ok(),
        fork_reason: None,
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
        } else if arg.starts_with("--model=") {
            meta.model = arg.trim_start_matches("--model=").to_string();
            i += 1;
            continue;
        } else if (arg == "--reasoning-effort" || arg == "--effort") && i + 1 < args.len() {
            meta.reasoning_effort = args[i + 1].clone();
            i += 2;
            continue;
        } else if arg.starts_with("--reasoning-effort=") {
            meta.reasoning_effort = arg.trim_start_matches("--reasoning-effort=").to_string();
            i += 1;
            continue;
        } else if (arg == "--session" || arg == "--thread" || arg == "-s") && i + 1 < args.len() {
            meta.native_session_id = args[i + 1].clone();
            i += 2;
            continue;
        } else if arg.starts_with("--session=") {
            meta.native_session_id = arg.trim_start_matches("--session=").to_string();
            i += 1;
            continue;
        } else if (arg == "--parent-session" || arg == "--fork-from" || arg == "--checkpoint") && i + 1 < args.len() {
            meta.parent_session_id = Some(args[i + 1].clone());
            meta.fork_reason = Some("FORK_ARG".to_string());
            i += 2;
            continue;
        } else if (arg == "--role" || arg == "--agent") && i + 1 < args.len() {
            meta.selected_agent_role = args[i + 1].clone();
            i += 2;
            continue;
        } else if !arg.starts_with('-') && meta.prompt_summary.is_none() {
            meta.prompt_summary = Some(arg.clone());
        }
        i += 1;
    }

    // Best-effort config discovery
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    let config_candidates = vec![
        PathBuf::from(&home).join(".codex").join("config.json"),
        cwd.join(".codex").join("config.json"),
    ];

    for path in config_candidates {
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let parsed: Option<Value> = serde_json::from_str(&content).ok();
                if let Some(json) = &parsed {
                    if let Some(mcp_servers) = json.get("mcpServers").and_then(|v| v.as_object()) {
                        for (k, _) in mcp_servers {
                            meta.components.push(DiscoveredComponent {
                                component_type: "MCP".to_string(),
                                name: k.clone(),
                                version: None,
                                description: None,
                                state: "CONFIGURED".to_string(),
                            });
                        }
                    }
                    if meta.model == "UNKNOWN" {
                        if let Some(m) = json.get("model").and_then(|v| v.as_str()) {
                            meta.model = m.to_string();
                        }
                    }
                }
                meta.config_snapshots.push(("codex".to_string(), content, parsed));
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_codex_args() {
        let args = vec![
            "--model".to_string(),
            "o3-mini".to_string(),
            "--reasoning-effort".to_string(),
            "high".to_string(),
            "--session".to_string(),
            "thread-12345".to_string(),
            "Refactor auth module".to_string(),
        ];
        let meta = parse_codex_args_and_env(&args, Path::new("."));
        assert_eq!(meta.model, "o3-mini");
        assert_eq!(meta.reasoning_effort, "high");
        assert_eq!(meta.native_session_id, "thread-12345");
        assert_eq!(meta.prompt_summary, Some("Refactor auth module".to_string()));
    }
}
