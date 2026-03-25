//! Scripting registry (SE-4): Holds injectable ScriptEnginePort and HttpFetcherPort.
//!
//! Enables DI for tests; defaults to BoaScriptEngine and ReqwestHttpFetcher.
//! SE-24: When http.use_async is true, uses AsyncReqwestHttpFetcher.

use super::async_reqwest_fetcher::AsyncReqwestHttpFetcher;
use super::boa_engine::BoaScriptEngine;
use super::config::get_config;
use super::http_port::HttpFetcherPort;
use super::reqwest_fetcher::ReqwestHttpFetcher;
use super::ScriptEnginePort;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// Registry holding script engines and HTTP fetcher. Set at startup or for tests.
pub struct ScriptingRegistry {
    pub engines: HashMap<String, Arc<dyn ScriptEnginePort>>,
    pub http_fetcher: Arc<dyn HttpFetcherPort>,
}

impl Default for ScriptingRegistry {
    fn default() -> Self {
        let mut engines: HashMap<String, Arc<dyn ScriptEnginePort>> = HashMap::new();
        engines.insert("js".to_string(), Arc::new(BoaScriptEngine::new()));
        engines.insert("py".to_string(), Arc::new(super::embedded_py::EmbeddedPyEngine::new()));
        engines.insert("lua".to_string(), Arc::new(super::embedded_lua::EmbeddedLuaEngine::new()));
        engines.insert("run".to_string(), Arc::new(super::run_executor::RunScriptEngine::new()));

        Self {
            engines,
            http_fetcher: default_http_fetcher(),
        }
    }
}

/// Select HTTP fetcher based on config (SE-24: use_async).
fn default_http_fetcher() -> Arc<dyn HttpFetcherPort> {
    let cfg = get_config();
    if cfg.http.use_async {
        Arc::new(AsyncReqwestHttpFetcher)
    } else {
        Arc::new(ReqwestHttpFetcher)
    }
}

static REGISTRY: Mutex<Option<ScriptingRegistry>> = Mutex::new(None);

/// Get the scripting registry. Initializes with defaults on first call.
pub fn get_registry() -> ScriptingRegistry {
    if let Ok(mut g) = REGISTRY.lock() {
        if g.is_none() {
            super::config::load_config();
            *g = Some(ScriptingRegistry::default());
        }
        if let Some(ref r) = *g {
            return ScriptingRegistry {
                engines: r.engines.clone(),
                http_fetcher: Arc::clone(&r.http_fetcher),
            };
        }
    }
    ScriptingRegistry::default()
}

/// Set registry (for tests). Call before process_with_config in tests.
pub fn set_registry(registry: ScriptingRegistry) {
    if let Ok(mut g) = REGISTRY.lock() {
        *g = Some(registry);
    }
}
