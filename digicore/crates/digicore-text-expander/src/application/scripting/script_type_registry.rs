//! Script type registry (SE-15): Central dispatch for {js:}, {http:}, future {run:}.
//!
//! Template processor iterates over registered prefixes and dispatches to handlers.

use super::clipboard_resolver::resolve_clipboard_in_js;
use super::config::get_config;
use super::dsl_evaluator::evaluate as evaluate_dsl;
use super::placeholder_parser::find_balanced_tag;
use super::lua_executor::execute_lua;
use super::py_executor::execute_py;
use super::run_executor::execute_run;
use super::script_context_builder::build_from_template_config;
use super::weather_lookup::resolve_weather_placeholder;
use super::get_registry;
use std::collections::HashMap;
use std::time::Instant;

/// Registered script-type prefixes: js, http, run, dsl, py, lua, weather.
pub const SCRIPT_TYPE_PREFIXES: &[&str] = &["js", "http", "run", "dsl", "py", "lua", "weather"];

/// Check if content has a balanced tag for the given prefix (e.g. "js" -> "{js:...}").
pub fn find_tag_for_prefix<'a>(s: &'a str, prefix: &str) -> Option<(&'a str, usize)> {
    find_balanced_tag(s, &format!("{prefix}:"))
}

/// Dispatch placeholder by prefix. Returns replacement string or None if prefix unknown.
pub fn dispatch(
    prefix: &str,
    inner: &str,
    date_format: &str,
    time_format: &str,
    clipboard: &str,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> Option<String> {
    let registry = get_registry();
    match prefix {
        "js" => {
            let script_ctx = build_from_template_config(
                date_format,
                time_format,
                clipboard,
                clip_history,
                user_vars,
            );
            let inner_resolved = resolve_clipboard_in_js(inner, clipboard);
            let cfg = get_config();
            let do_log = cfg.debug_logging && cfg.js.debug_execution;
            let start = if do_log { Some(Instant::now()) } else { None };
            let code_len = inner_resolved.len();
            let exec_result = registry.engine.execute("js", &inner_resolved, &script_ctx);
            if let (true, Some(t0)) = (do_log, start) {
                let dur_ms = t0.elapsed().as_millis();
                match &exec_result {
                    Ok(_) => eprintln!("[DigiCore] script js ok len={} dur_ms={}", code_len, dur_ms),
                    Err(e) => eprintln!(
                        "[DigiCore] script js err len={} dur_ms={} msg={}",
                        code_len, dur_ms, e.message
                    ),
                }
            }
            Some(exec_result.unwrap_or_else(|e| e.message))
        }
        "http" => {
            let (url, path) = if let Some(pipe) = inner.find('|') {
                let (u, p) = inner.split_at(pipe);
                (u.trim(), Some(p[1..].trim()))
            } else {
                (inner.trim(), None)
            };
            let cfg = get_config();
            let do_log = cfg.debug_logging;
            let start = if do_log { Some(Instant::now()) } else { None };
            let result = registry.http_fetcher.fetch(url, path);
            if let (true, Some(t0)) = (do_log, start) {
                let dur_ms = t0.elapsed().as_millis();
                eprintln!(
                    "[DigiCore] script http ok url_len={} dur_ms={}",
                    url.len(),
                    dur_ms
                );
            }
            Some(result)
        }
        "dsl" => {
            let cfg = get_config();
            if !cfg.dsl.enabled {
                return Some("[DSL disabled by config]".to_string());
            }
            let expr = inner.trim();
            let result = evaluate_dsl(expr);
            Some(result)
        }
        "py" => Some(execute_py(inner.trim())),
        "lua" => Some(execute_lua(inner.trim())),
        "run" => {
            let cmd = inner.trim();
            let cfg = get_config();
            let do_log = cfg.debug_logging;
            let start = if do_log { Some(Instant::now()) } else { None };
            let result = execute_run(cmd);
            if let (true, Some(t0)) = (do_log, start) {
                let dur_ms = t0.elapsed().as_millis();
                eprintln!(
                    "[DigiCore] script run ok cmd_len={} dur_ms={}",
                    cmd.len(),
                    dur_ms
                );
            }
            Some(result)
        }
        "weather" => {
            let resolved_inner = resolve_user_vars_in_inner(inner, user_vars);
            Some(resolve_weather_placeholder(
                &resolved_inner,
                registry.http_fetcher.as_ref(),
            ))
        }
        _ => None,
    }
}

fn resolve_user_vars_in_inner(
    inner: &str,
    user_vars: Option<&HashMap<String, String>>,
) -> String {
    let Some(user_vars) = user_vars else {
        return inner.to_string();
    };
    let mut out = inner.to_string();
    for (tag, value) in user_vars {
        out = out.replace(tag, value);
    }
    out
}
