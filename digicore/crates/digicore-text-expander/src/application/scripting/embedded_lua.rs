//! Embedded Lua engine implementation using mlua.
//! SE-32: Performance; SE-33: Context injection.
//! Refactored to be Send + Sync by using fresh Lua per execution.

use super::{ScriptContext, ScriptEnginePort, ScriptError};
use mlua::prelude::*;
use std::path::Path;
use std::sync::Mutex;

pub struct EmbeddedLuaEngine {
    /// Cached global library code.
    global_library: Mutex<String>,
}

impl EmbeddedLuaEngine {
    pub fn new() -> Self {
        Self {
            global_library: Mutex::new(String::new()),
        }
    }
}

impl ScriptEnginePort for EmbeddedLuaEngine {
    fn execute(
        &self,
        _script_type: &str,
        code: &str,
        context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        let lua = Lua::new();
        
        // In Lua, we can inject globals
        let globals = lua.globals();
        
        globals.set("clipboard", context.clipboard.as_str()).map_err(to_script_err)?;
        globals.set("clip_history", context.clip_history.clone()).map_err(to_script_err)?;
        
        for (tag, value) in &context.user_vars {
            let _ = globals.set(tag.as_str(), value.as_str());
        }

        // Apply global library if present
        if let Ok(lib) = self.global_library.lock() {
            if !lib.is_empty() {
                lua.load(&*lib).exec().map_err(to_script_err)?;
            }
        }

        let val = match lua.load(code).eval::<LuaValue>() {
            Ok(v) if !v.is_nil() => v,
            _ => {
                // If eval returned nil or failed, try exec and then check 'result' global
                lua.load(code).exec().map_err(to_script_err)?;
                lua.globals().get::<_, LuaValue>("result").unwrap_or(LuaValue::Nil)
            }
        };
        
        lua_val_to_string(val)
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["lua"]
    }

    fn load_global_library(&self, path: &Path) -> Result<(), ScriptError> {
        let lib_code = std::fs::read_to_string(path)
            .map_err(|e| ScriptError::new(format!("Failed to load library: {}", e)))?;
            
        if let Ok(mut g) = self.global_library.lock() {
            *g = lib_code;
        }
        Ok(())
    }
}

fn lua_val_to_string(val: LuaValue) -> Result<String, ScriptError> {
    match val {
        LuaValue::String(s) => Ok(s.to_str().unwrap_or_default().to_string()),
        LuaValue::Number(n) => Ok(n.to_string()),
        LuaValue::Integer(i) => Ok(i.to_string()),
        LuaValue::Boolean(b) => Ok(b.to_string()),
        _ => Ok(String::new()),
    }
}

fn to_script_err(e: LuaError) -> ScriptError {
    ScriptError::new(format!("[Lua Error: {}]", e)).with_script_type("lua")
}
