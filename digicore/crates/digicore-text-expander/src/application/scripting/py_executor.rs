//! Python script executor (SE-26): {py:code} via subprocess.
//!
//! Configuration-first: py.enabled, py.path. Uses temp file to avoid escaping.

use super::config::get_config;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::process::Command;

/// Execute Python code. Returns stdout or error string.
/// Code is passed via base64 as argv[1] to avoid escaping.
pub fn execute_py(code: &str) -> String {
    let cfg = get_config();
    if !cfg.py.enabled {
        return "[Python disabled by config]".to_string();
    }

    let python = if cfg.py.path.is_empty() {
        "python"
    } else {
        cfg.py.path.trim()
    };

    let code_b64 = B64.encode(code.as_bytes());
    let output = Command::new(python)
        .args(["-c", "import base64,sys;c=base64.b64decode(sys.argv[1]).decode();print(eval(c))", &code_b64])
        .output();

    match output {
        Ok(o) => {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            } else {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.is_empty() {
                    format!("[Python Error: exit code {}]", o.status.code().unwrap_or(-1))
                } else {
                    format!("[Python Error: {}]", stderr.trim())
                }
            }
        }
        Err(e) => format!("[Python Error: {}]", e),
    }
}
