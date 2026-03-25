//! Embedded Python engine implementation using pyo3.
//! SE-30: Performance; SE-31: Context injection.

use super::{ScriptContext, ScriptEnginePort, ScriptError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::Path;

use std::sync::Mutex;

pub struct EmbeddedPyEngine {
    /// Cached global library code.
    global_library: Mutex<String>,
}

impl EmbeddedPyEngine {
    pub fn new() -> Self {
        Python::with_gil(|py| {
            // Pre-initialize or check python status
            let _ = py.run_bound("import sys", None, None);
        });
        Self {
            global_library: Mutex::new(String::new()),
        }
    }
}

impl ScriptEnginePort for EmbeddedPyEngine {
    fn execute(
        &self,
        _script_type: &str,
        code: &str,
        context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        Python::with_gil(|py| {
            let globals = PyDict::new_bound(py);
            
            // Context injection
            globals.set_item("clipboard", &context.clipboard).map_err(to_script_err)?;
            globals.set_item("clip_history", &context.clip_history).map_err(to_script_err)?;
            
            for (tag, value) in &context.user_vars {
                let _ = globals.set_item(tag, value);
            }

            // Apply global library if present
            if let Ok(lib) = self.global_library.lock() {
                if !lib.is_empty() {
                    // Execute library in global scope so functions are available
                    py.run_bound(&*lib, Some(&globals), Some(&globals)).map_err(to_script_err)?;
                }
            }

            let result = py.run_bound(code, Some(&globals), Some(&globals));
            
            match result {
                Ok(_) => {
                    // Try to get 'result' variable if it exists
                    if let Ok(res) = globals.get_item("result") {
                        if let Some(res) = res {
                            return Ok(res.to_string());
                        }
                    }
                    Ok(String::new())
                }
                Err(e) => Err(to_script_err(e)),
            }
        })
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["py"]
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

fn to_script_err(e: PyErr) -> ScriptError {
    ScriptError::new(format!("[Python Error: {}]", e)).with_script_type("py")
}
