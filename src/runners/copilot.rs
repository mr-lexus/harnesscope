use serde_json::Value;
use std::path::{Path, PathBuf};

use super::codex::RunnerMetadata;

pub fn parse_copilot_args_and_env(args: &[String], cwd: &Path) -> RunnerMetadata {
    let mut meta = RunnerMetadata {
        runner_name: "copilot".to_string(),
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

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--model" && i + 1 < args.len() {
            meta.model = args[i + 1].clone();
            i += 2;
            continue;
        } else if arg.starts_with("--model=") {
            meta.model = arg.trim_start_matches("--model=").to_string();
            i += 1;
            continue;
        } else if arg == "--agent" && i + 1 < args.len() {
            meta.selected_agent_role = args[i + 1].clone();
            i += 2;
            continue;
        } else if arg == "--resume" && i + 1 < args.len() {
            if uuid::Uuid::parse_str(&args[i + 1]).is_ok() {
                meta.native_session_id = args[i + 1].clone();
            }
            i += 2;
            continue;
        } else if let Some(id) = arg.strip_prefix("--resume=") {
            if uuid::Uuid::parse_str(id).is_ok() {
                meta.native_session_id = id.into();
            }
        } else if matches!(arg.as_str(), "--prompt" | "-p" | "-i") && i + 1 < args.len() {
            meta.prompt_summary = Some(args[i + 1].clone());
            i += 2;
            continue;
        } else if let Some(prompt) = arg.strip_prefix("--prompt=") {
            meta.prompt_summary = Some(prompt.into());
        }
        i += 1;
    }

    // Copilot config discovery
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let config_candidates = vec![
        PathBuf::from(&home)
            .join(".config")
            .join("github-copilot")
            .join("config.json"),
        PathBuf::from(&home).join(".copilot").join("config.json"),
        cwd.join(".copilot").join("config.json"),
    ];

    for path in config_candidates {
        if path.metadata().is_ok_and(|m| m.len() <= 1024 * 1024) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let parsed: Option<Value> = serde_json::from_str(&content).ok();
                meta.config_snapshots
                    .push(("copilot".to_string(), content, parsed));
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_copilot_args() {
        let args = vec![
            "--model".to_string(),
            "claude-3-5-sonnet".to_string(),
            "--agent".to_string(),
            "test-gen".to_string(),
            "-p".to_string(),
            "Generate unit tests".to_string(),
        ];
        let meta = parse_copilot_args_and_env(&args, Path::new("."));
        assert_eq!(meta.model, "claude-3-5-sonnet");
        assert_eq!(meta.selected_agent_role, "test-gen");
        assert_eq!(meta.prompt_summary, Some("Generate unit tests".to_string()));
    }
}
