//! Scripting Engine - extensible port-based architecture for {js:}, {http:}, etc.
//!
//! Implements ScriptEnginePort, HttpFetcherPort per Dynamic Templates Plan Section 6, 11.

mod async_reqwest_fetcher;
mod boa_engine;
mod clipboard_resolver;
mod config;
mod dsl_evaluator;
mod http_fetcher;
mod http_port;
mod js_sandbox;
mod lua_executor;
mod mock_engine;
mod mock_http_fetcher;
mod placeholder_parser;
mod py_executor;
mod registry;
mod reqwest_fetcher;
mod run_executor;
mod script_context_builder;
mod script_library_loader;
mod script_type_registry;
mod url_allowlist;
mod weather_lookup;

pub use boa_engine::{set_global_library, BoaScriptEngine};
pub use clipboard_resolver::{escape_for_js_string, resolve_clipboard_in_js};
pub use config::{
    get_config as get_scripting_config, load_config as load_scripting_config,
    set_config as set_scripting_config, HttpConfig, JsConfig, RunConfig, ScriptingConfig,
};
pub use http_fetcher::fetch_http;
pub use script_library_loader::load_and_apply_script_libraries;
pub use http_port::HttpFetcherPort;
pub use mock_engine::MockScriptEngine;
pub use mock_http_fetcher::MockHttpFetcher;
pub use registry::{get_registry, set_registry, ScriptingRegistry};
pub use reqwest_fetcher::ReqwestHttpFetcher;
pub use placeholder_parser::{find_balanced_tag, parse_placeholder_at, ParsedPlaceholder};
pub use script_type_registry::{dispatch as dispatch_script_placeholder, find_tag_for_prefix, SCRIPT_TYPE_PREFIXES};
pub use weather_lookup::location_suggestions as weather_location_suggestions;

use std::collections::HashMap;
use std::path::Path;

/// Context injected into script execution (clipboard, clip history, env, etc.).
#[derive(Clone, Debug, Default)]
pub struct ScriptContext {
    /// Current clipboard for {clipboard} injection in JS.
    pub clipboard: String,
    /// Clipboard history entries for {clip:1}..{clip:N}.
    pub clip_history: Vec<String>,
    /// Pre-resolved date string (config format).
    pub date: String,
    /// Pre-resolved time string (config format).
    pub time: String,
    /// User-defined vars from VariableInputModal, injected as JS globals (SE-16).
    /// Keys: tag e.g. "{var:Env}"; values: user input.
    pub user_vars: HashMap<String, String>,
}

impl ScriptContext {
    /// Builder for ScriptContext (SE-6).
    pub fn builder() -> ScriptContextBuilder {
        ScriptContextBuilder::default()
    }
}

/// Builder for ScriptContext.
#[derive(Default)]
pub struct ScriptContextBuilder {
    clipboard: String,
    clip_history: Vec<String>,
    date: String,
    time: String,
    user_vars: HashMap<String, String>,
}

impl ScriptContextBuilder {
    pub fn clipboard(mut self, v: impl Into<String>) -> Self {
        self.clipboard = v.into();
        self
    }
    pub fn clip_history(mut self, v: Vec<String>) -> Self {
        self.clip_history = v;
        self
    }
    pub fn date(mut self, v: impl Into<String>) -> Self {
        self.date = v.into();
        self
    }
    pub fn time(mut self, v: impl Into<String>) -> Self {
        self.time = v.into();
        self
    }
    pub fn user_vars(mut self, v: HashMap<String, String>) -> Self {
        self.user_vars = v;
        self
    }
    pub fn build(self) -> ScriptContext {
        ScriptContext {
            clipboard: self.clipboard,
            clip_history: self.clip_history,
            date: self.date,
            time: self.time,
            user_vars: self.user_vars,
        }
    }
}

/// Error from script execution (SE-10: structured for diagnostics).
#[derive(Debug, Clone)]
pub struct ScriptError {
    pub message: String,
    /// Script type (e.g. "js").
    pub script_type: Option<String>,
    /// Source code snippet.
    pub source: Option<String>,
    /// Line number (1-based).
    pub line: Option<u32>,
    /// Column number (1-based).
    pub column: Option<u32>,
}

impl ScriptError {
    /// Create from message only (backward compatible).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            script_type: None,
            source: None,
            line: None,
            column: None,
        }
    }

    /// Builder: set script type.
    pub fn with_script_type(mut self, t: impl Into<String>) -> Self {
        self.script_type = Some(t.into());
        self
    }

    /// Builder: set source.
    pub fn with_source(mut self, s: impl Into<String>) -> Self {
        self.source = Some(s.into());
        self
    }

    /// Builder: set line/column.
    pub fn with_location(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ScriptError {}

/// Port for pluggable scripting engines (JS, future: Python, Lua).
pub trait ScriptEnginePort: Send + Sync {
    /// Execute script; returns string result or error.
    fn execute(
        &self,
        script_type: &str,
        code: &str,
        context: &ScriptContext,
    ) -> Result<String, ScriptError>;

    /// Supported script types (e.g. ["js"]).
    fn supported_types(&self) -> Vec<&'static str>;

    /// Load global library from path (for hot-reload).
    fn load_global_library(&mut self, path: &Path) -> Result<(), ScriptError>;
}

#[cfg(test)]
mod tests {
    use super::placeholder_parser::find_balanced_tag;

    #[test]
    fn test_find_balanced_tag_simple() {
        let s = "{js: 10 + 20}";
        let (inner, len) = find_balanced_tag(s, "js:").unwrap();
        assert_eq!(inner, "10 + 20");
        assert_eq!(len, 13);
    }

    #[test]
    fn test_find_balanced_tag_nested() {
        let s = r#"{js: "a" + (1 ? "b" : "c") }"#;
        let (inner, _) = find_balanced_tag(s, "js:").unwrap();
        assert!(inner.contains("a"));
    }

    #[test]
    fn test_find_balanced_tag_http() {
        let s = "{http:https://api.example.com}";
        let (inner, len) = find_balanced_tag(s, "http:").unwrap();
        assert_eq!(inner, "https://api.example.com");
        assert_eq!(len, s.len());
    }
}
