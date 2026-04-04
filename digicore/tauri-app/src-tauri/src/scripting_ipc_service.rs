//! Bounded inbound service for scripting logs, global libraries, engine config, and signer registry.

use crate::scripting_signer_registry::{
    load_scripting_signer_registry, normalize_fingerprint, now_unix_secs_string,
    save_scripting_signer_registry as persist_scripting_signer_registry_json, ScriptingSignerRegistryState,
};

use super::*;

pub(crate) async fn get_script_logs(_host: ApiImpl) -> Result<Vec<ScriptLogEntryDto>, String> {
    let logs = digicore_text_expander::application::scripting::get_script_logs();
    Ok(logs
        .into_iter()
        .map(|e| ScriptLogEntryDto {
            timestamp: e.timestamp,
            script_type: e.script_type,
            message: e.message,
            duration_ms: e.duration_ms as f64,
            code_len: e.code_len as u32,
            is_error: e.is_error,
        })
        .collect())
}

pub(crate) async fn clear_script_logs(_host: ApiImpl) -> Result<(), String> {
    digicore_text_expander::application::scripting::clear_script_logs();
    Ok(())
}

pub(crate) async fn get_script_library_js(_host: ApiImpl) -> Result<String, String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = if cfg.js.library_paths.is_empty() {
        base.join(&cfg.js.library_path)
    } else {
        base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
    };
    Ok(std::fs::read_to_string(&lib_path).unwrap_or_else(|_| {
        r#"/**
 * Text Expansion Pro - Global Script Library
 */
function greet(name) { return "Hello, " + name + "!"; }
"#
        .to_string()
    }))
}

pub(crate) async fn save_script_library_js(_host: ApiImpl, content: String) -> Result<(), String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = if cfg.js.library_paths.is_empty() {
        base.join(&cfg.js.library_path)
    } else {
        base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
    };
    if let Some(parent) = lib_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
    digicore_text_expander::application::scripting::load_and_apply_script_libraries();
    super::diag_log(
        "info",
        format!("[Scripting][JavaScript] Saved global library to {}", lib_path.display()),
    );
    Ok(())
}

pub(crate) async fn get_script_library_py(_host: ApiImpl) -> Result<String, String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = base.join(&cfg.py.library_path);
    Ok(std::fs::read_to_string(&lib_path).unwrap_or_else(|_| {
        r#"# DigiCore Global Python Library
def py_greet(name: str) -> str:
    return f"Hello, {name}!"
"#
        .to_string()
    }))
}

pub(crate) async fn save_script_library_py(_host: ApiImpl, content: String) -> Result<(), String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = base.join(&cfg.py.library_path);
    if let Some(parent) = lib_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
    digicore_text_expander::application::scripting::load_and_apply_script_libraries();
    super::diag_log(
        "info",
        format!("[Scripting][Python] Saved global library to {}", lib_path.display()),
    );
    Ok(())
}

pub(crate) async fn get_script_library_lua(_host: ApiImpl) -> Result<String, String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = base.join(&cfg.lua.library_path);
    Ok(std::fs::read_to_string(&lib_path).unwrap_or_else(|_| {
        r#"-- DigiCore Global Lua Library
function lua_greet(name)
  return "Hello, " .. tostring(name) .. "!"
end
"#
        .to_string()
    }))
}

pub(crate) async fn save_script_library_lua(_host: ApiImpl, content: String) -> Result<(), String> {
    let cfg = get_scripting_config();
    let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
    let lib_path = base.join(&cfg.lua.library_path);
    if let Some(parent) = lib_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
    digicore_text_expander::application::scripting::load_and_apply_script_libraries();
    super::diag_log(
        "info",
        format!("[Scripting][Lua] Saved global library to {}", lib_path.display()),
    );
    Ok(())
}

pub(crate) async fn get_scripting_engine_config(
    _host: ApiImpl,
) -> Result<ScriptingEngineConfigDto, String> {
    let cfg = get_scripting_config();
    Ok(ScriptingEngineConfigDto {
        dsl: ScriptingDslConfigDto {
            enabled: cfg.dsl.enabled,
        },
        http: ScriptingHttpConfigDto {
            timeout_secs: cfg.http.timeout_secs as u32,
            retry_count: cfg.http.retry_count,
            retry_delay_ms: cfg.http.retry_delay_ms as u32,
            use_async: cfg.http.use_async,
        },
        py: ScriptingPyConfigDto {
            enabled: cfg.py.enabled,
            path: cfg.py.path,
            library_path: cfg.py.library_path,
        },
        lua: ScriptingLuaConfigDto {
            enabled: cfg.lua.enabled,
            path: cfg.lua.path,
            library_path: cfg.lua.library_path,
        },
    })
}

