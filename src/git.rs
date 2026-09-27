use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangedFile {
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContext {
    pub repo_root: String,
    pub worktree_path: String,
    pub cwd: String,
    pub branch: String,
    pub head_sha: String,
    pub is_dirty: bool,
    pub changed_files: Vec<ChangedFile>,
    pub diff_stat: Option<String>,
}

fn run_git_cmd(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(text)
    } else {
        None
    }
}

pub fn is_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn capture_git_context(cwd: &Path) -> Option<GitContext> {
    // Check if inside work tree
    let is_inside = run_git_cmd(cwd, &["rev-parse", "--is-inside-work-tree"])?;
    if is_inside != "true" {
        return None;
    }

    let repo_root = run_git_cmd(cwd, &["rev-parse", "--show-toplevel"])
        .unwrap_or_else(|| "UNKNOWN".to_string());

    // Worktree path: check if rev-parse --show-superproject-working-tree exists or fallback to toplevel
    let worktree_path = run_git_cmd(cwd, &["rev-parse", "--show-toplevel"])
        .unwrap_or_else(|| repo_root.clone());

    let branch = run_git_cmd(cwd, &["branch", "--show-current"])
        .and_then(|b| if b.is_empty() { None } else { Some(b) })
        .or_else(|| run_git_cmd(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "HEAD (detached)".to_string());

    let head_sha = run_git_cmd(cwd, &["rev-parse", "HEAD"])
        .unwrap_or_else(|| "UNKNOWN".to_string());

    let porcelain = run_git_cmd(cwd, &["status", "--porcelain"])
        .unwrap_or_default();

    let mut changed_files = Vec::new();
    for line in porcelain.lines() {
        let line = line.trim_end();
        if line.len() >= 3 {
            let status = line[..2].trim().to_string();
            let file_path = line[3..].trim().to_string();
            changed_files.push(ChangedFile {
                status: if status.is_empty() { "?".to_string() } else { status },
                path: file_path,
            });
        }
    }

    let is_dirty = !changed_files.is_empty();

    let diff_stat = run_git_cmd(cwd, &["diff", "--stat"])
        .and_then(|s| if s.is_empty() { None } else { Some(s) });

    let cwd_str = cwd.to_string_lossy().replace('\\', "/");
    let repo_root_norm = repo_root.replace('\\', "/");
    let worktree_norm = worktree_path.replace('\\', "/");

    Some(GitContext {
        repo_root: repo_root_norm,
        worktree_path: worktree_norm,
        cwd: cwd_str,
        branch,
        head_sha,
        is_dirty,
        changed_files,
        diff_stat,
    })
}
