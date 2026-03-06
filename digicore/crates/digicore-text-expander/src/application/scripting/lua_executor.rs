//! Lua script executor (SE-27): {lua:code} via subprocess.
//!
//! Configuration-first: lua.enabled, lua.path. Uses temp file to pass code.
//! Code should print to stdout (e.g. print(1+2)).

use super::config::get_config;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Execute Lua code. Returns stdout or error string.
/// Code is written to temp file and executed. Code should call print() for output.
pub fn execute_lua(code: &str) -> String {
    let cfg = get_config();
    if !cfg.lua.enabled {
        return "[Lua disabled by config]".to_string();
    }

    let lua = if cfg.lua.path.is_empty() {
        "lua"
    } else {
        cfg.lua.path.trim()
    };

    let mut combined = load_lua_library(&cfg.lua.library_path);
    if !combined.is_empty() && !combined.ends_with('\n') {
        combined.push('\n');
    }
    combined.push_str(code);

    let tmp = std::env::temp_dir().join(format!("digicore_lua_{}.lua", std::process::id()));
    if fs::write(&tmp, combined).is_err() {
        return "[Lua Error: failed to write temp file]".to_string();
    }

    let output = Command::new(lua).arg(&tmp).output();

    let _ = fs::remove_file(&tmp);

    match output {
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.is_empty() {
                    format!("[Lua Error: exit code {}]", o.status.code().unwrap_or(-1))
                } else {
                    format!("[Lua Error: {}]", stderr.trim())
                }
            }
        }
        Err(e) => format!("[Lua Error: {}]", e),
    }
}

fn load_lua_library(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }
    let full = dirs::config_dir()
        .unwrap_or_else(|| Path::new(".").into())
        .join("DigiCore")
        .join(path);
    std::fs::read_to_string(full).unwrap_or_default()
}