pub(crate) async fn save_scripting_engine_config(
    _host: ApiImpl,
    config: ScriptingEngineConfigDto,
) -> Result<(), String> {
    let mut cfg = get_scripting_config();
    cfg.dsl.enabled = config.dsl.enabled;
    let timeout_clamped = config.http.timeout_secs.clamp(1, 60);
    let retry_count_clamped = config.http.retry_count.min(10);
    let retry_delay_clamped = config.http.retry_delay_ms.clamp(50, 20_000);
    cfg.http.timeout_secs = timeout_clamped as u64;
    cfg.http.retry_count = retry_count_clamped;
    cfg.http.retry_delay_ms = retry_delay_clamped as u64;
    cfg.http.use_async = config.http.use_async;

    cfg.py.enabled = config.py.enabled;
    cfg.py.path = config.py.path.trim().to_string();
    if !config.py.library_path.trim().is_empty() {
        cfg.py.library_path = config.py.library_path.trim().to_string();
    }

    cfg.lua.enabled = config.lua.enabled;
    cfg.lua.path = config.lua.path.trim().to_string();
    if !config.lua.library_path.trim().is_empty() {
        cfg.lua.library_path = config.lua.library_path.trim().to_string();
    }

    set_scripting_config(cfg);
    if timeout_clamped != config.http.timeout_secs
        || retry_count_clamped != config.http.retry_count
        || retry_delay_clamped != config.http.retry_delay_ms
    {
        super::diag_log(
            "warn",
            format!(
                "[Scripting][HTTP] Clamped settings timeout={} retry_count={} retry_delay_ms={}",
                timeout_clamped, retry_count_clamped, retry_delay_clamped
            ),
        );
    }
    super::diag_log(
        "info",
        format!(
            "[Scripting][Config] Saved dsl_enabled={} http_async={} py_enabled={} lua_enabled={}",
            config.dsl.enabled, config.http.use_async, config.py.enabled, config.lua.enabled
        ),
    );
    Ok(())
}

pub(crate) async fn get_scripting_signer_registry(_host: ApiImpl) -> Result<ScriptingSignerRegistryDto, String> {
    let state = load_scripting_signer_registry();
    Ok(ScriptingSignerRegistryDto {
        allow_unknown_signers: state.allow_unknown_signers,
        trust_on_first_use: state.trust_on_first_use,
        trusted_fingerprints: state.trusted_fingerprints,
        blocked_fingerprints: state.blocked_fingerprints,
    })
}

pub(crate) async fn save_scripting_signer_registry(
    _host: ApiImpl,
    registry: ScriptingSignerRegistryDto,
) -> Result<(), String> {
    let current = load_scripting_signer_registry();
    let mut first_seen = current.trusted_first_seen_utc;
    for fp in registry
        .trusted_fingerprints
        .iter()
        .map(|s| normalize_fingerprint(s))
        .filter(|s| !s.is_empty())
    {
        first_seen.entry(fp).or_insert_with(now_unix_secs_string);
    }
    let state = ScriptingSignerRegistryState {
        allow_unknown_signers: registry.allow_unknown_signers,
        trust_on_first_use: registry.trust_on_first_use,
        trusted_fingerprints: registry.trusted_fingerprints.clone(),
        blocked_fingerprints: registry.blocked_fingerprints.clone(),
        trusted_first_seen_utc: first_seen,
    };
    persist_scripting_signer_registry_json(&state)?;
    super::diag_log(
        "info",
        format!(
            "[ScriptingSignerRegistry] Saved allow_unknown={} tofu={} trusted={} blocked={}",
            registry.allow_unknown_signers,
            registry.trust_on_first_use,
            registry.trusted_fingerprints.len(),
            registry.blocked_fingerprints.len()
        ),
    );
    Ok(())
}

