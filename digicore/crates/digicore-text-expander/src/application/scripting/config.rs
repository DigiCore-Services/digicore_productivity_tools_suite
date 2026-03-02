//! Scripting configuration (SE-1, SE-2, SE-3): Externalized config for {js:}, {http:}, {run:}.
//!
//! Loads from %APPDATA%/DigiCore/config/scripting.json with fallback to defaults.
//! SE-3: Environment-based overrides via DIGICORE_ENV (dev, test, prod).

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Per-script-type configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptingConfig {
    #[serde(default)]
    pub dsl: DslConfig,
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub js: JsConfig,
    #[serde(default)]
    pub py: PyConfig,
    #[serde(default)]
    pub lua: LuaConfig,
    #[serde(default)]
    pub run: RunConfig,
    /// SE-3: When true, enable debug logging (script execution, HTTP, etc.).
    #[serde(default)]
    pub debug_logging: bool,
}

impl Default for ScriptingConfig {
    fn default() -> Self {
        Self {
            dsl: DslConfig::default(),
            http: HttpConfig::default(),
            js: JsConfig::default(),
            py: PyConfig::default(),
            lua: LuaConfig::default(),
            run: RunConfig::default(),
            debug_logging: false,
        }
    }
}

/// DSL config for {dsl:expr} (SE-25).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DslConfig {
    /// When true, {dsl:expr} is enabled.
    #[serde(default = "default_dsl_enabled")]
    pub enabled: bool,
}

fn default_dsl_enabled() -> bool {
    true
}

impl Default for DslConfig {
    fn default() -> Self {
        Self {
            enabled: true,
        }
    }
}

/// Python script config for {py:...} (SE-26).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyConfig {
    /// When true, {py:code} is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Path to python executable. Empty = "python" from PATH.
    #[serde(default)]
    pub path: String,
    /// Path to global Python library (relative to DigiCore config root). Plan 6.8.4.
    #[serde(default = "default_py_library_path")]
    pub library_path: String,
}

fn default_py_library_path() -> String {
    "scripts/global_library.py".to_string()
}

impl Default for PyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: String::new(),
            library_path: default_py_library_path(),
        }
    }
}

/// Lua script config for {lua:...} (SE-27).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaConfig {
    /// When true, {lua:code} is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Path to lua executable. Empty = "lua" from PATH.
    #[serde(default)]
    pub path: String,
    /// Path to global Lua library (relative to DigiCore config root). Plan 6.8.4.
    #[serde(default = "default_lua_library_path")]
    pub library_path: String,
}

fn default_lua_library_path() -> String {
    "scripts/global_library.lua".to_string()
}

impl Default for LuaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: String::new(),
            library_path: default_lua_library_path(),
        }
    }
}

impl ScriptingConfig {
    /// Apply environment-based overrides (SE-3). Call after loading base config.
    pub fn apply_env_overrides(&mut self) {
        self.apply_env_overrides_from(get_environment().as_str());
    }

    /// Apply overrides for a given env string (for testing).
    pub fn apply_env_overrides_from(&mut self, env: &str) {
        match env {
            "dev" => {
                self.http.timeout_secs = self.http.timeout_secs.max(10);
                self.js.timeout_secs = self.js.timeout_secs.max(10);
                self.debug_logging = true;
                self.js.debug_execution = true;
            }
            "test" => {
                self.http.timeout_secs = 2;
                self.js.timeout_secs = 2;
                self.js.sandbox_enabled = false;
                self.debug_logging = false;
            }
            "prod" => {
                self.js.sandbox_enabled = true;
            }
            _ => {}
        }
    }
}

/// Get environment from DIGICORE_ENV or RUST_ENV. Returns "dev", "test", "prod", or empty.
pub fn get_environment() -> String {
    std::env::var("DIGICORE_ENV")
        .or_else(|_| std::env::var("RUST_ENV"))
        .unwrap_or_default()
        .to_lowercase()
}

/// HTTP fetcher config for {http:url|path}.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    /// SE-22: Optional domain allowlist. Empty = allow all. e.g. ["api.example.com", "example.com"].
    #[serde(default)]
    pub url_allowlist: Vec<String>,
    /// Request timeout in seconds.
    #[serde(default = "default_http_timeout")]
    pub timeout_secs: u64,
    /// SE-9: Retry count (0 = no retry). Default 3 with exponential backoff.
    #[serde(default = "default_http_retry_count")]
    pub retry_count: u32,
    /// SE-9: Initial backoff delay in ms before first retry.
    #[serde(default = "default_http_retry_delay_ms")]
    pub retry_delay_ms: u64,
    /// SE-24: When true, use async reqwest (tokio) instead of blocking. Default false.
    #[serde(default)]
    pub use_async: bool,
}

