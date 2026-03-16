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

/// Load script libraries from config and apply via set_global_library.
/// Called at startup and when config is reloaded.
pub fn load_and_apply_script_libraries() {
    let cfg = get_config();
    let base = digicore_config_root();
    let paths: Vec<std::path::PathBuf> = if cfg.js.library_paths.is_empty() {
        if cfg.js.library_path.is_empty() {
            return;
        }
        vec![base.join(&cfg.js.library_path)]
    } else {
        cfg.js.library_paths
            .iter()
            .map(|p| base.join(p))
            .collect()
    };

    let mut content = String::new();
    for path in paths {
        if let Ok(s) = std::fs::read_to_string(&path) {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&s);
        }
    }
    set_global_library(content);
}
