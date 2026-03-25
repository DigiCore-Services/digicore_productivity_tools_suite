//! Script library loader (SE-17): Load global_library.js from config paths.
//!
//! Reads library_path or library_paths from JsConfig, resolves against com.digicore.text-expander config root,
//! concatenates content, and calls set_global_library.

use super::boa_engine::set_global_library;
use super::config::get_config;

/// DigiCore config root (e.g. %APPDATA%/com.digicore.text-expander).
fn digicore_config_root() -> std::path::PathBuf {
    crate::ports::data_path_resolver::DataPathResolver::root()
}

/// Load script libraries from config and apply to all registered engines.
/// Called at startup and when config is reloaded.
pub fn load_and_apply_script_libraries() {
    let cfg = get_config();
    let base = digicore_config_root();
    let registry = super::get_registry();

    // 1. Handle JavaScript (Boa) - uses legacy static setter + engine load
    let js_paths: Vec<std::path::PathBuf> = if cfg.js.library_paths.is_empty() {
        if cfg.js.library_path.is_empty() {
            Vec::new()
        } else {
            vec![base.join(&cfg.js.library_path)]
        }
    } else {
        cfg.js.library_paths.iter().map(|p| base.join(p)).collect()
    };

    let mut js_content = String::new();
    for path in &js_paths {
        if let Ok(s) = std::fs::read_to_string(path) {
            if !js_content.is_empty() {
                js_content.push('\n');
            }
            js_content.push_str(&s);
        }
    }
    set_global_library(js_content);

    // 2. Sync JS paths to JS engines in registry
    if let Some(engine) = registry.engines.get("js") {
        for path in &js_paths {
            let _ = engine.load_global_library(path);
        }
    }

    // 3. Handle Python
    if let Some(engine) = registry.engines.get("py") {
        if !cfg.py.library_path.is_empty() {
            let path = base.join(&cfg.py.library_path);
            let _ = engine.load_global_library(&path);
        }
    }

    // 4. Handle Lua
    if let Some(engine) = registry.engines.get("lua") {
        if !cfg.lua.library_path.is_empty() {
            let path = base.join(&cfg.lua.library_path);
            let _ = engine.load_global_library(&path);
        }
    }
}
