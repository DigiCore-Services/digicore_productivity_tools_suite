//! Boa JavaScript engine adapter for {js:...} placeholder execution.
//! SE-7: JS execution timeout; SE-8: Memory/recursion limits; SE-11: Graceful degradation; SE-21: JS sandbox.

use super::config::get_config;
use super::js_sandbox::check_sandbox;
use super::{ScriptContext, ScriptEnginePort, ScriptError};
use boa_engine::vm::RuntimeLimits;
use boa_engine::{js_string, Context, JsValue, Source};
use std::path::Path;
use std::sync::mpsc;
use std::sync::Mutex;

/// In-memory global JS library (prepended before each {js:...} eval).
static GLOBAL_LIBRARY: Mutex<String> = Mutex::new(String::new());

/// Convert tag (e.g. "{var:Env}") to valid JS identifier (e.g. "var_Env"). SE-16.
fn tag_to_js_identifier(tag: &str) -> Option<String> {
    let inner = tag.strip_prefix('{')?.strip_suffix('}')?;
    let (type_part, label_part) = inner.split_once(':')?;
    let label = label_part.split('|').next().unwrap_or("").trim();
    let slug: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if slug.is_empty() {
        return None;
    }
    Some(format!("{}_{}", type_part, slug))
}

/// Run eval in current thread (used by timeout spawn).
fn run_eval(full_code: &str, script_ctx: &ScriptContext) -> Result<String, ScriptError> {
    let engine = BoaScriptEngine::new();
    let mut context = engine.create_context(script_ctx);
    let result = context.eval(Source::from_bytes(full_code.as_bytes()));
    match result {
        Ok(value) => {
            let str_result = value
                .to_string(&mut context)
                .map_err(|e| ScriptError::new(format!("JS to_string error: {:?}", e)).with_script_type("js"))?;
            Ok(str_result.to_std_string_escaped())
        }
        Err(e) => Err(ScriptError::new(format!("[JS Error: {}]", e.to_string()))
            .with_script_type("js")
            .with_source(full_code)),
    }
}

/// Set the global JavaScript library content. Called when Save & Reload JS or when tab loads.
pub fn set_global_library(content: String) {
    if let Ok(mut g) = GLOBAL_LIBRARY.lock() {
        *g = content;
    }
}

/// Boa-based JavaScript engine implementing ScriptEnginePort.
pub struct BoaScriptEngine {
    global_library_loaded: Mutex<bool>,
}

impl Default for BoaScriptEngine {
    fn default() -> Self {
        Self {
            global_library_loaded: Mutex::new(false),
        }
    }
}

impl BoaScriptEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_context(&self, ctx: &ScriptContext) -> Context {
        let mut context = Context::default();

        let cfg = get_config();
        if cfg.js.recursion_limit > 0 || cfg.js.loop_iteration_limit > 0 {
            let mut limits = RuntimeLimits::default();
            if cfg.js.recursion_limit > 0 {
                limits.set_recursion_limit(cfg.js.recursion_limit);
            }
            if cfg.js.loop_iteration_limit > 0 {
                limits.set_loop_iteration_limit(cfg.js.loop_iteration_limit);
            }
            context.set_runtime_limits(limits);
        }

        context
            .register_global_property(
                js_string!("clipboard"),
                JsValue::from(boa_engine::JsString::from(ctx.clipboard.as_str())),
                boa_engine::property::Attribute::all(),
            )
            .expect("register clipboard");

        for (i, clip) in ctx.clip_history.iter().take(10).enumerate() {
            let name = format!("clip{}", i + 1);
            context
                .register_global_property(
                    boa_engine::JsString::from(name.as_str()),
                    JsValue::from(boa_engine::JsString::from(clip.as_str())),
                    boa_engine::property::Attribute::all(),
                )
                .expect("register clip");
        }

        for (tag, value) in &ctx.user_vars {
            if let Some(js_name) = tag_to_js_identifier(tag) {
                let _ = context.register_global_property(
                    boa_engine::JsString::from(js_name.as_str()),
                    JsValue::from(boa_engine::JsString::from(value.as_str())),
                    boa_engine::property::Attribute::all(),
                );
            }
        }

        context
    }

    fn eval_code(&self, code: &str, script_ctx: &ScriptContext) -> Result<String, ScriptError> {
        let cfg = get_config();
        let timeout_secs = cfg.js.timeout_secs;
        let fallback = cfg.js.fallback_on_error.clone();

        let full_code = if let Ok(global) = GLOBAL_LIBRARY.lock() {
            if global.is_empty() {
                code.to_string()
            } else {
                format!("{}\n{}", global.as_str(), code)
            }
        } else {
            code.to_string()
        };

        if cfg.js.sandbox_enabled {
            if let Err(e) = check_sandbox(&full_code) {
                return Err(ScriptError::new(e).with_script_type("js").with_source(&full_code));
            }
        }

        let script_ctx = script_ctx.clone();

        if timeout_secs > 0 {
            let (tx, rx) = mpsc::channel();
            let full_code = full_code.clone();
            std::thread::spawn(move || {
                let result = run_eval(&full_code, &script_ctx);
                let _ = tx.send(result);
            });
            match rx.recv_timeout(std::time::Duration::from_secs(timeout_secs)) {
                Ok(Ok(s)) => Ok(s),
                Ok(Err(e)) => Err(e),
                Err(mpsc::RecvTimeoutError::Timeout) => Err(ScriptError::new(format!(
                    "[JS Error: execution timeout ({}s)]",
                    timeout_secs
                ))
                .with_script_type("js")),
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(ScriptError::new(fallback).with_script_type("js")),
            }
        } else {
            run_eval(&full_code, &script_ctx)
        }
    }
}

impl ScriptEnginePort for BoaScriptEngine {
    fn execute(
        &self,
        script_type: &str,
        code: &str,
        context: &ScriptContext,
    ) -> Result<String, ScriptError> {
        if script_type != "js" {
            return Err(ScriptError::new(format!("Unsupported script type: {}", script_type))
                .with_script_type(script_type));
        }
        self.eval_code(code.trim(), context)
    }

    fn supported_types(&self) -> Vec<&'static str> {
        vec!["js"]
    }

    fn load_global_library(&self, path: &Path) -> Result<(), ScriptError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ScriptError::new(format!("Failed to read JS library: {}", e)))?;
        set_global_library(content);
        if let Ok(mut g) = self.global_library_loaded.lock() {
            *g = true;
        }
        Ok(())
    }
}
