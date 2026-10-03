//! Reversible additions only. Existing telemetry routes are never overwritten.
use serde_json::{json, Value};
use std::{io::Write, path::Path};

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Configuration path has no parent")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cannot create configuration")?;
    file.write_all(bytes)
        .map_err(|_| "Cannot write configuration")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "Cannot sync configuration")?;
    file.persist(path)
        .map_err(|_| "Cannot publish configuration")?;
    Ok(())
}
pub fn configure(home: &Path, executable: &Path, port: u16, remove: bool) -> Result<Value, String> {
    if !home.is_absolute() {
        return Err("Codex home must be absolute".into());
    }
    std::fs::create_dir_all(home).map_err(|_| "Cannot create Codex home")?;
    let hook_path = home.join("hooks.json");
    let config_path = home.join("config.toml");
    for path in [&hook_path, &config_path] {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Refusing to replace a linked configuration file".into());
        }
    }
    let mut hooks: Value = if hook_path.exists() {
        serde_json::from_slice(&std::fs::read(&hook_path).map_err(|_| "Cannot read hooks")?)
            .map_err(|_| "Invalid existing hooks JSON; unchanged")?
    } else {
        json!({"hooks":{}})
    };
    let old = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(|_| "Cannot read Codex configuration")?
    } else {
        String::new()
    };
    let parsed: toml::Value =
        toml::from_str(&old).map_err(|_| "Invalid existing TOML; unchanged")?;
    let own =
        crate::capture_io::hook_configuration(executable, &crate::config::Config::load().db_path)?;
    let map = hooks
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or("Unsupported hook configuration shape; unchanged")?;
    // Validate all modified arrays before publishing anything.
    for event in own["hooks"].as_object().unwrap().keys() {
        if map.get(event).is_some_and(|v| !v.is_array()) {
            return Err("Unsupported hook matcher array; unchanged".into());
        }
    }
    for (event, groups) in own["hooks"].as_object().unwrap() {
        let entries = map
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap();
        let handler = &groups[0];
        if remove {
            entries.retain(|v| v != handler);
        } else if !entries.contains(handler) {
            entries.push(handler.clone());
        }
    }
    let block=format!("\n# BEGIN HARNESSCOPE LOCAL TELEMETRY\n[otel]\nenvironment = \"harnesscope-local\"\nlog_user_prompt = true\nexporter = {{ otlp-http = {{ endpoint = \"http://127.0.0.1:{port}/v1/logs\", protocol = \"json\" }} }}\ntrace_exporter = {{ otlp-http = {{ endpoint = \"http://127.0.0.1:{port}/v1/traces\", protocol = \"json\" }} }}\n# END HARNESSCOPE LOCAL TELEMETRY\n");
    let (new, otel) = if remove {
        if old.contains(&block) {
            (old.replace(&block, ""), "removed_own_block")
        } else {
            (old.clone(), "unchanged")
        }
    } else if parsed.get("otel").is_none() {
        (format!("{old}{block}"), "configured_local_json")
    } else {
        (old.clone(), "existing_otel_preserved")
    };
    write(
        &hook_path,
        &serde_json::to_vec_pretty(&hooks).map_err(|_| "Cannot encode hooks")?,
    )?;
    if new != old {
        write(&config_path, new.as_bytes())?;
    }
    Ok(
        json!({"hooks":if remove{"own_exact_handlers_removed"}else{"configured_review_and_trust_in_codex"},"otel":otel,"hook_trust":"not_modified","codex_home":home}),
    )
}
