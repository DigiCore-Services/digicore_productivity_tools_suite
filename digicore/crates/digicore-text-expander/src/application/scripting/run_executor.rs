//! Run command executor (Phase D): {run:cmd} with allowlist and disable.
//!
//! IsRunAllowed: path prefix (contains `\`) or exec name match.
//! Errors: [Run disabled by config], [Run blocked: not in allowlist], [Run Error: ...]

use super::config::get_config;
use std::process::Command;

/// Check if command is allowed by allowlist. Empty allowlist = block all.
/// - Path prefix: entry contains `\` -> allow if cmd executable starts with that path
/// - Exec name: allow if executable name matches (e.g. python, cmd, hostname)
pub fn is_run_allowed(cmd: &str, allowlist: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return false;
    }
    let exec = cmd.split_whitespace().next().unwrap_or("");
    if exec.is_empty() {
        return false;
    }
    let exec_lower = exec.to_lowercase();
    let exec_has_path = exec.contains('\\');

    for entry in allowlist.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let entry_lower = entry.to_lowercase();
        if entry.contains('\\') {
            if exec_has_path && exec_lower.starts_with(&entry_lower) {
                return true;
            }
        } else {
            let base = if exec_has_path {
                exec.rsplit('\\').next().unwrap_or(exec)
            } else {
                exec
            };
            let base_lower = base.to_lowercase();
            if base_lower == entry_lower
                || base_lower.starts_with(&format!("{}.", entry_lower))
                || base_lower == format!("{}.exe", entry_lower)
            {
                return true;
            }
        }
    }
    false
}

/// Execute run command. Returns stdout or error string.
pub fn execute_run(cmd: &str) -> String {
    let cfg = get_config();
    if cfg.run.disabled {
        return "[Run disabled by config]".to_string();
    }
    if !is_run_allowed(cmd, &cfg.run.allowlist) {
        return "[Run blocked: not in allowlist]".to_string();
    }

    #[cfg(target_os = "windows")]
    let output = Command::new("cmd")
        .args(["/C", cmd])
        .output();

    #[cfg(not(target_os = "windows"))]
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output();

    match output {
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.is_empty() {
                    format!("[Run Error: exit code {}]", o.status.code().unwrap_or(-1))
                } else {
                    format!("[Run Error: {}]", stderr.trim())
                }
            }
        }
        Err(e) => format!("[Run Error: {}]", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_run_allowed_exec_name() {
        assert!(is_run_allowed("hostname", "hostname,cmd,python"));
        assert!(is_run_allowed("cmd /c dir", "hostname,cmd,python"));
        assert!(!is_run_allowed("evil", "hostname,cmd,python"));
    }

    #[test]
    fn test_is_run_allowed_path_prefix() {
        assert!(is_run_allowed(r"C:\Scripts\run.ps1", r"C:\Scripts\"));
        assert!(!is_run_allowed(r"C:\Other\run.ps1", r"C:\Scripts\"));
    }

    #[test]
    fn test_is_run_allowed_empty_allowlist() {
        assert!(!is_run_allowed("hostname", ""));
    }
}
