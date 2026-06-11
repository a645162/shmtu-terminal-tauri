use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

/// Email domains to ignore when collecting git contributors.
/// These are typically auto-generated or placeholder addresses
/// that don't correspond to real GitHub accounts.
const IGNORED_EMAIL_DOMAINS: &[&str] = &[
    "users.noreply.github.com",
    "example.com",
];

fn main() {
    // Extract git contributors (name + email) BEFORE tauri_build::build()
    let contributors_json = extract_git_contributors();
    println!("cargo:rustc-env=GIT_CONTRIBUTORS={}", contributors_json);

    tauri_build::build();
}

/// Extract unique git contributors from the current repo AND its submodules.
/// Only scans this repo (shmtu-terminal-tauri) + its own submodules (e.g. shmtu-cas-rs).
fn extract_git_contributors() -> String {
    let local_root = find_git_root();
    if local_root.is_none() {
        return "[]".to_string();
    }
    let local_root = local_root.unwrap();

    let mut seen: HashSet<String> = HashSet::new();
    let mut entries: Vec<String> = Vec::new();

    // 1. Collect from this repo
    collect_from_repo(&local_root, &mut seen, &mut entries);

    // 2. Collect from this repo's own submodules (e.g. src-tauri/vendor/shmtu-cas-rs)
    collect_from_submodules(&local_root, &mut seen, &mut entries);

    if entries.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", entries.join(","))
    }
}

/// Walk up from CWD to find the top-level git work tree.
fn find_git_root() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

/// Parse `git log --format=%aN||%aE` from a given repo directory.
fn collect_from_repo(dir: &PathBuf, seen: &mut HashSet<String>, entries: &mut Vec<String>) {
    let output = match Command::new("git")
        .args(["log", "--format=%aN||%aE"])
        .current_dir(dir)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let log = String::from_utf8_lossy(&output.stdout);
    for line in log.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, "||");
        let name = parts.next().unwrap_or("").trim().to_string();
        let email = parts.next().unwrap_or("").trim().to_lowercase();

        if name.is_empty() || email.is_empty() || seen.contains(&email) {
            continue;
        }

        // Skip ignored email domains
        if is_ignored_email(&email) {
            continue;
        }

        seen.insert(email.clone());

        let escaped_name = json_escape(&name);
        let escaped_email = json_escape(&email);
        entries.push(format!(r#"{{"name":"{}","email":"{}"}}"#, escaped_name, escaped_email));
    }
}

/// Recursively discover submodules via `git submodule status` and collect from each.
fn collect_from_submodules(root: &PathBuf, seen: &mut HashSet<String>, entries: &mut Vec<String>) {
    let output = match Command::new("git")
        .args(["submodule", "status"])
        .current_dir(root)
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let status = String::from_utf8_lossy(&output.stdout);
    for line in status.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: <status-char><sha1> <path>
        // e.g. " b1c5289... shmtu-terminal-android"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let sub_path = root.join(parts[1]);
        if sub_path.join(".git").exists() || sub_path.join(".git").is_dir() {
            collect_from_repo(&sub_path, seen, entries);
            // Recurse into nested submodules
            collect_from_submodules(&sub_path, seen, entries);
        }
    }
}

/// Check if an email address belongs to an ignored domain.
fn is_ignored_email(email: &str) -> bool {
    let domain = email.split('@').last().unwrap_or("");
    IGNORED_EMAIL_DOMAINS.iter().any(|d| domain == *d)
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
