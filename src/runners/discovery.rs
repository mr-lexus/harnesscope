use std::path::{Path, PathBuf};

fn is_file_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            return meta.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(not(unix))]
    {
        true
    }
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    // If name is already an absolute or relative path that exists
    let direct = Path::new(name);
    if is_file_executable(direct) {
        return Some(direct.to_path_buf());
    }

    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if let Some(path_var) = std::env::var_os("PATH") {
        search_dirs.extend(std::env::split_paths(&path_var));
    }

    // Common fallback directories across platforms
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    if !home.is_empty() {
        let home_path = PathBuf::from(&home);
        search_dirs.push(home_path.join(".cargo").join("bin"));
        search_dirs.push(home_path.join(".local").join("bin"));
        search_dirs.push(home_path.join("bin"));
    }

    #[cfg(target_os = "macos")]
    {
        search_dirs.push(PathBuf::from("/opt/homebrew/bin"));
        search_dirs.push(PathBuf::from("/opt/homebrew/sbin"));
        search_dirs.push(PathBuf::from("/usr/local/bin"));
    }

    #[cfg(target_os = "linux")]
    {
        search_dirs.push(PathBuf::from("/usr/local/bin"));
        search_dirs.push(PathBuf::from("/usr/bin"));
        search_dirs.push(PathBuf::from("/snap/bin"));
    }

    #[cfg(windows)]
    let extensions: &[&str] = &[".exe", ".cmd", ".bat", ".ps1", ""];
    #[cfg(not(windows))]
    let extensions: &[&str] = &[""];

    for p in &search_dirs {
        for ext in extensions {
            let candidate = p.join(format!("{}{}", name, ext));
            if is_file_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    // On macOS: check /Applications/<Name>.app/Contents/MacOS/<Name>
    #[cfg(target_os = "macos")]
    {
        let capitalized = {
            let mut c = name.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        };

        let app_candidates = vec![
            format!("/Applications/{}.app/Contents/MacOS/{}", capitalized, capitalized),
            format!("/Applications/{}.app/Contents/MacOS/{}", name, name),
            format!("{}/Applications/{}.app/Contents/MacOS/{}", home, capitalized, capitalized),
            format!("{}/Applications/{}.app/Contents/MacOS/{}", home, name, name),
        ];

        for cand in app_candidates {
            let p = PathBuf::from(cand);
            if is_file_executable(&p) {
                return Some(p);
            }
        }
    }

    None
}

pub fn discover_runner_binary(runner_name: &str) -> Option<PathBuf> {
    let env_var_name = match runner_name {
        "codex" => "HARNESSCOPE_CODEX_BIN",
        "copilot" => "HARNESSCOPE_COPILOT_BIN",
        "opencode" => "HARNESSCOPE_OPENCODE_BIN",
        _ => return None,
    };

    if let Ok(custom_path) = std::env::var(env_var_name) {
        let p = PathBuf::from(custom_path);
        if p.is_file() {
            return Some(p);
        }
    }

    // Default binary name searches
    let search_names: &[&str] = match runner_name {
        "codex" => &["codex"],
        "copilot" => &["copilot", "github-copilot-cli"],
        "opencode" => &["opencode"],
        _ => &[],
    };

    for name in search_names {
        if let Some(bin) = find_executable(name) {
            return Some(bin);
        }
    }

    None
}
