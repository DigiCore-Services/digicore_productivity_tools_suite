//! Python script executor (SE-26): {py:code} via subprocess.
//!
//! Configuration-first: py.enabled, py.path. Uses temp file to avoid escaping.

use super::config::get_config;
use super::{ScriptContext, ScriptEnginePort, ScriptError};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::path::Path;
use std::process::Command;

/// Python-based scripting engine implementing ScriptEnginePort.
#[derive(Default)]
pub struct PyScriptEngine;

impl PyScriptEngine {
    pub fn new() -> Self {
        Self
    }
}

impl ScriptEnginePort for PyScriptEngine {
    fn execute(
        &self,
        _script_type: &str,
        code: &str,
        _context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        let cfg = get_config();
        if !cfg.py.enabled {
            return Ok("[Python disabled by config]".to_string());
        }

        let python = if cfg.py.path.is_empty() {
            "python"
        } else {
            cfg.py.path.trim()
        };

        let library = load_py_library(&cfg.py.library_path);
        let code_b64 = B64.encode(code.as_bytes());
        let lib_b64 = B64.encode(library.as_bytes());
        let runner = r#"import base64,sys
g = {}
lib = base64.b64decode(sys.argv[1]).decode()
code = base64.b64decode(sys.argv[2]).decode()
if lib.strip():
    exec(lib, g, g)
try:
    r = eval(code, g, g)
except SyntaxError:
    exec(code, g, g)
    r = g.get("result", "")
print("" if r is None else r)
"#;
        let output = Command::new(python)
            .args(["-c", runner, &lib_b64, &code_b64])
            .output();

        match output {
            Ok(o) => {
                if o.status.success() {
                    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    let msg = if stderr.is_empty() {
                        format!("[Python Error: exit code {}]", o.status.code().unwrap_or(-1))
                    } else {
                        format!("[Python Error: {}]", stderr.trim())
                    };
                    Err(ScriptError::new(msg).with_script_type("py"))
                }
            }
            Err(e) => Err(ScriptError::new(format!("[Python Error: {}]", e)).with_script_type("py")),
        }
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["py"]
    }

    fn load_global_library(&self, _path: &Path) -> Result<(), ScriptError> {
        // Managed by config library_path for now.
        Ok(())
    }
}

fn load_py_library(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }
    let full = crate::ports::data_path_resolver::DataPathResolver::root()
        .join(path);
    std::fs::read_to_string(full).unwrap_or_default()
}
