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

    let worktree_path =
        run_git_cmd(cwd, &["rev-parse", "--show-toplevel"]).unwrap_or_else(|| repo_root.clone());

    let branch = run_git_cmd(cwd, &["branch", "--show-current"])
        .filter(|b| !b.is_empty())
        .or_else(|| run_git_cmd(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "HEAD (detached)".to_string());

    let head_sha =
        run_git_cmd(cwd, &["rev-parse", "HEAD"]).unwrap_or_else(|| "UNKNOWN".to_string());

    let status = Command::new("git")
        .args(["status", "--porcelain=v1", "-z"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !status.status.success() {
        return None;
    }

    let changed_files = parse_porcelain(&status.stdout);

    let is_dirty = !changed_files.is_empty();

    let diff_stat = run_git_cmd(cwd, &["diff", "--stat", "HEAD"])
        .or_else(|| run_git_cmd(cwd, &["diff", "--stat"]))
        .filter(|s| !s.is_empty());

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

fn parse_porcelain(bytes: &[u8]) -> Vec<ChangedFile> {
    let mut records = bytes.split(|b| *b == 0);

    let mut files = Vec::new();

    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }

        let status = String::from_utf8_lossy(&record[..2]).trim().to_string();

        let path = String::from_utf8_lossy(&record[3..]).into_owned();

        // In -z mode a rename is destination NUL source NUL.

        if record[..2].iter().any(|b| *b == b'R' || *b == b'C') {
            records.next();
        }

        files.push(ChangedFile { status, path });
    }

    files
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]

    fn status_preserves_first_column_unicode_spaces_and_renames() {
        let files =
            parse_porcelain(" M файл.rs\0R  new name.rs\0old.rs\0??  leading.txt\0".as_bytes());

        assert_eq!(files.len(), 3);

        assert_eq!(files[0].status, "M");

        assert_eq!(files[0].path, "файл.rs");

        assert_eq!(files[1].path, "new name.rs");

        assert_eq!(files[2].path, " leading.txt");
    }
}
