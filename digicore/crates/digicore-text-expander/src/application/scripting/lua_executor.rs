//! Lua script executor (SE-27): {lua:code} via subprocess.
//!
//! Configuration-first: lua.enabled, lua.path. Uses temp file to pass code.
//! Code should print to stdout (e.g. print(1+2)).

use super::config::get_config;
use super::{ScriptContext, ScriptEnginePort, ScriptError};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Lua-based scripting engine implementing ScriptEnginePort.
#[derive(Default)]
pub struct LuaScriptEngine;

impl LuaScriptEngine {
    pub fn new() -> Self {
        Self
    }
}

impl ScriptEnginePort for LuaScriptEngine {
    fn execute(
        &self,
        _script_type: &str,
        code: &str,
        _context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        let cfg = get_config();
        if !cfg.lua.enabled {
            return Ok("[Lua disabled by config]".to_string());
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
            return Err(ScriptError::new("[Lua Error: failed to write temp file]").with_script_type("lua"));
        }

        let output = Command::new(lua).arg(&tmp).output();

        let _ = fs::remove_file(&tmp);

        match output {
            Ok(o) => {
                if o.status.success() {
                    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    let msg = if stderr.is_empty() {
                        format!("[Lua Error: exit code {}]", o.status.code().unwrap_or(-1))
                    } else {
                        format!("[Lua Error: {}]", stderr.trim())
                    };
                    Err(ScriptError::new(msg).with_script_type("lua"))
                }
            }
            Err(e) => Err(ScriptError::new(format!("[Lua Error: {}]", e)).with_script_type("lua")),
        }
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["lua"]
    }

    fn load_global_library(&self, _path: &Path) -> Result<(), ScriptError> {
        // Managed by config library_path for now.
        Ok(())
    }
}

fn load_lua_library(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }
    let full = crate::ports::data_path_resolver::DataPathResolver::root()
        .join(path);
    std::fs::read_to_string(full).unwrap_or_default()
}
