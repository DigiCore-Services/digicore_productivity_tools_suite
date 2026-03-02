//! Mock ScriptEnginePort (SE-18): For unit tests without Boa dependency.

use super::{ScriptContext, ScriptEnginePort, ScriptError};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Mock script engine for tests. Returns predefined results by code.
pub struct MockScriptEngine {
    /// Map of (script_type, code) -> result. Use "*" for script_type to match any.
    results: Mutex<HashMap<(String, String), Result<String, String>>>,
}

impl MockScriptEngine {
    pub fn new() -> Self {
        Self {
            results: Mutex::new(HashMap::new()),
        }
    }

    /// Register expected result for a (script_type, code) pair.
    pub fn expect(&self, script_type: &str, code: &str, result: Result<String, String>) {
        if let Ok(mut g) = self.results.lock() {
            g.insert((script_type.to_string(), code.to_string()), result);
        }
    }

    /// Register result for any script type.
    pub fn expect_any(&self, code: &str, result: Result<String, String>) {
        self.expect("*", code, result);
    }
}

impl Default for MockScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEnginePort for MockScriptEngine {
    fn execute(
        &self,
        script_type: &str,
        code: &str,
        _context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        let code = code.trim();
        if let Ok(g) = self.results.lock() {
            if let Some(r) = g.get(&(script_type.to_string(), code.to_string())) {
                return r.clone().map_err(|m| ScriptError::new(m).with_script_type(script_type));
            }
            if let Some(r) = g.get(&("*".to_string(), code.to_string())) {
                return r.clone().map_err(|m| ScriptError::new(m).with_script_type(script_type));
            }
        }
        Err(ScriptError::new(format!(
            "[MockScriptEngine: no expectation for ({}, {:?})]",
            script_type, code
        ))
        .with_script_type(script_type))
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["js"]
    }

    fn load_global_library(&mut self, _path: &Path) -> Result<(), ScriptError> {
        Ok(())
    }
}
