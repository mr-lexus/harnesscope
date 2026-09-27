use serde_json::Value;
use std::path::{Path, PathBuf};

use super::codex::{DiscoveredComponent, RunnerMetadata};

pub fn parse_opencode_args_and_env(args: &[String], cwd: &Path) -> RunnerMetadata {
    let mut meta = RunnerMetadata {
        runner_name: "opencode".to_string(),
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
        if (arg == "--session" || arg == "--session-id" || arg == "-s") && i + 1 < args.len() {
            meta.native_session_id = args[i + 1].clone();
            i += 2;
            continue;
        } else if arg.starts_with("--session=") {
            meta.native_session_id = arg.trim_start_matches("--session=").to_string();
            i += 1;
            continue;
        } else if (arg == "--agent" || arg == "-a") && i + 1 < args.len() {
            meta.selected_agent_role = args[i + 1].clone();
            i += 2;
            continue;
        } else if (arg == "--model" || arg == "-m") && i + 1 < args.len() {
            meta.model = args[i + 1].clone();
            i += 2;
            continue;
        } else if !arg.starts_with('-') && meta.prompt_summary.is_none() {
            meta.prompt_summary = Some(arg.clone());
        }
        i += 1;
    }

    // Config discovery
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    let config_candidates = vec![
        PathBuf::from(&home).join(".config").join("opencode").join("opencode.json"),
        PathBuf::from(&home).join(".opencode").join("opencode.json"),
        cwd.join(".opencode").join("opencode.json"),
        cwd.join("opencode.json"),
        // Oh My OpenCode (OMO)
        PathBuf::from(&home).join(".omo").join("config.json"),
        cwd.join(".omo").join("config.json"),
        cwd.join("oh-my-opencode.json"),
    ];

    for path in config_candidates {
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let parsed: Option<Value> = serde_json::from_str(&content).ok();
                if let Some(json) = &parsed {
                    // Extract MCP servers
                    if let Some(mcps) = json.get("mcpServers").or_else(|| json.get("mcp")).and_then(|v| v.as_object()) {
                        for (name, _) in mcps {
                            meta.components.push(DiscoveredComponent {
                                component_type: "MCP".to_string(),
                                name: name.clone(),
                                version: None,
                                description: None,
                                state: "CONFIGURED".to_string(),
                            });
                        }
                    }
                    // Extract skills
                    if let Some(skills) = json.get("skills").and_then(|v| v.as_array()) {
                        for s in skills {
                            if let Some(name) = s.as_str().or_else(|| s.get("name").and_then(|v| v.as_str())) {
                                meta.components.push(DiscoveredComponent {
                                component_type: "SKILL".to_string(),
                                name: name.to_string(),
                                version: None,
                                description: None,
                                state: "CONFIGURED".to_string(),
                            });
                            }
                        }
                    }
                    // Extract plugins
                    if let Some(plugins) = json.get("plugins").and_then(|v| v.as_array()) {
                        for p in plugins {
                            if let Some(name) = p.as_str().or_else(|| p.get("name").and_then(|v| v.as_str())) {
                                meta.components.push(DiscoveredComponent {
                                    component_type: "PLUGIN".to_string(),
                                    name: name.to_string(),
                                    version: None,
                                    description: None,
                                    state: "CONFIGURED".to_string(),
                                });
                            }
                        }
                    }
                    // Model
                    if meta.model == "UNKNOWN" {
                        if let Some(m) = json.get("model").and_then(|v| v.as_str()) {
                            meta.model = m.to_string();
                        }
                    }
                }
                let config_type = if path.to_string_lossy().contains("omo") { "omo" } else { "opencode" };
                meta.config_snapshots.push((config_type.to_string(), content, parsed));
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opencode_args() {
        let args = vec![
            "--session".to_string(),
            "sess-9941".to_string(),
            "--agent".to_string(),
            "architect".to_string(),
            "--model".to_string(),
            "claude-3-5-sonnet".to_string(),
            "Plan new microservice".to_string(),
        ];
        let meta = parse_opencode_args_and_env(&args, Path::new("."));
        assert_eq!(meta.native_session_id, "sess-9941");
        assert_eq!(meta.selected_agent_role, "architect");
        assert_eq!(meta.model, "claude-3-5-sonnet");
        assert_eq!(meta.prompt_summary, Some("Plan new microservice".to_string()));
    }
}