fn default_http_timeout() -> u64 {
    5
}

fn default_http_retry_count() -> u32 {
    3
}

fn default_http_retry_delay_ms() -> u64 {
    500
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            url_allowlist: Vec::new(),
            timeout_secs: 5,
            retry_count: 3,
            retry_delay_ms: 500,
            use_async: false,
        }
    }
}

/// JavaScript engine config for {js:...}.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsConfig {
    /// SE-17: Path to global library (relative to DigiCore config root). Default scripts/global_library.js.
    #[serde(default = "default_library_path")]
    pub library_path: String,
    /// SE-17: Multiple library paths (concatenated). When non-empty, overrides library_path.
    #[serde(default)]
    pub library_paths: Vec<String>,
    /// Execution timeout in seconds (0 = no limit).
    #[serde(default)]
    pub timeout_secs: u64,
    /// Fallback when engine fails (e.g. "[JS unavailable]").
    #[serde(default = "default_js_fallback")]
    pub fallback_on_error: String,
    /// SE-21: When true, reject code containing eval(, Function(, new Function (sandbox).
    #[serde(default = "default_sandbox_enabled")]
    pub sandbox_enabled: bool,
    /// SE-3: When true, log script execution (type, duration, success). Requires debug_logging.
    #[serde(default)]
    pub debug_execution: bool,
    /// SE-8: Max recursion depth before error (0 = use Boa default).
    #[serde(default)]
    pub recursion_limit: usize,
    /// SE-8: Max loop iterations before error (0 = no limit).
    #[serde(default)]
    pub loop_iteration_limit: u64,
}

fn default_library_path() -> String {
    "scripts/global_library.js".to_string()
}

fn default_sandbox_enabled() -> bool {
    true
}

fn default_js_fallback() -> String {
    "[JS Error]".to_string()
}

impl Default for JsConfig {
    fn default() -> Self {
        Self {
            library_path: default_library_path(),
            library_paths: Vec::new(),
            timeout_secs: 5,
            fallback_on_error: "[JS Error]".to_string(),
            sandbox_enabled: true,
            debug_execution: false,
            recursion_limit: 1000,
            loop_iteration_limit: 1_000_000,
        }
    }
}

/// Run command config for {run:} (F24).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    /// When true, {run:command} is disabled (recommended for security).
    #[serde(default = "default_run_disabled")]
    pub disabled: bool,
    /// Comma-separated allowlist: python, cmd, C:\Scripts\, etc. Empty = block all.
    #[serde(default)]
    pub allowlist: String,
}

fn default_run_disabled() -> bool {
    true
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            disabled: true,
            allowlist: String::new(),
        }
    }
}

static SCRIPTING_CONFIG: Mutex<Option<ScriptingConfig>> = Mutex::new(None);

/// Load config from standard path. Called at startup. SE-3: Applies env overrides.
pub fn load_config() -> ScriptingConfig {
    let path = config_path();
    let mut cfg = if let Some(p) = &path {
        if let Ok(content) = std::fs::read_to_string(p) {
            if let Ok(c) = serde_json::from_str::<ScriptingConfig>(&content) {
                c
            } else {
                ScriptingConfig::default()
            }
        } else {
            ScriptingConfig::default()
        }
    } else {
        ScriptingConfig::default()
    };
    cfg.apply_env_overrides();
    if let Ok(mut g) = SCRIPTING_CONFIG.lock() {
        *g = Some(cfg.clone());
    }
    cfg
}

/// Get current config (loads defaults if not yet loaded).
pub fn get_config() -> ScriptingConfig {
    if let Ok(g) = SCRIPTING_CONFIG.lock() {
        if let Some(ref c) = *g {
            return c.clone();
        }
    }
    load_config()
}

/// Update and persist config.
pub fn set_config(config: ScriptingConfig) {
    if let Ok(mut g) = SCRIPTING_CONFIG.lock() {
        *g = Some(config.clone());
    }
    if let Some(p) = config_path() {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(f) = std::fs::File::create(&p) {
            let _ = serde_json::to_writer_pretty(f, &config);
        }
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("DigiCore").join("config").join("scripting.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_env_overrides_dev() {
        let mut cfg = ScriptingConfig::default();
        cfg.apply_env_overrides_from("dev");
        assert!(cfg.debug_logging);
        assert!(cfg.http.timeout_secs >= 10);
        assert!(cfg.js.debug_execution);
    }

    #[test]
    fn test_apply_env_overrides_test() {
        let mut cfg = ScriptingConfig::default();
        cfg.apply_env_overrides_from("test");
        assert_eq!(cfg.http.timeout_secs, 2);
        assert!(!cfg.js.sandbox_enabled);
    }
}
