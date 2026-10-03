use super::Repository;
use crate::capture;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

impl Repository {
    pub fn register_workflow(&self, root: &Path) -> Result<(), String> {
        let root = root
            .canonicalize()
            .map_err(|_| "Workflow root does not exist")?;
        if !root.is_dir() {
            return Err("Workflow root must be a directory".into());
        }
        let path = root.to_str().ok_or("Workflow path must be Unicode")?;
        if capture::sensitive_path(path) {
            return Err("Credential directories cannot be registered".into());
        }
        self.db.with_conn(|c|c.execute("INSERT INTO workflow_roots(path) VALUES(?1) ON CONFLICT(path) DO UPDATE SET enabled=1",[path])).map_err(|_|"Cannot register workflow root")?;
        self.snapshot_workflow(&root)
    }
    pub fn scan_workflows(&self) -> Result<(), String> {
        let roots: Vec<String> = self
            .db
            .with_conn(|c| {
                c.prepare("SELECT path FROM workflow_roots WHERE enabled=1")?
                    .query_map([], |r| r.get(0))?
                    .collect()
            })
            .map_err(|_| "Cannot list workflow roots")?;
        let mut failed = false;
        for root in roots {
            if self.snapshot_workflow(Path::new(&root)).is_err() {
                failed = true;
            }
        }
        if failed {
            Err("Some registered workflow roots could not be captured".into())
        } else {
            Ok(())
        }
    }
    fn snapshot_workflow(&self, root: &Path) -> Result<(), String> {
        if root
            .canonicalize()
            .map_err(|_| "Workflow root unavailable")?
            != root
        {
            return Err("Workflow root changed location".into());
        }
        let mut dirs = vec![root.to_path_buf()];
        let mut visited = 0;
        let mut seen = std::collections::BTreeSet::new();
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).map_err(|_| "Cannot list workflow files")? {
                let entry = entry.map_err(|_| "Cannot inspect workflow entry")?;
                visited += 1;
                if visited > 10_000 {
                    return Err("Workflow root exceeds scan limit".into());
                }
                let path = entry.path();
                let ty = entry
                    .file_type()
                    .map_err(|_| "Cannot inspect workflow entry")?;
                if ty.is_symlink() {
                    continue;
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if entry
                        .metadata()
                        .map_err(|_| "Cannot inspect workflow entry")?
                        .file_attributes()
                        & 0x400
                        != 0
                    {
                        continue;
                    }
                }
                let rel = path
                    .strip_prefix(root)
                    .map_err(|_| "Workflow path escaped root")?
                    .to_string_lossy()
                    .replace('\\', "/");
                if capture::sensitive_path(&rel) {
                    continue;
                }
                if ty.is_dir() {
                    if [
                        ".agents",
                        ".codex",
                        ".github",
                        ".cursor",
                        ".claude",
                        "skills",
                        "instructions",
                        "rules",
                    ]
                    .contains(&rel.as_str())
                        || [
                            ".agents/skills/",
                            ".codex/skills/",
                            "skills/",
                            ".github/instructions/",
                            ".cursor/rules/",
                        ]
                        .iter()
                        .any(|p| rel.starts_with(p))
                        || [
                            ".agents/skills",
                            ".codex/skills",
                            ".github/instructions",
                            ".cursor/rules",
                        ]
                        .contains(&rel.as_str())
                    {
                        dirs.push(path);
                    }
                    continue;
                }
                if !ty.is_file() {
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy();
                let relevant = [
                    "AGENTS.md",
                    "CLAUDE.md",
                    "GEMINI.md",
                    "WORKFLOW.md",
                    "SKILL.md",
                    "copilot-instructions.md",
                ]
                .contains(&name.as_ref())
                    || (rel.ends_with(".md")
                        && [
                            "skills/",
                            ".agents/skills/",
                            ".codex/skills/",
                            ".github/instructions/",
                            ".cursor/rules/",
                        ]
                        .iter()
                        .any(|p| rel.starts_with(p)))
                    || [".codex/config.toml", "config.toml", "package.json"]
                        .contains(&rel.as_str());
                if !relevant {
                    continue;
                }
                seen.insert(rel.clone());
                let canonical = path
                    .canonicalize()
                    .map_err(|_| "Cannot resolve workflow file")?;
                if !canonical.starts_with(root) {
                    return Err("Workflow file escaped root".into());
                }
                let meta = std::fs::metadata(&path).map_err(|_| "Workflow file unavailable")?;
                let safe = if meta.len() > 2 * 1024 * 1024 {
                    capture::SafeContent::excluded("workflow_object_limit")
                } else {
                    let text =
                        std::fs::read_to_string(&path).map_err(|_| "Cannot read workflow file")?;
                    let value = if name == "package.json" {
                        serde_json::from_str::<Value>(&text)
                            .ok()
                            .map(|v| json!({"scripts":v["scripts"]}))
                    } else if name == "config.toml" {
                        toml::from_str::<toml::Value>(&text)
                            .ok()
                            .and_then(|v| serde_json::to_value(v).ok())
                    } else {
                        Some(json!(text))
                    };
                    value.map(|v| capture::sanitize(&v)).unwrap_or_else(|| {
                        capture::SafeContent::excluded("unparseable_workflow_config")
                    })
                };
                self.transaction(||{
                    let hash=self.store_object(&safe)?;
                    let root=capture::project_path(&root.to_string_lossy());
                    let latest:Option<(Option<String>,String)>=self.db.with_conn(|c|c.query_row("SELECT object_hash,disposition FROM workflow_versions WHERE root=?1 AND path=?2 ORDER BY observed_at DESC,rowid DESC LIMIT 1",params![root,rel],|r|Ok((r.get(0)?,r.get(1)?))).optional())?;
                    if latest.as_ref().is_some_and(|v|v.0==hash && v.1==safe.disposition){return Ok(())}
                    self.db.with_conn(|c|{c.execute("INSERT INTO workflow_versions(id,root,path,observed_at,object_hash,disposition,reason) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![uuid::Uuid::new_v4().to_string(),root,capture::metadata(&rel),chrono::Utc::now().to_rfc3339(),hash,safe.disposition,safe.reason])?;Ok(())})
                }).map_err(|_|"Cannot save workflow snapshot")?;
            }
        }
        let root_text = capture::project_path(&root.to_string_lossy());
        let previous:Vec<String>=self.db.with_conn(|c|c.prepare("SELECT path FROM workflow_versions v WHERE root=?1 AND rowid=(SELECT MAX(rowid) FROM workflow_versions latest WHERE latest.root=v.root AND latest.path=v.path) AND COALESCE(reason,'')!='workflow_missing_or_disallowed'")?.query_map([root_text.as_str()],|r|r.get(0))?.collect()).map_err(|_|"Cannot inspect prior workflow versions")?;
        for path in previous {
            if !seen.contains(&path) {
                self.db.with_conn(|c|c.execute("INSERT INTO workflow_versions(id,root,path,observed_at,disposition,reason) VALUES(?1,?2,?3,?4,'excluded','workflow_missing_or_disallowed')",params![uuid::Uuid::new_v4().to_string(),root_text,path,chrono::Utc::now().to_rfc3339()])).map_err(|_|"Cannot record missing workflow file")?;
            }
        }
        Ok(())
    }
    pub fn workflow_versions(&self, root: Option<&str>, after: i64) -> rusqlite::Result<Value> {
        let root = root.map(capture::project_path);
        self.db.with_conn(|c|{
            let items:Vec<Value>=c.prepare("SELECT rowid,id,root,path,observed_at,object_hash,disposition,reason,state FROM workflow_versions WHERE rowid>?1 AND (?2 IS NULL OR root=?2) ORDER BY rowid LIMIT 100")?.query_map(params![after,root],|r|Ok(json!({"cursor":r.get::<_,i64>(0)?,"id":r.get::<_,String>(1)?,"root":r.get::<_,String>(2)?,"path":r.get::<_,String>(3)?,"observed_at":r.get::<_,String>(4)?,"object_hash":r.get::<_,Option<String>>(5)?,"disposition":r.get::<_,String>(6)?,"reason":r.get::<_,Option<String>>(7)?,"state":r.get::<_,String>(8)?})))?.collect::<rusqlite::Result<_>>()?;
            Ok(json!({"items":items,"schema_version":1,"historical_claim":"observed_at_only"}))
        })
    }
}
