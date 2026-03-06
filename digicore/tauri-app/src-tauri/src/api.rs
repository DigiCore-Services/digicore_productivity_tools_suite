//! TauRPC API - type-safe IPC procedures for DigiCore Text Expander.

use crate::{
    clipboard_repository,
    app_state_to_dto, AppearanceTransparencyRuleDto, ConfigUpdateDto, ExpansionStatsDto, GhostFollowerStateDto,
    GhostSuggestorStateDto, ScriptingDslConfigDto, ScriptingEngineConfigDto, ScriptingHttpConfigDto,
    ScriptingDetachedSignatureExportDto, ScriptingLuaConfigDto, ScriptingProfileDiffEntryDto,
    ScriptingProfileDryRunDto, ScriptingProfileImportResultDto, ScriptingProfilePreviewDto,
    ScriptingSignerRegistryDto, CopyToClipboardConfigDto, CopyToClipboardStatsDto,
    ScriptingPyConfigDto, SettingsBundlePreviewDto, SettingsImportResultDto, SnippetLogicTestResultDto,
    UiPrefsDto,
};
use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::expansion_diagnostics;
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
use digicore_text_expander::application::expansion_stats;
use digicore_text_expander::application::discovery;
use digicore_text_expander::application::ghost_follower;
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::{
    get_scripting_config, set_global_library, set_scripting_config,
};
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{
    sync_discovery_config, sync_ghost_config, update_library, GhostConfig,
};
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::ports::{storage_keys, StoragePort};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use regex::Regex;
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use windows::core::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetLayeredWindowAttributes, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible,
    SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
};

use crate::{
    ClipEntryDto, DiagnosticEntryDto, InteractiveVarDto, PinnedSnippetDto,
    PendingVariableInputDto as PendingVarDto, SuggestionDto,
};

fn load_appearance_rules(storage: &JsonFileStorageAdapter) -> Vec<AppearanceTransparencyRuleDto> {
    storage
        .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
        .and_then(|s| serde_json::from_str::<Vec<AppearanceTransparencyRuleDto>>(&s).ok())
        .unwrap_or_default()
}

fn normalize_process_key(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_string()
}

fn sort_appearance_rules_deterministic(rules: &mut [AppearanceTransparencyRuleDto]) {
    rules.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| normalize_process_key(&a.app_process).cmp(&normalize_process_key(&b.app_process)))
            .then_with(|| a.app_process.to_ascii_lowercase().cmp(&b.app_process.to_ascii_lowercase()))
            .then_with(|| a.opacity.cmp(&b.opacity))
    });
}

fn effective_rules_for_enforcement(mut rules: Vec<AppearanceTransparencyRuleDto>) -> Vec<AppearanceTransparencyRuleDto> {
    sort_appearance_rules_deterministic(&mut rules);
    let mut seen = HashSet::new();
    let mut effective = Vec::new();
    for rule in rules {
        let key = normalize_process_key(&rule.app_process);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        if rule.enabled {
            effective.push(rule);
        }
    }
    effective
}

fn save_appearance_rules(rules: &[AppearanceTransparencyRuleDto]) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    let serialized = serde_json::to_string(rules).map_err(|e| e.to_string())?;
    storage.set(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON, &serialized);
    storage
        .persist_if_safe()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn default_copy_to_clipboard_config(max_history_entries: u32) -> CopyToClipboardConfigDto {
    let json_dir = digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_json_dir();
    let image_dir = digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_images_dir();
    CopyToClipboardConfigDto {
        enabled: true,
        min_log_length: 1,
        mask_cc: false,
        mask_ssn: false,
        mask_email: false,
        blacklist_processes: String::new(),
        max_history_entries,
        json_output_enabled: true,
        json_output_dir: json_dir.to_string_lossy().to_string(),
        image_storage_dir: image_dir.to_string_lossy().to_string(),
    }
}

fn load_copy_to_clipboard_config(storage: &JsonFileStorageAdapter, max_history_entries: u32) -> CopyToClipboardConfigDto {
    let mut cfg = default_copy_to_clipboard_config(max_history_entries);
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_ENABLED) {
        cfg.enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MIN_LOG_LENGTH) {
        if let Ok(parsed) = v.parse::<u32>() {
            cfg.min_log_length = parsed.clamp(1, 2000);
        }
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_CC) {
        cfg.mask_cc = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_SSN) {
        cfg.mask_ssn = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_EMAIL) {
        cfg.mask_email = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_BLACKLIST_PROCESSES) {
        cfg.blacklist_processes = v;
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_ENABLED) {
        cfg.json_output_enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_DIR) {
        cfg.json_output_dir = v;
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_IMAGE_STORAGE_DIR) {
        cfg.image_storage_dir = v;
    }
    cfg
}

fn save_copy_to_clipboard_config(config: &CopyToClipboardConfigDto) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::COPY_TO_CLIPBOARD_ENABLED, &config.enabled.to_string());
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_MIN_LOG_LENGTH,
        &config.min_log_length.clamp(1, 2000).to_string(),
    );
    storage.set(storage_keys::COPY_TO_CLIPBOARD_MASK_CC, &config.mask_cc.to_string());
    storage.set(storage_keys::COPY_TO_CLIPBOARD_MASK_SSN, &config.mask_ssn.to_string());
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_MASK_EMAIL,
        &config.mask_email.to_string(),
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_BLACKLIST_PROCESSES,
        &config.blacklist_processes,
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_ENABLED,
        &config.json_output_enabled.to_string(),
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_DIR,
        &config.json_output_dir,
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_IMAGE_STORAGE_DIR,
        &config.image_storage_dir,
    );
    storage.persist_if_safe().map(|_| ()).map_err(|e| e.to_string())
}

fn process_is_blacklisted(process_name: &str, blacklist_csv: &str) -> bool {
    let process_norm = process_name.trim().to_ascii_lowercase();
    if process_norm.is_empty() {
        return false;
    }
    blacklist_csv
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .any(|blocked| process_norm == blocked || process_norm == format!("{blocked}.exe"))
}

fn apply_masking(mut content: String, cfg: &CopyToClipboardConfigDto) -> String {
    if cfg.mask_email {
        let email_re = Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").ok();
        if let Some(re) = email_re {
            content = re.replace_all(&content, "[masked_email]").to_string();
        }
    }
    if cfg.mask_ssn {
        let ssn_re = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").ok();
        if let Some(re) = ssn_re {
            content = re.replace_all(&content, "[masked_ssn]").to_string();
        }
    }
    if cfg.mask_cc {
        let cc_re = Regex::new(r"\b(?:\d[ -]?){13,19}\b").ok();
        if let Some(re) = cc_re {
            content = re.replace_all(&content, "[masked_card]").to_string();
        }
    }
    content
}

fn normalize_clipboard_path_or_default(raw: &str, fallback: PathBuf) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        PathBuf::from(trimmed)
    }
}

fn write_clipboard_text_json_record(
    output_dir: &str,
    content: &str,
    process_name: &str,
    window_title: &str,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let output_root = normalize_clipboard_path_or_default(
        output_dir,
        digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_json_dir(),
    );
    std::fs::create_dir_all(&output_root).map_err(|e| e.to_string())?;
    let file_name = format!("clipboard_{:013}_{:06}.json", now.as_millis(), now.subsec_micros());
    let file_path = output_root.join(file_name);
    let payload = serde_json::json!({
        "schema_version": "1.0.0",
        "entry_type": "text",
        "created_at_unix_ms": now.as_millis().to_string(),
        "process_name": process_name,
        "window_title": window_title,
        "content": content
    });
    let serialized = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(file_path, serialized).map_err(|e| e.to_string())
}

pub(crate) fn persist_clipboard_entry_with_settings(
    content: &str,
    process_name: &str,
    window_title: &str,
) -> Result<bool, String> {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled || !cfg.json_output_enabled {
        return Ok(false);
    }
    if process_is_blacklisted(process_name, &cfg.blacklist_processes) {
        return Ok(false);
    }
    if content.trim().chars().count() < cfg.min_log_length as usize {
        return Ok(false);
    }
    let masked = apply_masking(content.to_string(), &cfg);
    let inserted = clipboard_repository::insert_entry(&masked, process_name, window_title)?;
    log::info!("[Clipboard] clipboard_repository::insert_entry returned {}", inserted);
    if inserted {
        if let Err(err) = write_clipboard_text_json_record(
            &cfg.json_output_dir,
            &masked,
            process_name,
            window_title,
        ) {
            diag_log("warn", format!("[Clipboard][json.write_err] {err}"));
        }
        if cfg.max_history_entries > 0 {
            let _ = clipboard_repository::trim_to_depth(cfg.max_history_entries);
        }
    }
    Ok(inserted)
}

const SETTINGS_GROUP_TEMPLATES: &str = "templates";
const SETTINGS_GROUP_SYNC: &str = "sync";
const SETTINGS_GROUP_DISCOVERY: &str = "discovery";
const SETTINGS_GROUP_GHOST_SUGGESTOR: &str = "ghost_suggestor";
const SETTINGS_GROUP_GHOST_FOLLOWER: &str = "ghost_follower";
const SETTINGS_GROUP_CLIPBOARD_HISTORY: &str = "clipboard_history";
const SETTINGS_GROUP_COPY_TO_CLIPBOARD: &str = "copy_to_clipboard";
const SETTINGS_GROUP_CORE: &str = "core";
const SETTINGS_GROUP_SCRIPT_RUNTIME: &str = "script_runtime";
const SETTINGS_GROUP_APPEARANCE: &str = "appearance";

const SCRIPTING_PROFILE_GROUP_JAVASCRIPT: &str = "javascript";
const SCRIPTING_PROFILE_GROUP_PYTHON: &str = "python";
const SCRIPTING_PROFILE_GROUP_LUA: &str = "lua";
const SCRIPTING_PROFILE_GROUP_HTTP: &str = "http";
const SCRIPTING_PROFILE_GROUP_DSL: &str = "dsl";
const SCRIPTING_PROFILE_GROUP_RUN: &str = "run";
const SCRIPTING_PROFILE_SCHEMA_V2: &str = "2.0.0";
const SCRIPTING_PROFILE_SIGN_ALGO: &str = "ed25519-sha256-v1";
const SCRIPTING_PROFILE_SIGNING_KEY_STORAGE: &str = "scripting_profile_signing_key_b64";
const SCRIPTING_PROFILE_SIGNER_REGISTRY_STORAGE: &str = "scripting_profile_signer_registry_json";

fn all_settings_groups() -> Vec<&'static str> {
    vec![
        SETTINGS_GROUP_TEMPLATES,
        SETTINGS_GROUP_SYNC,
        SETTINGS_GROUP_DISCOVERY,
        SETTINGS_GROUP_GHOST_SUGGESTOR,
        SETTINGS_GROUP_GHOST_FOLLOWER,
        SETTINGS_GROUP_CLIPBOARD_HISTORY,
        SETTINGS_GROUP_COPY_TO_CLIPBOARD,
        SETTINGS_GROUP_CORE,
        SETTINGS_GROUP_SCRIPT_RUNTIME,
        SETTINGS_GROUP_APPEARANCE,
    ]
}

fn normalize_settings_group(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "templates" => Some(SETTINGS_GROUP_TEMPLATES),
        "sync" => Some(SETTINGS_GROUP_SYNC),
        "discovery" => Some(SETTINGS_GROUP_DISCOVERY),
        "ghost_suggestor" | "ghost-suggestor" | "ghost suggestor" => Some(SETTINGS_GROUP_GHOST_SUGGESTOR),
        "ghost_follower" | "ghost-follower" | "ghost follower" => Some(SETTINGS_GROUP_GHOST_FOLLOWER),
        "clipboard_history" | "clipboard-history" | "clipboard history" => Some(SETTINGS_GROUP_CLIPBOARD_HISTORY),
        "copy_to_clipboard" | "copy-to-clipboard" | "copy to clipboard" => {
            Some(SETTINGS_GROUP_COPY_TO_CLIPBOARD)
        }
        "core" => Some(SETTINGS_GROUP_CORE),
        "script_runtime" | "script-runtime" | "script runtime" => Some(SETTINGS_GROUP_SCRIPT_RUNTIME),
        "appearance" => Some(SETTINGS_GROUP_APPEARANCE),
        _ => None,
    }
}

fn normalized_selected_groups(groups: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    if groups.is_empty() {
        return all_settings_groups().into_iter().map(str::to_string).collect();
    }
    for g in groups {
        if let Some(n) = normalize_settings_group(g) {
            if !out.iter().any(|v| v == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

fn all_scripting_profile_groups() -> Vec<&'static str> {
    vec![
        SCRIPTING_PROFILE_GROUP_JAVASCRIPT,
        SCRIPTING_PROFILE_GROUP_PYTHON,
        SCRIPTING_PROFILE_GROUP_LUA,
        SCRIPTING_PROFILE_GROUP_HTTP,
        SCRIPTING_PROFILE_GROUP_DSL,
        SCRIPTING_PROFILE_GROUP_RUN,
    ]
}

fn normalize_scripting_profile_group(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "javascript" | "js" => Some(SCRIPTING_PROFILE_GROUP_JAVASCRIPT),
        "python" | "py" => Some(SCRIPTING_PROFILE_GROUP_PYTHON),
        "lua" => Some(SCRIPTING_PROFILE_GROUP_LUA),
        "http" | "weather" | "http_weather" | "http-weather" => Some(SCRIPTING_PROFILE_GROUP_HTTP),
        "dsl" => Some(SCRIPTING_PROFILE_GROUP_DSL),
        "run" | "run_security" | "run-security" | "run security" => Some(SCRIPTING_PROFILE_GROUP_RUN),
        _ => None,
    }
}

fn normalized_selected_scripting_profile_groups(groups: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    if groups.is_empty() {
        return all_scripting_profile_groups()
            .into_iter()
            .map(str::to_string)
            .collect();
    }
    for g in groups {
        if let Some(n) = normalize_scripting_profile_group(g) {
            if !out.iter().any(|v| v == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

#[derive(Clone)]
struct ParsedScriptingProfileBundle {
    schema_version_used: String,
    selected_groups: Vec<String>,
    groups_obj: serde_json::Map<String, serde_json::Value>,
    warnings: Vec<String>,
    valid: bool,
    signed_bundle: bool,
    signature_valid: bool,
    migrated_from_schema: Option<String>,
    signature_key_id: Option<String>,
    signer_fingerprint: Option<String>,
    signer_trusted: bool,
    signer_unknown: bool,
    trust_on_first_use: bool,
}

#[derive(Clone, Debug)]
struct ScriptingSignerRegistryState {
    allow_unknown_signers: bool,
    trust_on_first_use: bool,
    trusted_fingerprints: Vec<String>,
    blocked_fingerprints: Vec<String>,
    trusted_first_seen_utc: HashMap<String, String>,
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
}

fn load_scripting_signer_registry() -> ScriptingSignerRegistryState {
    let storage = JsonFileStorageAdapter::load();
    if let Some(raw) = storage.get(SCRIPTING_PROFILE_SIGNER_REGISTRY_STORAGE) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            let allow_unknown_signers = parsed
                .get("allow_unknown_signers")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let trust_on_first_use = parsed
                .get("trust_on_first_use")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let trusted_fingerprints = parsed
                .get("trusted_fingerprints")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(normalize_fingerprint)
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            let blocked_fingerprints = parsed
                .get("blocked_fingerprints")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(normalize_fingerprint)
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            let trusted_first_seen_utc = parsed
                .get("trusted_first_seen_utc")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (normalize_fingerprint(k), s.to_string())))
                        .filter(|(k, _)| !k.is_empty())
                        .collect::<HashMap<String, String>>()
                })
                .unwrap_or_default();
            return ScriptingSignerRegistryState {
                allow_unknown_signers,
                trust_on_first_use,
                trusted_fingerprints,
                blocked_fingerprints,
                trusted_first_seen_utc,
            };
        }
    }
    ScriptingSignerRegistryState {
        allow_unknown_signers: true,
        trust_on_first_use: false,
        trusted_fingerprints: Vec::new(),
        blocked_fingerprints: Vec::new(),
        trusted_first_seen_utc: HashMap::new(),
    }
}

fn save_scripting_signer_registry(dto: &ScriptingSignerRegistryState) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    let normalized = ScriptingSignerRegistryState {
        allow_unknown_signers: dto.allow_unknown_signers,
        trust_on_first_use: dto.trust_on_first_use,
        trusted_fingerprints: dto
            .trusted_fingerprints
            .iter()
            .map(|s| normalize_fingerprint(s))
            .filter(|s| !s.is_empty())
            .collect(),
        blocked_fingerprints: dto
            .blocked_fingerprints
            .iter()
            .map(|s| normalize_fingerprint(s))
            .filter(|s| !s.is_empty())
            .collect(),
        trusted_first_seen_utc: dto
            .trusted_first_seen_utc
            .iter()
            .map(|(k, v)| (normalize_fingerprint(k), v.clone()))
            .filter(|(k, _)| !k.is_empty())
            .collect(),
    };
    let serialized = serde_json::to_string(&serde_json::json!({
        "allow_unknown_signers": normalized.allow_unknown_signers,
        "trust_on_first_use": normalized.trust_on_first_use,
        "trusted_fingerprints": normalized.trusted_fingerprints,
        "blocked_fingerprints": normalized.blocked_fingerprints,
        "trusted_first_seen_utc": normalized.trusted_first_seen_utc
    }))
    .map_err(|e| e.to_string())?;
    storage.set(SCRIPTING_PROFILE_SIGNER_REGISTRY_STORAGE, &serialized);
    storage
        .persist_if_safe()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn upsert_trust_on_first_use_signer(signer_fingerprint: &str, source: &str) -> Result<bool, String> {
    let signer = normalize_fingerprint(signer_fingerprint);
    if signer.is_empty() {
        return Ok(false);
    }
    let mut state = load_scripting_signer_registry();
    if !state.trust_on_first_use {
        return Ok(false);
    }
    if state.blocked_fingerprints.iter().any(|v| v == &signer) {
        return Ok(false);
    }
    if state.trusted_fingerprints.iter().any(|v| v == &signer) {
        return Ok(false);
    }
    state.trusted_fingerprints.push(signer.clone());
    state
        .trusted_first_seen_utc
        .entry(signer.clone())
        .or_insert_with(now_unix_secs_string);
    save_scripting_signer_registry(&state)?;
    diag_log(
        "info",
        format!(
            "[ScriptingSignerTOFU][AUDIT] First trust established signer={} at={} source={}",
            signer,
            state
                .trusted_first_seen_utc
                .get(&signer)
                .cloned()
                .unwrap_or_else(now_unix_secs_string),
            source
        ),
    );
    Ok(true)
}

fn now_unix_secs_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn summarize_for_diff(value: &str) -> String {
    let clean = value.replace('\r', "");
    let max = 140usize;
    if clean.len() <= max {
        return clean;
    }
    format!("{}... (len={})", &clean[..max], clean.len())
}

fn get_or_create_scripting_profile_signing_key() -> Result<SigningKey, String> {
    let mut storage = JsonFileStorageAdapter::load();
    if let Some(existing) = storage.get(SCRIPTING_PROFILE_SIGNING_KEY_STORAGE) {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(existing.as_bytes())
            .map_err(|e| e.to_string())?;
        let key_bytes: [u8; 32] = raw
            .as_slice()
            .try_into()
            .map_err(|_| "Invalid signing key length in storage.".to_string())?;
        return Ok(SigningKey::from_bytes(&key_bytes));
    }

    let mut key_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key_bytes);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key_bytes);
    storage.set(SCRIPTING_PROFILE_SIGNING_KEY_STORAGE, &encoded);
    storage
        .persist_if_safe()
        .map_err(|e| format!("Failed to persist scripting profile signing key: {e}"))?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

fn verify_scripting_profile_signature(
    selected_groups: &[String],
    groups_obj: &serde_json::Map<String, serde_json::Value>,
    integrity: &serde_json::Map<String, serde_json::Value>,
) -> Result<(bool, String, String, Option<String>), String> {
    let algorithm = integrity
        .get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if algorithm != SCRIPTING_PROFILE_SIGN_ALGO {
        return Err(format!(
            "Unsupported scripting profile signature algorithm '{algorithm}'."
        ));
    }
    let public_key_b64 = integrity
        .get("public_key_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing integrity.public_key_b64".to_string())?;
    let signature_b64 = integrity
        .get("signature_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing integrity.signature_b64".to_string())?;
    let key_id = integrity
        .get("key_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let payload_sha256 = integrity
        .get("payload_sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing integrity.payload_sha256".to_string())?;

    let sign_payload = serde_json::json!({
        "schema_version": SCRIPTING_PROFILE_SCHEMA_V2,
        "selected_groups": selected_groups,
        "groups": groups_obj
    });
    let sign_bytes = serde_json::to_vec(&sign_payload).map_err(|e| e.to_string())?;
    let computed_hash = sha256_hex(&sign_bytes);
    if computed_hash != payload_sha256 {
        return Ok((
            false,
            computed_hash,
            "hash-mismatch".to_string(),
            key_id,
        ));
    }

    let pk_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    let pk_arr: [u8; 32] = pk_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid integrity public key length.".to_string())?;
    let verifying_key = VerifyingKey::from_bytes(&pk_arr).map_err(|e| e.to_string())?;
    let signer_fingerprint = sha256_hex(&verifying_key.to_bytes());
    if let Some(expected_key_id) = key_id.as_deref() {
        let actual_key_id = signer_fingerprint
            .chars()
            .take(16)
            .collect::<String>();
        if expected_key_id.trim().to_ascii_lowercase() != actual_key_id {
            return Ok((false, computed_hash, signer_fingerprint, key_id));
        }
    }

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.as_bytes())
        .map_err(|e| e.to_string())?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid integrity signature length.".to_string())?;
    let signature = Signature::from_bytes(&sig_arr);
    Ok((
        verifying_key.verify(&sign_bytes, &signature).is_ok(),
        computed_hash,
        signer_fingerprint,
        key_id,
    ))
}

fn parse_scripting_profile_bundle(
    path: &str,
) -> Result<ParsedScriptingProfileBundle, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let schema = root
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let mut warnings = Vec::new();
    let mut valid = true;
    let mut signed_bundle = false;
    let mut signature_valid = false;
    let mut migrated_from_schema = None;
    let mut signature_key_id: Option<String> = None;
    let mut signer_fingerprint: Option<String> = None;

    let groups_obj = root
        .get("groups")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing or invalid 'groups' object.".to_string())?
        .clone();

    let mut selected_groups = root
        .get("selected_groups")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    if selected_groups.is_empty() {
        selected_groups = groups_obj.keys().cloned().collect();
    }

    if schema.starts_with("1.") {
        migrated_from_schema = Some(schema.clone());
        warnings.push(format!(
            "Legacy profile schema '{schema}' detected; auto-migrated to {SCRIPTING_PROFILE_SCHEMA_V2} compatibility mode."
        ));
        warnings.push(
            "Legacy 1.x profile is unsigned. Signature verification unavailable for this file."
                .to_string(),
        );
    } else if schema == SCRIPTING_PROFILE_SCHEMA_V2 {
        match root.get("integrity").and_then(|v| v.as_object()) {
            Some(integrity) => {
                signed_bundle = true;
                match verify_scripting_profile_signature(&selected_groups, &groups_obj, integrity) {
                    Ok((ok, computed_hash, signer_fp, key_id)) => {
                        signature_valid = ok;
                        signer_fingerprint = Some(signer_fp);
                        signature_key_id = key_id;
                        if !ok {
                            valid = false;
                            warnings.push(
                                "Signature verification failed or payload hash mismatch."
                                    .to_string(),
                            );
                            warnings.push(format!("Computed payload_sha256={computed_hash}"));
                        }
                    }
                    Err(err) => {
                        valid = false;
                        warnings.push(format!("Signature verification error: {err}"));
                    }
                }
            }
            None => {
                valid = false;
                warnings.push("Missing integrity block for schema 2.0.0 profile.".to_string());
            }
        }
    } else {
        valid = false;
        warnings.push(format!(
            "Unsupported schema_version '{schema}'. Expected 1.x or {SCRIPTING_PROFILE_SCHEMA_V2}."
        ));
    }

    for key in groups_obj.keys() {
        if normalize_scripting_profile_group(key).is_none() {
            warnings.push(format!("Unknown group '{key}' will be ignored."));
        }
    }

    let mut signer_trusted = false;
    let mut signer_unknown = false;
    let mut trust_on_first_use = false;
    if signed_bundle && signature_valid {
        let registry = load_scripting_signer_registry();
        trust_on_first_use = registry.trust_on_first_use;
        if let Some(fp) = signer_fingerprint.as_ref().map(|s| normalize_fingerprint(s)) {
            let is_blocked = registry.blocked_fingerprints.iter().any(|v| v == &fp);
            let is_trusted = registry.trusted_fingerprints.iter().any(|v| v == &fp);
            signer_trusted = is_trusted;
            if is_blocked {
                valid = false;
                warnings.push(format!(
                    "Signer fingerprint '{}' is blocked by local trust policy.",
                    fp
                ));
            } else if !is_trusted {
                signer_unknown = true;
                if !registry.allow_unknown_signers && !registry.trust_on_first_use {
                    valid = false;
                    warnings.push(format!(
                        "Signer fingerprint '{}' is unknown and unknown signers are not allowed.",
                        fp
                    ));
                } else if registry.trust_on_first_use {
                    warnings.push(format!(
                        "Signer fingerprint '{}' is unknown. TOFU is enabled and can auto-trust this signer on first verified use.",
                        fp
                    ));
                } else {
                    warnings.push(format!(
                        "Signer fingerprint '{}' is unknown. Add to trusted registry if this source is expected.",
                        fp
                    ));
                }
            }
        }
    }

    Ok(ParsedScriptingProfileBundle {
        schema_version_used: if schema.starts_with("1.") {
            SCRIPTING_PROFILE_SCHEMA_V2.to_string()
        } else {
            schema
        },
        selected_groups,
        groups_obj,
        warnings,
        valid,
        signed_bundle,
        signature_valid,
        migrated_from_schema,
        signature_key_id,
        signer_fingerprint,
        signer_trusted,
        signer_unknown,
        trust_on_first_use,
    })
}

fn diag_log(level: &str, message: impl Into<String>) {
    let msg = message.into();
    expansion_diagnostics::push(level, msg.clone());
    match level {
        "error" => log::error!("{msg}"),
        "warn" => log::warn!("{msg}"),
        _ => log::info!("{msg}"),
    }
}

pub(crate) fn enforce_appearance_transparency_rules() {
    let now = std::time::Instant::now();
    let storage = JsonFileStorageAdapter::load();
    let rules = load_appearance_rules(&storage);
    let effective = effective_rules_for_enforcement(rules);
    if effective.is_empty() {
        return;
    }
    log::debug!("[Appearance] Starting enforcement for {} rules", effective.len());
    for rule in effective {
        let _ = apply_process_transparency(
            &rule.app_process,
            Some(rule.opacity.clamp(20, 255) as u8),
        );
    }
    log::debug!("[Appearance] Enforcement cycle completed in {:?}", now.elapsed());
}

#[cfg(target_os = "windows")]
fn transparency_cache() -> &'static Mutex<HashMap<isize, u8>> {
    static CACHE: OnceLock<Mutex<HashMap<isize, u8>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "windows")]
struct TransparencyApplyContext {
    target_pids: std::collections::HashSet<u32>,
    alpha: Option<u8>,
    applied: u32,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_apply_transparency(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if lparam.0 == 0 {
        return BOOL(1);
    }
    let ctx = &mut *(lparam.0 as *mut TransparencyApplyContext);
    
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
    if !IsWindow(Some(hwnd)).as_bool() {
        return BOOL(1);
    }

    let hwnd_key = hwnd.0 as isize;
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || !ctx.target_pids.contains(&pid) {
        return BOOL(1);
    }
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if let Some(alpha) = ctx.alpha {
        if let Ok(cache) = transparency_cache().lock() {
            if cache.get(&hwnd_key).copied() == Some(alpha) {
                return BOOL(1);
            }
        }
        let mut next_style = ex_style;
        if (next_style & WS_EX_LAYERED.0 as i32) == 0 {
            next_style |= WS_EX_LAYERED.0 as i32;
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, next_style);
        }
        let mut current_color_key = COLORREF(0);
        let mut current_alpha: u8 = 0;
        let mut current_flags = windows::Win32::UI::WindowsAndMessaging::LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
        let has_current_alpha = GetLayeredWindowAttributes(
            hwnd,
            Some(&mut current_color_key),
            Some(&mut current_alpha),
            Some(&mut current_flags),
        )
        .is_ok()
            && (current_flags.0 & LWA_ALPHA.0) != 0;
        if !has_current_alpha || current_alpha != alpha {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        }
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.insert(hwnd_key, alpha);
        }
    } else if (ex_style & WS_EX_LAYERED.0 as i32) != 0 {
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style & !(WS_EX_LAYERED.0 as i32));
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.remove(&hwnd_key);
        }
    }
    ctx.applied = ctx.applied.saturating_add(1);
    BOOL(1)
}

#[cfg(target_os = "windows")]
fn process_name_matches(target: &str, name: &str) -> bool {
    let t = normalize_process_key(target);
    let n = normalize_process_key(name);
    !t.is_empty() && t == n
}

#[cfg(target_os = "windows")]
fn apply_process_transparency(app_process: &str, alpha: Option<u8>) -> Result<u32, String> {
    use sysinfo::{ProcessesToUpdate, System};
    let target = app_process.trim();
    if target.is_empty() {
        return Ok(0);
    }
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let target_pids: std::collections::HashSet<u32> = sys
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy();
            if process_name_matches(target, &name) {
                Some(pid.as_u32())
            } else {
                None
            }
        })
        .collect();

    if target_pids.is_empty() {
        return Ok(0);
    }

    let mut ctx = TransparencyApplyContext {
        target_pids,
        alpha,
        applied: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_apply_transparency),
            LPARAM((&mut ctx as *mut TransparencyApplyContext) as isize),
        );
    }
    Ok(ctx.applied)
}

#[cfg(not(target_os = "windows"))]
fn apply_process_transparency(_app_process: &str, _alpha: Option<u8>) -> Result<u32, String> {
    Ok(0)
}

#[cfg(target_os = "windows")]
fn get_running_process_names() -> Vec<String> {
    use std::collections::BTreeSet;
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut names = BTreeSet::new();
    for process in sys.processes().values() {
        let mut name = process.name().to_string_lossy().trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if !name.ends_with(".exe") {
            name.push_str(".exe");
        }
        names.insert(name);
    }
    names.into_iter().collect()
}

#[cfg(not(target_os = "windows"))]
fn get_running_process_names() -> Vec<String> {
    Vec::new()
}

/// Persists app state to JSON storage. Used by save_settings and save_all_on_exit.
fn persist_settings_to_storage(state: &AppState) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::LIBRARY_PATH, &state.library_path);
    storage.set(storage_keys::SYNC_URL, &state.sync_url);
    storage.set(
        storage_keys::TEMPLATE_DATE_FORMAT,
        &state.template_date_format,
    );
    storage.set(
        storage_keys::TEMPLATE_TIME_FORMAT,
        &state.template_time_format,
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_DISABLED,
        &state.script_library_run_disabled.to_string(),
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST,
        &state.script_library_run_allowlist,
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS,
        &state.ghost_suggestor_display_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_SNOOZE_DURATION_MINS,
        &state.ghost_suggestor_snooze_duration_mins.to_string(),
    );
    storage.set(
        storage_keys::CLIP_HISTORY_MAX_DEPTH,
        &state.clip_history_max_depth.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_ENABLED,
        &state.ghost_follower_enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EDGE_RIGHT,
        &state.ghost_follower_edge_right.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR,
        &state.ghost_follower_monitor_anchor.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW,
        &state.ghost_follower_hover_preview.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS,
        &state.ghost_follower_collapse_delay_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_OPACITY,
        &state.ghost_follower_opacity.to_string(),
    );
    if let Some((px, py)) = state.ghost_follower_position {
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, &px.to_string());
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, &py.to_string());
    }
    storage.set(
        storage_keys::EXPANSION_PAUSED,
        &state.expansion_paused.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_ENABLED,
        &state.ghost_suggestor_enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_DEBOUNCE_MS,
        &state.ghost_suggestor_debounce_ms.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_OFFSET_X,
        &state.ghost_suggestor_offset_x.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_OFFSET_Y,
        &state.ghost_suggestor_offset_y.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_ENABLED,
        &state.discovery_enabled.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_THRESHOLD,
        &state.discovery_threshold.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_LOOKBACK,
        &state.discovery_lookback.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_MIN_LEN,
        &state.discovery_min_len.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_MAX_LEN,
        &state.discovery_max_len.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_EXCLUDED_APPS,
        &state.discovery_excluded_apps,
    );
    storage.set(
        storage_keys::DISCOVERY_EXCLUDED_WINDOW_TITLES,
        &state.discovery_excluded_window_titles,
    );
    storage.persist().map_err(|e| e.to_string())
}

/// Persist only settings from current shared AppState.
pub fn persist_settings_for_state(state: &Arc<Mutex<AppState>>) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    persist_settings_to_storage(&*guard)
}

/// Saves settings and library to disk. Call on app exit to persist unsaved changes.
pub fn save_all_on_exit(state: &Arc<Mutex<AppState>>) {
    if let Ok(mut guard) = state.lock() {
        if let Err(e) = persist_settings_to_storage(&*guard) {
            log::warn!("[Exit] persist_settings failed: {}", e);
        }
        if let Err(e) = guard.try_save_library() {
            log::warn!("[Exit] try_save_library failed: {}", e);
        }
    }
}

// Export to frontend src/ (outside src-tauri) to avoid watcher rebuild loop
#[taurpc::procedures(export_to = "../src/bindings.ts")]
pub trait Api {
    async fn greet(name: String) -> String;
    async fn get_app_state() -> Result<crate::AppStateDto, String>;
    async fn load_library() -> Result<u32, String>;
    async fn save_library() -> Result<(), String>;
    async fn set_library_path(path: String) -> Result<(), String>;
    async fn save_settings() -> Result<(), String>;
    async fn get_ui_prefs() -> Result<UiPrefsDto, String>;
    async fn save_ui_prefs(last_tab: u32, column_order: Vec<String>) -> Result<(), String>;
    async fn add_snippet(category: String, snippet: Snippet) -> Result<(), String>;
    async fn update_snippet(
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String>;
    async fn delete_snippet(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn update_config(config: ConfigUpdateDto) -> Result<(), String>;
    async fn get_clipboard_entries() -> Result<Vec<ClipEntryDto>, String>;
    async fn search_clipboard_entries(
        search: String,
        operator: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ClipEntryDto>, String>;
    async fn delete_clip_entry(index: u32) -> Result<(), String>;
    async fn delete_clip_entry_by_id(id: u32) -> Result<(), String>;
    async fn clear_clipboard_history() -> Result<(), String>;
    async fn get_copy_to_clipboard_config() -> Result<CopyToClipboardConfigDto, String>;
    async fn save_copy_to_clipboard_config(config: CopyToClipboardConfigDto) -> Result<(), String>;
    async fn get_copy_to_clipboard_stats() -> Result<CopyToClipboardStatsDto, String>;
    async fn copy_to_clipboard(text: String) -> Result<(), String>;
    async fn copy_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn save_clipboard_image_by_id(id: u32, path: String) -> Result<(), String>;
    async fn open_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn get_script_library_js() -> Result<String, String>;
    async fn save_script_library_js(content: String) -> Result<(), String>;
    async fn get_script_library_py() -> Result<String, String>;
    async fn save_script_library_py(content: String) -> Result<(), String>;
    async fn get_script_library_lua() -> Result<String, String>;
    async fn save_script_library_lua(content: String) -> Result<(), String>;
    async fn get_scripting_engine_config() -> Result<ScriptingEngineConfigDto, String>;
    async fn save_scripting_engine_config(config: ScriptingEngineConfigDto) -> Result<(), String>;
    async fn get_appearance_transparency_rules() -> Result<Vec<AppearanceTransparencyRuleDto>, String>;
    async fn get_running_process_names() -> Result<Vec<String>, String>;
    async fn export_settings_bundle_to_file(
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String>;
    async fn preview_settings_bundle_from_file(path: String) -> Result<SettingsBundlePreviewDto, String>;
    async fn import_settings_bundle_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String>;
    async fn export_scripting_profile_to_file(path: String, selected_groups: Vec<String>) -> Result<u32, String>;
    async fn export_scripting_profile_with_detached_signature_to_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingDetachedSignatureExportDto, String>;
    async fn preview_scripting_profile_from_file(path: String) -> Result<ScriptingProfilePreviewDto, String>;
    async fn dry_run_import_scripting_profile_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileDryRunDto, String>;
    async fn import_scripting_profile_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileImportResultDto, String>;
    async fn get_scripting_signer_registry() -> Result<ScriptingSignerRegistryDto, String>;
    async fn save_scripting_signer_registry(registry: ScriptingSignerRegistryDto) -> Result<(), String>;
    async fn save_appearance_transparency_rule(app_process: String, opacity: u32, enabled: bool) -> Result<(), String>;
    async fn delete_appearance_transparency_rule(app_process: String) -> Result<(), String>;
    async fn apply_appearance_transparency_now(app_process: String, opacity: u32) -> Result<u32, String>;
    async fn restore_appearance_defaults() -> Result<u32, String>;
    async fn get_ghost_suggestor_state() -> Result<GhostSuggestorStateDto, String>;
    async fn ghost_suggestor_accept() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_snooze() -> Result<(), String>;
    async fn ghost_suggestor_dismiss() -> Result<(), String>;
    async fn ghost_suggestor_ignore(phrase: String) -> Result<(), String>;
    async fn ghost_suggestor_create_snippet() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_cycle_forward() -> Result<u32, String>;
    async fn get_ghost_follower_state(search_filter: Option<String>)
        -> Result<GhostFollowerStateDto, String>;
    async fn ghost_follower_insert(trigger: String, content: String) -> Result<(), String>;
    async fn ghost_follower_set_search(filter: String) -> Result<(), String>;
    async fn bring_main_window_to_foreground() -> Result<(), String>;
    async fn ghost_follower_restore_always_on_top() -> Result<(), String>;
    async fn ghost_follower_capture_target_window() -> Result<(), String>;
    async fn ghost_follower_touch() -> Result<(), String>;
    async fn ghost_follower_set_collapsed(collapsed: bool) -> Result<(), String>;
    async fn ghost_follower_set_size(width: f64, height: f64) -> Result<(), String>;
    async fn ghost_follower_set_opacity(opacity_pct: u32) -> Result<(), String>;
    async fn ghost_follower_save_position(x: i32, y: i32) -> Result<(), String>;
    async fn ghost_follower_hide() -> Result<(), String>;
    async fn ghost_follower_request_view_full(content: String) -> Result<(), String>;
    async fn ghost_follower_request_edit(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn ghost_follower_request_promote(content: String, trigger: String) -> Result<(), String>;
    async fn ghost_follower_toggle_pin(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn get_pending_variable_input() -> Result<Option<PendingVarDto>, String>;
    async fn submit_variable_input(values: HashMap<String, String>) -> Result<(), String>;
    async fn cancel_variable_input() -> Result<(), String>;
    async fn get_expansion_stats() -> Result<ExpansionStatsDto, String>;
    async fn reset_expansion_stats() -> Result<(), String>;
    async fn get_diagnostic_logs() -> Result<Vec<DiagnosticEntryDto>, String>;
    async fn clear_diagnostic_logs() -> Result<(), String>;
    async fn test_snippet_logic(
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String>;
    async fn get_weather_location_suggestions(
        city_query: String,
        country: Option<String>,
        region: Option<String>,
    ) -> Result<Vec<String>, String>;
}

#[derive(Clone)]
pub struct ApiImpl {
    pub state: Arc<Mutex<digicore_text_expander::application::app_state::AppState>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
}

fn get_app(app: &Arc<Mutex<Option<AppHandle>>>) -> AppHandle {
    app.lock()
        .unwrap()
        .clone()
        .expect("AppHandle not yet set (setup not run)")
}

fn bring_main_to_foreground(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Lowers Ghost Follower's always_on_top so the main window can appear above it,
/// then brings the main window to foreground. Call ghost_follower_restore_always_on_top
/// when the modal is closed.
fn bring_main_to_foreground_above_ghost_follower(app: &AppHandle) {
    if let Some(ghost) = app.get_webview_window("ghost-follower") {
        let _ = ghost.set_always_on_top(false);
    }
    bring_main_to_foreground(app);
}

fn var_type_to_string(t: &InteractiveVarType) -> &'static str {
    match t {
        InteractiveVarType::Edit => "edit",
        InteractiveVarType::Choice => "choice",
        InteractiveVarType::Checkbox => "checkbox",
        InteractiveVarType::DatePicker => "date_picker",
        InteractiveVarType::FilePicker => "file_picker",
    }
}

pub(crate) fn sync_runtime_clipboard_entries_to_sqlite() {
    let entries = clipboard_history::get_entries();
    if entries.is_empty() {
        sync_current_clipboard_image_to_sqlite(String::new(), String::new());
        return;
    }
    for entry in entries.into_iter().rev() {
        let _ = persist_clipboard_entry_with_settings(
            &entry.content,
            &entry.process_name,
            &entry.window_title,
        );
    }
    sync_current_clipboard_image_to_sqlite(String::new(), String::new());
}

pub(crate) fn sync_current_clipboard_image_to_sqlite(process_name: String, window_title: String) {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled {
        return;
    }
    let image = match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
        Ok(img) => img,
        Err(_) => return,
    };
    if image.width == 0 || image.height == 0 || image.bytes.is_empty() {
        return;
    }
    let inserted = clipboard_repository::insert_image_entry(
        image.bytes.as_ref(),
        image.width as u32,
        image.height as u32,
        &process_name,
        &window_title,
        Some("image/png"),
        &cfg.image_storage_dir,
    )
    .unwrap_or(false);
    if inserted {
        if cfg.max_history_entries > 0 {
            let _ = clipboard_repository::trim_to_depth(cfg.max_history_entries);
        }
        diag_log("info", "[Clipboard][capture.image] persisted clipboard image");
    }
}

fn open_file_in_default_app(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Image path is empty.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Open image not supported on this platform.".to_string())
}

#[taurpc::resolvers]
impl Api for ApiImpl {
    async fn greet(self, name: String) -> String {
        format!("Hello, {}! DigiCore Text Expander backend ready.", name)
    }

    async fn get_app_state(self) -> Result<crate::AppStateDto, String> {
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        Ok(app_state_to_dto(&guard))
    }

    async fn load_library(self) -> Result<u32, String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        let count = guard.try_load_library().map_err(|e| e.to_string())? as u32;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(count)
    }

    async fn save_library(self) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.try_save_library().map_err(|e| e.to_string())
    }

    async fn set_library_path(self, path: String) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.library_path = path;
        Ok(())
    }

    async fn save_settings(self) -> Result<(), String> {
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        persist_settings_to_storage(&*guard)
    }

    async fn get_ui_prefs(self) -> Result<UiPrefsDto, String> {
        let storage = JsonFileStorageAdapter::load();
        let last_tab: u32 = storage
            .get(storage_keys::UI_LAST_TAB)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let column_order: Vec<String> = storage
            .get(storage_keys::UI_COLUMN_ORDER)
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_else(|| {
                vec![
                    "Profile".into(),
                    "Category".into(),
                    "Trigger".into(),
                    "Content Preview".into(),
                    "AppLock".into(),
                    "Options".into(),
                    "Last Modified".into(),
                ]
            });
        Ok(UiPrefsDto {
            last_tab,
            column_order,
        })
    }

    async fn save_ui_prefs(self, last_tab: u32, column_order: Vec<String>) -> Result<(), String> {
        let mut storage = JsonFileStorageAdapter::load();
        storage.set(storage_keys::UI_LAST_TAB, &last_tab.to_string());
        storage.set(storage_keys::UI_COLUMN_ORDER, &column_order.join(","));
        storage.persist().map_err(|e| e.to_string())
    }

    async fn add_snippet(self, category: String, snippet: Snippet) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.add_snippet(&category, &snippet);
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn update_snippet(
        self,
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard
            .update_snippet(&category, snippet_idx as usize, &snippet)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn delete_snippet(self, category: String, snippet_idx: u32) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard
            .delete_snippet(&category, snippet_idx as usize)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn update_config(self, config: ConfigUpdateDto) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(v) = config.expansion_paused {
            guard.expansion_paused = v;
        }
        if let Some(ref v) = config.template_date_format {
            guard.template_date_format = v.clone();
        }
        if let Some(ref v) = config.template_time_format {
            guard.template_time_format = v.clone();
        }
        if let Some(ref v) = config.sync_url {
            guard.sync_url = v.clone();
        }
        if let Some(v) = config.discovery_enabled {
            guard.discovery_enabled = v;
        }
        if let Some(v) = config.discovery_threshold {
            guard.discovery_threshold = v;
        }
        if let Some(v) = config.discovery_lookback {
            guard.discovery_lookback = v;
        }
        if let Some(v) = config.discovery_min_len {
            guard.discovery_min_len = v as usize;
        }
        if let Some(v) = config.discovery_max_len {
            guard.discovery_max_len = v as usize;
        }
        if let Some(ref v) = config.discovery_excluded_apps {
            guard.discovery_excluded_apps = v.clone();
        }
        if let Some(ref v) = config.discovery_excluded_window_titles {
            guard.discovery_excluded_window_titles = v.clone();
        }
        if let Some(v) = config.ghost_suggestor_enabled {
            guard.ghost_suggestor_enabled = v;
        }
        if let Some(v) = config.ghost_suggestor_debounce_ms {
            guard.ghost_suggestor_debounce_ms = v as u64;
        }
        if let Some(v) = config.ghost_suggestor_display_secs {
            guard.ghost_suggestor_display_secs = v as u64;
        }
        if let Some(v) = config.ghost_suggestor_snooze_duration_mins {
            guard.ghost_suggestor_snooze_duration_mins = v.clamp(1, 120) as u64;
        }
        if let Some(v) = config.ghost_suggestor_offset_x {
            guard.ghost_suggestor_offset_x = v;
        }
        if let Some(v) = config.ghost_suggestor_offset_y {
            guard.ghost_suggestor_offset_y = v;
        }
        if let Some(v) = config.ghost_follower_enabled {
            guard.ghost_follower_enabled = v;
        }
        if let Some(v) = config.ghost_follower_edge_right {
            guard.ghost_follower_edge_right = v;
        }
        if let Some(v) = config.ghost_follower_monitor_anchor {
            guard.ghost_follower_monitor_anchor = v as usize;
        }
        if let Some(ref v) = config.ghost_follower_search {
            guard.ghost_follower_search = v.clone();
        }
        if let Some(v) = config.ghost_follower_hover_preview {
            guard.ghost_follower_hover_preview = v;
        }
        if let Some(v) = config.ghost_follower_collapse_delay_secs {
            guard.ghost_follower_collapse_delay_secs = v as u64;
        }
        if let Some(v) = config.ghost_follower_opacity {
            guard.ghost_follower_opacity = v.clamp(10, 100);
        }
        if let Some(v) = config.clip_history_max_depth {
            let depth = v as usize;
            guard.clip_history_max_depth = depth;
            clipboard_history::update_config(ClipboardHistoryConfig {
                enabled: true,
                max_depth: if depth == 0 { usize::MAX } else { depth },
            });
            let storage = JsonFileStorageAdapter::load();
            let mut copy_cfg = load_copy_to_clipboard_config(&storage, depth as u32);
            copy_cfg.max_history_entries = depth as u32;
            let _ = save_copy_to_clipboard_config(&copy_cfg);
            if depth > 0 {
                let _ = clipboard_repository::trim_to_depth(depth as u32);
            }
        }
        if let Some(v) = config.script_library_run_disabled {
            guard.script_library_run_disabled = v;
        }
        if let Some(ref v) = config.script_library_run_allowlist {
            guard.script_library_run_allowlist = v.clone();
        }
        sync_discovery_config(
            guard.discovery_enabled,
            discovery::DiscoveryConfig {
                threshold: guard.discovery_threshold,
                lookback_minutes: guard.discovery_lookback,
                min_phrase_len: guard.discovery_min_len,
                max_phrase_len: guard.discovery_max_len,
                excluded_apps: guard
                    .discovery_excluded_apps
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                excluded_window_titles: guard
                    .discovery_excluded_window_titles
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
            },
        );
        sync_ghost_config(GhostConfig {
            suggestor_enabled: guard.ghost_suggestor_enabled,
            suggestor_debounce_ms: guard.ghost_suggestor_debounce_ms,
            suggestor_display_secs: guard.ghost_suggestor_display_secs,
            suggestor_snooze_duration_mins: guard.ghost_suggestor_snooze_duration_mins,
            suggestor_offset_x: guard.ghost_suggestor_offset_x,
            suggestor_offset_y: guard.ghost_suggestor_offset_y,
            follower_enabled: guard.ghost_follower_enabled,
            follower_edge_right: guard.ghost_follower_edge_right,
            follower_monitor_anchor: guard.ghost_follower_monitor_anchor,
            follower_search: guard.ghost_follower_search.clone(),
            follower_hover_preview: guard.ghost_follower_hover_preview,
            follower_collapse_delay_secs: guard.ghost_follower_collapse_delay_secs,
        });
        set_expansion_paused(guard.expansion_paused);
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn get_clipboard_entries(self) -> Result<Vec<ClipEntryDto>, String> {
        let rows = clipboard_repository::list_entries(None, 500)?;
        Ok(rows
            .into_iter()
            .map(|r| ClipEntryDto {
                id: r.id,
                content: r.content,
                process_name: r.process_name,
                window_title: r.window_title,
                length: r.char_count,
                word_count: r.word_count,
                created_at: r.created_at_unix_ms.to_string(),
                entry_type: r.entry_type,
                mime_type: r.mime_type,
                image_path: r.image_path,
                thumb_path: r.thumb_path,
                image_width: r.image_width,
                image_height: r.image_height,
                image_bytes: r.image_bytes,
            })
            .collect())
    }

    async fn search_clipboard_entries(
        self,
        search: String,
        operator: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ClipEntryDto>, String> {
        let rows = clipboard_repository::search_entries(
            &search,
            operator.as_deref(),
            limit.unwrap_or(500),
        )?;
        Ok(rows
            .into_iter()
            .map(|r| ClipEntryDto {
                id: r.id,
                content: r.content,
                process_name: r.process_name,
                window_title: r.window_title,
                length: r.char_count,
                word_count: r.word_count,
                created_at: r.created_at_unix_ms.to_string(),
                entry_type: r.entry_type,
                mime_type: r.mime_type,
                image_path: r.image_path,
                thumb_path: r.thumb_path,
                image_width: r.image_width,
                image_height: r.image_height,
                image_bytes: r.image_bytes,
            })
            .collect())
    }

    async fn delete_clip_entry(self, index: u32) -> Result<(), String> {
        let rows = clipboard_repository::list_entries(None, index.saturating_add(1))?;
        if let Some(row) = rows.get(index as usize) {
            clipboard_repository::delete_entry_by_id(row.id)?;
            diag_log(
                "info",
                format!("[Clipboard][delete] removed entry id={} via index", row.id),
            );
        }
        clipboard_history::delete_entry_at(index as usize);
        Ok(())
    }

    async fn delete_clip_entry_by_id(self, id: u32) -> Result<(), String> {
        clipboard_repository::delete_entry_by_id(id)?;
        diag_log("info", format!("[Clipboard][delete] removed entry id={id}"));
        Ok(())
    }

    async fn clear_clipboard_history(self) -> Result<(), String> {
        clipboard_repository::clear_all()?;
        clipboard_history::clear_all();
        diag_log("info", "[Clipboard][clear] cleared all clipboard history");
        Ok(())
    }

    async fn get_copy_to_clipboard_config(self) -> Result<CopyToClipboardConfigDto, String> {
        let storage = JsonFileStorageAdapter::load();
        let max_depth = storage
            .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(20);
        Ok(load_copy_to_clipboard_config(&storage, max_depth))
    }

    async fn save_copy_to_clipboard_config(self, config: CopyToClipboardConfigDto) -> Result<(), String> {
        let mut normalized = config;
        normalized.min_log_length = normalized.min_log_length.clamp(1, 2000);
        let default_cfg = default_copy_to_clipboard_config(normalized.max_history_entries);
        let json_root = normalize_clipboard_path_or_default(
            &normalized.json_output_dir,
            PathBuf::from(&default_cfg.json_output_dir),
        );
        let image_root = normalize_clipboard_path_or_default(
            &normalized.image_storage_dir,
            PathBuf::from(&default_cfg.image_storage_dir),
        );
        normalized.json_output_dir = json_root.to_string_lossy().to_string();
        normalized.image_storage_dir = image_root.to_string_lossy().to_string();
        std::fs::create_dir_all(&normalized.json_output_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(PathBuf::from(&normalized.image_storage_dir).join("full"))
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(PathBuf::from(&normalized.image_storage_dir).join("thumbs"))
            .map_err(|e| e.to_string())?;
        let current = self.clone().get_copy_to_clipboard_config().await?;
        save_copy_to_clipboard_config(&normalized)?;
        let migrated_assets = if current.image_storage_dir.trim() != normalized.image_storage_dir.trim() {
            clipboard_repository::migrate_image_assets_root(
                &current.image_storage_dir,
                &normalized.image_storage_dir,
            )?
        } else {
            0
        };
        {
            let mut guard = self.state.lock().map_err(|e| e.to_string())?;
            guard.clip_history_max_depth = normalized.max_history_entries as usize;
            clipboard_history::update_config(ClipboardHistoryConfig {
                enabled: normalized.enabled,
                max_depth: if normalized.max_history_entries == 0 {
                    usize::MAX
                } else {
                    normalized.max_history_entries as usize
                },
            });
        }
        let trimmed = if normalized.max_history_entries > 0 {
            clipboard_repository::trim_to_depth(normalized.max_history_entries).unwrap_or(0)
        } else {
            0
        };
        diag_log(
            "info",
            format!(
                "[Clipboard][config] saved enabled={} min_len={} max_entries={} trimmed={} migrated_assets={}",
                normalized.enabled,
                normalized.min_log_length,
                normalized.max_history_entries,
                trimmed,
                migrated_assets
            ),
        );
        Ok(())
    }

    async fn get_copy_to_clipboard_stats(self) -> Result<CopyToClipboardStatsDto, String> {
        Ok(CopyToClipboardStatsDto {
            total_entries: clipboard_repository::count()?,
        })
    }

    async fn copy_to_clipboard(self, text: String) -> Result<(), String> {
        arboard::Clipboard::new()
            .map_err(|e| e.to_string())?
            .set_text(&text)
            .map_err(|e| e.to_string())?;
        diag_log("info", "[Clipboard][copy] copied text to system clipboard");
        Ok(())
    }

    async fn copy_clipboard_image_by_id(self, id: u32) -> Result<(), String> {
        let row = clipboard_repository::get_entry_by_id(id)?
            .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
        if row.entry_type != "image" {
            return Err("Selected clipboard entry is not an image.".to_string());
        }
        let image_path = row
            .image_path
            .ok_or_else(|| "Image file path is missing.".to_string())?;
        let img = image::open(&image_path).map_err(|e| e.to_string())?.to_rgba8();
        let width = img.width() as usize;
        let height = img.height() as usize;
        let bytes = img.into_raw();
        arboard::Clipboard::new()
            .map_err(|e| e.to_string())?
            .set_image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(bytes),
            })
            .map_err(|e| e.to_string())?;
        diag_log("info", format!("[Clipboard][copy.image] copied image id={id}"));
        Ok(())
    }

    async fn save_clipboard_image_by_id(self, id: u32, path: String) -> Result<(), String> {
        let row = clipboard_repository::get_entry_by_id(id)?
            .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
        if row.entry_type != "image" {
            return Err("Selected clipboard entry is not an image.".to_string());
        }
        let src = row
            .image_path
            .ok_or_else(|| "Image file path is missing.".to_string())?;
        std::fs::copy(src, &path).map_err(|e| e.to_string())?;
        diag_log("info", format!("[Clipboard][save.image] saved image id={} to {}", id, path));
        Ok(())
    }

    async fn open_clipboard_image_by_id(self, id: u32) -> Result<(), String> {
        let row = clipboard_repository::get_entry_by_id(id)?
            .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
        if row.entry_type != "image" {
            return Err("Selected clipboard entry is not an image.".to_string());
        }
        let image_path = row
            .image_path
            .ok_or_else(|| "Image file path is missing.".to_string())?;
        open_file_in_default_app(&image_path)?;
        diag_log("info", format!("[Clipboard][open.image] opened image id={id}"));
        Ok(())
    }

    async fn get_script_library_js(self) -> Result<String, String> {
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

    async fn save_script_library_js(self, content: String) -> Result<(), String> {
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
        set_global_library(content);
        diag_log(
            "info",
            format!("[Scripting][JavaScript] Saved global library to {}", lib_path.display()),
        );
        Ok(())
    }

    async fn get_script_library_py(self) -> Result<String, String> {
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

    async fn save_script_library_py(self, content: String) -> Result<(), String> {
        let cfg = get_scripting_config();
        let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
        let lib_path = base.join(&cfg.py.library_path);
        if let Some(parent) = lib_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
        diag_log(
            "info",
            format!("[Scripting][Python] Saved global library to {}", lib_path.display()),
        );
        Ok(())
    }

    async fn get_script_library_lua(self) -> Result<String, String> {
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

    async fn save_script_library_lua(self, content: String) -> Result<(), String> {
        let cfg = get_scripting_config();
        let base = digicore_text_expander::ports::data_path_resolver::DataPathResolver::root();
        let lib_path = base.join(&cfg.lua.library_path);
        if let Some(parent) = lib_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
        diag_log(
            "info",
            format!("[Scripting][Lua] Saved global library to {}", lib_path.display()),
        );
        Ok(())
    }

    async fn get_scripting_engine_config(self) -> Result<ScriptingEngineConfigDto, String> {
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

    async fn save_scripting_engine_config(self, config: ScriptingEngineConfigDto) -> Result<(), String> {
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
            diag_log(
                "warn",
                format!(
                    "[Scripting][HTTP] Clamped settings timeout={} retry_count={} retry_delay_ms={}",
                    timeout_clamped, retry_count_clamped, retry_delay_clamped
                ),
            );
        }
        diag_log(
            "info",
            format!(
                "[Scripting][Config] Saved dsl_enabled={} http_async={} py_enabled={} lua_enabled={}",
                config.dsl.enabled, config.http.use_async, config.py.enabled, config.lua.enabled
            ),
        );
        Ok(())
    }

    async fn export_scripting_profile_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<u32, String> {
        let groups = normalized_selected_scripting_profile_groups(&selected_groups);
        if groups.is_empty() {
            return Err("No valid scripting profile groups selected for export.".to_string());
        }

        let js_library = self.clone().get_script_library_js().await?;
        let py_library = self.clone().get_script_library_py().await?;
        let lua_library = self.clone().get_script_library_lua().await?;
        let cfg = get_scripting_config();
        let guard = self.state.lock().map_err(|e| e.to_string())?;

        let mut groups_obj = serde_json::Map::new();
        for group in &groups {
            match group.as_str() {
                SCRIPTING_PROFILE_GROUP_JAVASCRIPT => {
                    groups_obj.insert(group.clone(), serde_json::json!({ "library": js_library }));
                }
                SCRIPTING_PROFILE_GROUP_PYTHON => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "library": py_library,
                            "enabled": cfg.py.enabled,
                            "path": cfg.py.path,
                            "library_path": cfg.py.library_path
                        }),
                    );
                }
                SCRIPTING_PROFILE_GROUP_LUA => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "library": lua_library,
                            "enabled": cfg.lua.enabled,
                            "path": cfg.lua.path,
                            "library_path": cfg.lua.library_path
                        }),
                    );
                }
                SCRIPTING_PROFILE_GROUP_HTTP => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "timeout_secs": cfg.http.timeout_secs.clamp(1, 60) as u32,
                            "retry_count": cfg.http.retry_count.min(10),
                            "retry_delay_ms": cfg.http.retry_delay_ms.clamp(50, 20_000) as u32,
                            "use_async": cfg.http.use_async
                        }),
                    );
                }
                SCRIPTING_PROFILE_GROUP_DSL => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "enabled": cfg.dsl.enabled
                        }),
                    );
                }
                SCRIPTING_PROFILE_GROUP_RUN => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "script_library_run_disabled": guard.script_library_run_disabled,
                            "script_library_run_allowlist": guard.script_library_run_allowlist
                        }),
                    );
                }
                _ => {}
            }
        }

        let sign_payload = serde_json::json!({
            "schema_version": SCRIPTING_PROFILE_SCHEMA_V2,
            "selected_groups": groups,
            "groups": groups_obj
        });
        let sign_bytes = serde_json::to_vec(&sign_payload).map_err(|e| e.to_string())?;
        let payload_sha256 = sha256_hex(&sign_bytes);

        let signing_key = get_or_create_scripting_profile_signing_key()?;
        let verifying_key = signing_key.verifying_key();
        let signer_fingerprint = sha256_hex(&verifying_key.to_bytes());
        let signature = signing_key.sign(&sign_bytes);
        let key_id = {
            signer_fingerprint.chars().take(16).collect::<String>()
        };

        let payload = serde_json::json!({
            "schema_version": SCRIPTING_PROFILE_SCHEMA_V2,
            "exported_at_utc": now_unix_secs_string(),
            "app": {
                "name": "DigiCore Text Expander",
                "format": "scripting-engine-profile"
            },
            "selected_groups": groups,
            "groups": groups_obj,
            "integrity": {
                "algorithm": SCRIPTING_PROFILE_SIGN_ALGO,
                "key_id": key_id,
                "public_key_b64": base64::engine::general_purpose::STANDARD.encode(verifying_key.to_bytes()),
                "signer_fingerprint": signer_fingerprint,
                "payload_sha256": payload_sha256,
                "signature_b64": base64::engine::general_purpose::STANDARD.encode(signature.to_bytes())
            }
        });

        let serialized = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        std::fs::write(&path, serialized).map_err(|e| e.to_string())?;
        diag_log(
            "info",
            format!(
                "[ScriptingProfileExport] Wrote scripting engine profile to {}",
                path
            ),
        );
        Ok(payload["selected_groups"]
            .as_array()
            .map(|a| a.len() as u32)
            .unwrap_or(0))
    }

    async fn export_scripting_profile_with_detached_signature_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingDetachedSignatureExportDto, String> {
        self.clone()
            .export_scripting_profile_to_file(path.clone(), selected_groups)
            .await?;

        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let integrity = root
            .get("integrity")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "Exported profile missing integrity block.".to_string())?;

        let key_id = integrity
            .get("key_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let signer_fingerprint = integrity
            .get("signer_fingerprint")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let payload_sha256 = integrity
            .get("payload_sha256")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let signature_path = if path.to_ascii_lowercase().ends_with(".json") {
            format!("{}.sig.json", &path[..path.len() - 5])
        } else {
            format!("{path}.sig.json")
        };

        let detached = serde_json::json!({
            "schema_version": SCRIPTING_PROFILE_SCHEMA_V2,
            "format": "scripting-engine-profile-detached-signature",
            "profile_path": path,
            "exported_at_utc": now_unix_secs_string(),
            "integrity": integrity
        });
        std::fs::write(
            &signature_path,
            serde_json::to_string_pretty(&detached).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        diag_log(
            "info",
            format!(
                "[ScriptingProfileExport] Wrote detached signature to {}",
                signature_path
            ),
        );
        Ok(ScriptingDetachedSignatureExportDto {
            profile_path: path,
            signature_path,
            key_id,
            signer_fingerprint,
            payload_sha256,
        })
    }

    async fn preview_scripting_profile_from_file(
        self,
        path: String,
    ) -> Result<ScriptingProfilePreviewDto, String> {
        let mut parsed = parse_scripting_profile_bundle(&path)?;
        if parsed.signer_unknown && parsed.trust_on_first_use && parsed.valid {
            if let Some(fp) = parsed.signer_fingerprint.clone() {
                if upsert_trust_on_first_use_signer(&fp, "preview")? {
                    parsed.signer_trusted = true;
                    parsed.warnings.push(format!(
                        "TOFU auto-trusted signer '{}' on preview.",
                        normalize_fingerprint(&fp)
                    ));
                }
            }
        }
        let available_groups = parsed.groups_obj.keys().cloned().collect::<Vec<String>>();
        if parsed.warnings.is_empty() {
            diag_log(
                "info",
                format!(
                    "[ScriptingProfilePreview] OK path={} groups={}",
                    path,
                    available_groups.len()
                ),
            );
        } else {
            diag_log(
                "warn",
                format!(
                    "[ScriptingProfilePreview] path={} warnings={}",
                    path,
                    parsed.warnings.join("; ")
                ),
            );
        }

        Ok(ScriptingProfilePreviewDto {
            path,
            schema_version: parsed.schema_version_used,
            available_groups,
            warnings: parsed.warnings,
            valid: parsed.valid,
            signed_bundle: parsed.signed_bundle,
            signature_valid: parsed.signature_valid,
            migrated_from_schema: parsed.migrated_from_schema,
            signature_key_id: parsed.signature_key_id,
            signer_fingerprint: parsed.signer_fingerprint.clone(),
            signer_trusted: parsed.signer_trusted,
        })
    }

    async fn dry_run_import_scripting_profile_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileDryRunDto, String> {
        let mut parsed = parse_scripting_profile_bundle(&path)?;
        if parsed.signer_unknown && parsed.trust_on_first_use && parsed.valid {
            if let Some(fp) = parsed.signer_fingerprint.clone() {
                if upsert_trust_on_first_use_signer(&fp, "dry-run")? {
                    parsed.signer_trusted = true;
                    parsed.warnings.push(format!(
                        "TOFU auto-trusted signer '{}' during dry-run.",
                        normalize_fingerprint(&fp)
                    ));
                }
            }
        }
        let selected = if selected_groups.is_empty() {
            normalized_selected_scripting_profile_groups(&parsed.selected_groups)
        } else {
            normalized_selected_scripting_profile_groups(&selected_groups)
        };

        if !parsed.valid {
            return Ok(ScriptingProfileDryRunDto {
                path,
                selected_groups: selected,
                changed_groups: Vec::new(),
                estimated_updates: 0,
                warnings: parsed.warnings,
                diff_entries: Vec::new(),
                schema_version_used: parsed.schema_version_used,
                signature_valid: parsed.signature_valid,
                migrated_from_schema: parsed.migrated_from_schema,
                signer_fingerprint: parsed.signer_fingerprint,
                signer_trusted: parsed.signer_trusted,
            });
        }

        let current_js = self.clone().get_script_library_js().await?;
        let current_py = self.clone().get_script_library_py().await?;
        let current_lua = self.clone().get_script_library_lua().await?;
        let cfg = get_scripting_config();
        let guard = self.state.lock().map_err(|e| e.to_string())?;

        let mut changed = std::collections::BTreeSet::new();
        let mut diff_entries = Vec::<ScriptingProfileDiffEntryDto>::new();
        let mut warnings = parsed.warnings.clone();

        let mut push_diff = |group: &str, field: &str, current: String, incoming: String| {
            if current != incoming {
                changed.insert(group.to_string());
                diff_entries.push(ScriptingProfileDiffEntryDto {
                    group: group.to_string(),
                    field: field.to_string(),
                    current_value: summarize_for_diff(&current),
                    incoming_value: summarize_for_diff(&incoming),
                });
            }
        };

        for group in &selected {
            let Some(value) = parsed.groups_obj.get(group) else {
                warnings.push(format!("Group '{group}' not present in profile."));
                continue;
            };
            let Some(obj) = value.as_object() else {
                warnings.push(format!("Group '{group}' has invalid payload type."));
                continue;
            };
            match group.as_str() {
                SCRIPTING_PROFILE_GROUP_JAVASCRIPT => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        push_diff(group, "library", current_js.clone(), library.to_string());
                    }
                }
                SCRIPTING_PROFILE_GROUP_PYTHON => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        push_diff(group, "library", current_py.clone(), library.to_string());
                    }
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        push_diff(group, "enabled", cfg.py.enabled.to_string(), enabled.to_string());
                    }
                    if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                        push_diff(group, "path", cfg.py.path.clone(), path.to_string());
                    }
                    if let Some(path) = obj.get("library_path").and_then(|v| v.as_str()) {
                        push_diff(
                            group,
                            "library_path",
                            cfg.py.library_path.clone(),
                            path.to_string(),
                        );
                    }
                }
                SCRIPTING_PROFILE_GROUP_LUA => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        push_diff(group, "library", current_lua.clone(), library.to_string());
                    }
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        push_diff(group, "enabled", cfg.lua.enabled.to_string(), enabled.to_string());
                    }
                    if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                        push_diff(group, "path", cfg.lua.path.clone(), path.to_string());
                    }
                    if let Some(path) = obj.get("library_path").and_then(|v| v.as_str()) {
                        push_diff(
                            group,
                            "library_path",
                            cfg.lua.library_path.clone(),
                            path.to_string(),
                        );
                    }
                }
                SCRIPTING_PROFILE_GROUP_HTTP => {
                    if let Some(timeout) = obj.get("timeout_secs").and_then(|v| v.as_u64()) {
                        push_diff(
                            group,
                            "timeout_secs",
                            cfg.http.timeout_secs.clamp(1, 60).to_string(),
                            timeout.clamp(1, 60).to_string(),
                        );
                    }
                    if let Some(retry) = obj.get("retry_count").and_then(|v| v.as_u64()) {
                        push_diff(
                            group,
                            "retry_count",
                            cfg.http.retry_count.min(10).to_string(),
                            (retry as u32).min(10).to_string(),
                        );
                    }
                    if let Some(delay) = obj.get("retry_delay_ms").and_then(|v| v.as_u64()) {
                        push_diff(
                            group,
                            "retry_delay_ms",
                            cfg.http.retry_delay_ms.clamp(50, 20_000).to_string(),
                            delay.clamp(50, 20_000).to_string(),
                        );
                    }
                    if let Some(use_async) = obj.get("use_async").and_then(|v| v.as_bool()) {
                        push_diff(
                            group,
                            "use_async",
                            cfg.http.use_async.to_string(),
                            use_async.to_string(),
                        );
                    }
                }
                SCRIPTING_PROFILE_GROUP_DSL => {
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        push_diff(group, "enabled", cfg.dsl.enabled.to_string(), enabled.to_string());
                    }
                }
                SCRIPTING_PROFILE_GROUP_RUN => {
                    if let Some(disabled) = obj
                        .get("script_library_run_disabled")
                        .and_then(|v| v.as_bool())
                    {
                        push_diff(
                            group,
                            "script_library_run_disabled",
                            guard.script_library_run_disabled.to_string(),
                            disabled.to_string(),
                        );
                    }
                    if let Some(allowlist) = obj
                        .get("script_library_run_allowlist")
                        .and_then(|v| v.as_str())
                    {
                        push_diff(
                            group,
                            "script_library_run_allowlist",
                            guard.script_library_run_allowlist.clone(),
                            allowlist.to_string(),
                        );
                    }
                }
                _ => {}
            }
        }

        Ok(ScriptingProfileDryRunDto {
            path,
            selected_groups: selected,
            changed_groups: changed.into_iter().collect(),
            estimated_updates: diff_entries.len() as u32,
            warnings,
            diff_entries,
            schema_version_used: parsed.schema_version_used,
            signature_valid: parsed.signature_valid,
            migrated_from_schema: parsed.migrated_from_schema,
            signer_fingerprint: parsed.signer_fingerprint.clone(),
            signer_trusted: parsed.signer_trusted,
        })
    }

    async fn import_scripting_profile_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileImportResultDto, String> {
        let mut parsed = parse_scripting_profile_bundle(&path)?;
        if parsed.signer_unknown && parsed.trust_on_first_use && parsed.valid {
            if let Some(fp) = parsed.signer_fingerprint.clone() {
                if upsert_trust_on_first_use_signer(&fp, "import")? {
                    parsed.signer_trusted = true;
                    parsed.warnings.push(format!(
                        "TOFU auto-trusted signer '{}' on import.",
                        normalize_fingerprint(&fp)
                    ));
                }
            }
        }
        if !parsed.valid {
            let msg = format!(
                "Invalid scripting profile preview. {}",
                parsed.warnings.join(" | ")
            );
            diag_log("error", format!("[ScriptingProfileImport] {msg}"));
            return Err(msg);
        }

        let selected = if selected_groups.is_empty() {
            normalized_selected_scripting_profile_groups(&parsed.selected_groups)
        } else {
            normalized_selected_scripting_profile_groups(&selected_groups)
        };

        let mut result = ScriptingProfileImportResultDto {
            applied_groups: Vec::new(),
            skipped_groups: Vec::new(),
            warnings: Vec::new(),
            updated_keys: 0,
            schema_version_used: parsed.schema_version_used.clone(),
            signature_valid: parsed.signature_valid,
            migrated_from_schema: parsed.migrated_from_schema.clone(),
            signer_fingerprint: parsed.signer_fingerprint.clone(),
            signer_trusted: parsed.signer_trusted,
        };
        result.warnings.extend(parsed.warnings.clone());
        let mut cfg = get_scripting_config();
        let mut cfg_changed = false;
        let mut run_changed = false;

        for group in selected {
            let Some(value) = parsed.groups_obj.get(&group) else {
                result.skipped_groups.push(group.clone());
                result
                    .warnings
                    .push(format!("Group '{group}' not present in profile."));
                continue;
            };
            let Some(obj) = value.as_object() else {
                result.skipped_groups.push(group.clone());
                result
                    .warnings
                    .push(format!("Group '{group}' has invalid payload type."));
                continue;
            };

            match group.as_str() {
                SCRIPTING_PROFILE_GROUP_JAVASCRIPT => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        self.clone()
                            .save_script_library_js(library.to_string())
                            .await?;
                        result.updated_keys += 1;
                    } else {
                        result
                            .warnings
                            .push("Group 'javascript' missing 'library'.".to_string());
                    }
                    result.applied_groups.push(group.clone());
                }
                SCRIPTING_PROFILE_GROUP_PYTHON => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        self.clone()
                            .save_script_library_py(library.to_string())
                            .await?;
                        result.updated_keys += 1;
                    }
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        cfg.py.enabled = enabled;
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                        cfg.py.path = path.trim().to_string();
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(path) = obj.get("library_path").and_then(|v| v.as_str()) {
                        if !path.trim().is_empty() {
                            cfg.py.library_path = path.trim().to_string();
                            cfg_changed = true;
                            result.updated_keys += 1;
                        }
                    }
                    result.applied_groups.push(group.clone());
                }
                SCRIPTING_PROFILE_GROUP_LUA => {
                    if let Some(library) = obj.get("library").and_then(|v| v.as_str()) {
                        self.clone()
                            .save_script_library_lua(library.to_string())
                            .await?;
                        result.updated_keys += 1;
                    }
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        cfg.lua.enabled = enabled;
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                        cfg.lua.path = path.trim().to_string();
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(path) = obj.get("library_path").and_then(|v| v.as_str()) {
                        if !path.trim().is_empty() {
                            cfg.lua.library_path = path.trim().to_string();
                            cfg_changed = true;
                            result.updated_keys += 1;
                        }
                    }
                    result.applied_groups.push(group.clone());
                }
                SCRIPTING_PROFILE_GROUP_HTTP => {
                    if let Some(timeout_secs) = obj.get("timeout_secs").and_then(|v| v.as_u64()) {
                        cfg.http.timeout_secs = timeout_secs.clamp(1, 60);
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(retry_count) = obj.get("retry_count").and_then(|v| v.as_u64()) {
                        cfg.http.retry_count = (retry_count as u32).min(10);
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(retry_delay_ms) = obj.get("retry_delay_ms").and_then(|v| v.as_u64()) {
                        cfg.http.retry_delay_ms = retry_delay_ms.clamp(50, 20_000);
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(use_async) = obj.get("use_async").and_then(|v| v.as_bool()) {
                        cfg.http.use_async = use_async;
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    result.applied_groups.push(group.clone());
                }
                SCRIPTING_PROFILE_GROUP_DSL => {
                    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
                        cfg.dsl.enabled = enabled;
                        cfg_changed = true;
                        result.updated_keys += 1;
                    }
                    result.applied_groups.push(group.clone());
                }
                SCRIPTING_PROFILE_GROUP_RUN => {
                    let mut guard = self.state.lock().map_err(|e| e.to_string())?;
                    if let Some(disabled) = obj
                        .get("script_library_run_disabled")
                        .and_then(|v| v.as_bool())
                    {
                        guard.script_library_run_disabled = disabled;
                        run_changed = true;
                        result.updated_keys += 1;
                    }
                    if let Some(allowlist) = obj
                        .get("script_library_run_allowlist")
                        .and_then(|v| v.as_str())
                    {
                        guard.script_library_run_allowlist = allowlist.to_string();
                        run_changed = true;
                        result.updated_keys += 1;
                    }
                    if run_changed {
                        persist_settings_to_storage(&guard)?;
                    }
                    result.applied_groups.push(group.clone());
                }
                _ => {
                    result.skipped_groups.push(group.clone());
                    result
                        .warnings
                        .push(format!("Unknown scripting group '{group}' skipped."));
                }
            }
        }

        if cfg_changed {
            set_scripting_config(cfg);
        }

        if result.warnings.is_empty() {
            diag_log(
                "info",
                format!(
                    "[ScriptingProfileImport] Applied {} groups from {} schema={} signature_valid={}",
                    result.applied_groups.len(),
                    path,
                    result.schema_version_used,
                    result.signature_valid
                ),
            );
        } else {
            diag_log(
                "warn",
                format!(
                    "[ScriptingProfileImport] Applied {} groups from {} schema={} signature_valid={} with warnings: {}",
                    result.applied_groups.len(),
                    path,
                    result.schema_version_used,
                    result.signature_valid,
                    result.warnings.join("; ")
                ),
            );
        }
        Ok(result)
    }

    async fn get_scripting_signer_registry(self) -> Result<ScriptingSignerRegistryDto, String> {
        let state = load_scripting_signer_registry();
        Ok(ScriptingSignerRegistryDto {
            allow_unknown_signers: state.allow_unknown_signers,
            trust_on_first_use: state.trust_on_first_use,
            trusted_fingerprints: state.trusted_fingerprints,
            blocked_fingerprints: state.blocked_fingerprints,
        })
    }

    async fn save_scripting_signer_registry(
        self,
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
        save_scripting_signer_registry(&state)?;
        diag_log(
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

    async fn get_appearance_transparency_rules(self) -> Result<Vec<AppearanceTransparencyRuleDto>, String> {
        let storage = JsonFileStorageAdapter::load();
        let mut rules = load_appearance_rules(&storage);
        for rule in effective_rules_for_enforcement(rules.clone()) {
            let _ = apply_process_transparency(
                &rule.app_process,
                Some(rule.opacity.clamp(20, 255) as u8),
            );
        }
        sort_appearance_rules_deterministic(&mut rules);
        Ok(rules)
    }

    async fn get_running_process_names(self) -> Result<Vec<String>, String> {
        Ok(get_running_process_names())
    }

    async fn export_settings_bundle_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String> {
        let groups = normalized_selected_groups(&selected_groups);
        if groups.is_empty() {
            return Err("No valid settings groups selected for export.".to_string());
        }
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        let mut groups_obj = serde_json::Map::new();

        for group in &groups {
            match group.as_str() {
                SETTINGS_GROUP_TEMPLATES => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "template_date_format": guard.template_date_format,
                            "template_time_format": guard.template_time_format
                        }),
                    );
                }
                SETTINGS_GROUP_SYNC => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "sync_url": guard.sync_url
                        }),
                    );
                }
                SETTINGS_GROUP_DISCOVERY => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "discovery_enabled": guard.discovery_enabled,
                            "discovery_threshold": guard.discovery_threshold,
                            "discovery_lookback": guard.discovery_lookback,
                            "discovery_min_len": guard.discovery_min_len,
                            "discovery_max_len": guard.discovery_max_len,
                            "discovery_excluded_apps": guard.discovery_excluded_apps,
                            "discovery_excluded_window_titles": guard.discovery_excluded_window_titles
                        }),
                    );
                }
                SETTINGS_GROUP_GHOST_SUGGESTOR => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "ghost_suggestor_enabled": guard.ghost_suggestor_enabled,
                            "ghost_suggestor_debounce_ms": guard.ghost_suggestor_debounce_ms,
                            "ghost_suggestor_display_secs": guard.ghost_suggestor_display_secs,
                            "ghost_suggestor_snooze_duration_mins": guard.ghost_suggestor_snooze_duration_mins,
                            "ghost_suggestor_offset_x": guard.ghost_suggestor_offset_x,
                            "ghost_suggestor_offset_y": guard.ghost_suggestor_offset_y
                        }),
                    );
                }
                SETTINGS_GROUP_GHOST_FOLLOWER => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "ghost_follower_enabled": guard.ghost_follower_enabled,
                            "ghost_follower_edge_right": guard.ghost_follower_edge_right,
                            "ghost_follower_monitor_anchor": guard.ghost_follower_monitor_anchor,
                            "ghost_follower_hover_preview": guard.ghost_follower_hover_preview,
                            "ghost_follower_collapse_delay_secs": guard.ghost_follower_collapse_delay_secs,
                            "ghost_follower_opacity": guard.ghost_follower_opacity
                        }),
                    );
                }
                SETTINGS_GROUP_CLIPBOARD_HISTORY => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "clip_history_max_depth": guard.clip_history_max_depth
                        }),
                    );
                }
                SETTINGS_GROUP_COPY_TO_CLIPBOARD => {
                    let storage = JsonFileStorageAdapter::load();
                    let copy_cfg =
                        load_copy_to_clipboard_config(&storage, guard.clip_history_max_depth as u32);
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "copy_to_clipboard_enabled": copy_cfg.enabled,
                            "copy_to_clipboard_min_log_length": copy_cfg.min_log_length,
                            "copy_to_clipboard_mask_cc": copy_cfg.mask_cc,
                            "copy_to_clipboard_mask_ssn": copy_cfg.mask_ssn,
                            "copy_to_clipboard_mask_email": copy_cfg.mask_email,
                            "copy_to_clipboard_blacklist_processes": copy_cfg.blacklist_processes,
                            "copy_to_clipboard_json_output_enabled": copy_cfg.json_output_enabled,
                            "copy_to_clipboard_json_output_dir": copy_cfg.json_output_dir,
                            "copy_to_clipboard_image_storage_dir": copy_cfg.image_storage_dir,
                            "copy_to_clipboard_max_history_entries": copy_cfg.max_history_entries
                        }),
                    );
                }
                SETTINGS_GROUP_CORE => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "expansion_paused": guard.expansion_paused,
                            "theme": theme,
                            "autostart_enabled": autostart_enabled
                        }),
                    );
                }
                SETTINGS_GROUP_SCRIPT_RUNTIME => {
                    groups_obj.insert(
                        group.clone(),
                        serde_json::json!({
                            "script_library_run_disabled": guard.script_library_run_disabled,
                            "script_library_run_allowlist": guard.script_library_run_allowlist
                        }),
                    );
                }
                SETTINGS_GROUP_APPEARANCE => {
                    let storage = JsonFileStorageAdapter::load();
                    let mut rules = load_appearance_rules(&storage);
                    sort_appearance_rules_deterministic(&mut rules);
                    groups_obj.insert(group.clone(), serde_json::json!({ "rules": rules }));
                }
                _ => {}
            }
        }

        let payload = serde_json::json!({
            "schema_version": "1.0.0",
            "exported_at_utc": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string()),
            "app": {
                "name": "DigiCore Text Expander",
                "format": "settings-bundle"
            },
            "selected_groups": groups,
            "groups": groups_obj
        });

        let serialized = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        std::fs::write(&path, serialized).map_err(|e| e.to_string())?;
        diag_log("info", format!("[SettingsExport] Wrote settings bundle to {path}"));
        Ok(payload["selected_groups"].as_array().map(|a| a.len() as u32).unwrap_or(0))
    }

    async fn preview_settings_bundle_from_file(
        self,
        path: String,
    ) -> Result<SettingsBundlePreviewDto, String> {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let schema = root
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let mut warnings = Vec::new();
        let mut available_groups = Vec::new();
        let mut valid = true;

        if schema != "1.0.0" {
            valid = false;
            warnings.push(format!(
                "Unsupported schema_version '{schema}'. Expected '1.0.0'."
            ));
        }

        match root.get("groups").and_then(|v| v.as_object()) {
            Some(groups_obj) => {
                for key in groups_obj.keys() {
                    available_groups.push(key.clone());
                    if normalize_settings_group(key).is_none() {
                        warnings.push(format!("Unknown group '{key}' will be ignored."));
                    }
                }
            }
            None => {
                valid = false;
                warnings.push("Missing or invalid 'groups' object.".to_string());
            }
        }

        if warnings.is_empty() {
            diag_log(
                "info",
                format!(
                    "[SettingsImportPreview] OK path={} groups={}",
                    path,
                    available_groups.len()
                ),
            );
        } else {
            diag_log(
                "warn",
                format!(
                    "[SettingsImportPreview] path={} warnings={}",
                    path,
                    warnings.join("; ")
                ),
            );
        }

        Ok(SettingsBundlePreviewDto {
            path,
            schema_version: schema,
            available_groups,
            warnings,
            valid,
        })
    }

    async fn import_settings_bundle_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String> {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let schema = root
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if schema != "1.0.0" {
            let msg = format!("Unsupported schema_version '{schema}'. Expected '1.0.0'.");
            diag_log("error", format!("[SettingsImport] {msg}"));
            return Err(msg);
        }
        let groups_obj = root
            .get("groups")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "Invalid settings bundle: missing groups object.".to_string())?;
        let mut result = SettingsImportResultDto {
            applied_groups: Vec::new(),
            skipped_groups: Vec::new(),
            warnings: Vec::new(),
            updated_keys: 0,
            appearance_rules_applied: 0,
            theme: None,
            autostart_enabled: None,
        };
        let selected = if selected_groups.is_empty() {
            groups_obj
                .keys()
                .filter_map(|k| normalize_settings_group(k).map(str::to_string))
                .collect::<Vec<String>>()
        } else {
            normalized_selected_groups(&selected_groups)
        };

        for group in selected {
            let Some(value) = groups_obj.get(&group) else {
                result.skipped_groups.push(group.clone());
                result.warnings.push(format!("Group '{group}' not present in bundle."));
                continue;
            };
            let obj = match value.as_object() {
                Some(v) => v,
                None => {
                    result.skipped_groups.push(group.clone());
                    result.warnings.push(format!("Group '{group}' has invalid payload type."));
                    continue;
                }
            };

            match group.as_str() {
                SETTINGS_GROUP_TEMPLATES => {
                    self.clone()
                        .update_config(ConfigUpdateDto {
                            expansion_paused: None,
                            template_date_format: obj.get("template_date_format").and_then(|v| v.as_str()).map(str::to_string),
                            template_time_format: obj.get("template_time_format").and_then(|v| v.as_str()).map(str::to_string),
                            sync_url: None,
                            discovery_enabled: None,
                            discovery_threshold: None,
                            discovery_lookback: None,
                            discovery_min_len: None,
                            discovery_max_len: None,
                            discovery_excluded_apps: None,
                            discovery_excluded_window_titles: None,
                            ghost_suggestor_enabled: None,
                            ghost_suggestor_debounce_ms: None,
                            ghost_suggestor_display_secs: None,
                            ghost_suggestor_snooze_duration_mins: None,
                            ghost_suggestor_offset_x: None,
                            ghost_suggestor_offset_y: None,
                            ghost_follower_enabled: None,
                            ghost_follower_edge_right: None,
                            ghost_follower_monitor_anchor: None,
                            ghost_follower_search: None,
                            ghost_follower_hover_preview: None,
                            ghost_follower_collapse_delay_secs: None,
                            ghost_follower_opacity: None,
                            clip_history_max_depth: None,
                            script_library_run_disabled: None,
                            script_library_run_allowlist: None,
                        })
                        .await?;
                    result.updated_keys = result.updated_keys.saturating_add(2);
                }
                SETTINGS_GROUP_SYNC => {
                    self.clone()
                        .update_config(ConfigUpdateDto {
                            expansion_paused: None,
                            template_date_format: None,
                            template_time_format: None,
                            sync_url: obj.get("sync_url").and_then(|v| v.as_str()).map(str::to_string),
                            discovery_enabled: None,
                            discovery_threshold: None,
                            discovery_lookback: None,
                            discovery_min_len: None,
                            discovery_max_len: None,
                            discovery_excluded_apps: None,
                            discovery_excluded_window_titles: None,
                            ghost_suggestor_enabled: None,
                            ghost_suggestor_debounce_ms: None,
                            ghost_suggestor_display_secs: None,
                            ghost_suggestor_snooze_duration_mins: None,
                            ghost_suggestor_offset_x: None,
                            ghost_suggestor_offset_y: None,
                            ghost_follower_enabled: None,
                            ghost_follower_edge_right: None,
                            ghost_follower_monitor_anchor: None,
                            ghost_follower_search: None,
                            ghost_follower_hover_preview: None,
                            ghost_follower_collapse_delay_secs: None,
                            ghost_follower_opacity: None,
                            clip_history_max_depth: None,
                            script_library_run_disabled: None,
                            script_library_run_allowlist: None,
                        })
                        .await?;
                    result.updated_keys = result.updated_keys.saturating_add(1);
                }
                SETTINGS_GROUP_DISCOVERY
                | SETTINGS_GROUP_GHOST_SUGGESTOR
                | SETTINGS_GROUP_GHOST_FOLLOWER
                | SETTINGS_GROUP_CLIPBOARD_HISTORY
                | SETTINGS_GROUP_COPY_TO_CLIPBOARD
                | SETTINGS_GROUP_CORE
                | SETTINGS_GROUP_SCRIPT_RUNTIME => {
                    let cfg = ConfigUpdateDto {
                        expansion_paused: obj.get("expansion_paused").and_then(|v| v.as_bool()),
                        template_date_format: None,
                        template_time_format: None,
                        sync_url: None,
                        discovery_enabled: obj.get("discovery_enabled").and_then(|v| v.as_bool()),
                        discovery_threshold: obj.get("discovery_threshold").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_lookback: obj.get("discovery_lookback").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_min_len: obj.get("discovery_min_len").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_max_len: obj.get("discovery_max_len").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_excluded_apps: obj.get("discovery_excluded_apps").and_then(|v| v.as_str()).map(str::to_string),
                        discovery_excluded_window_titles: obj
                            .get("discovery_excluded_window_titles")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        ghost_suggestor_enabled: obj.get("ghost_suggestor_enabled").and_then(|v| v.as_bool()),
                        ghost_suggestor_debounce_ms: obj
                            .get("ghost_suggestor_debounce_ms")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_suggestor_display_secs: obj
                            .get("ghost_suggestor_display_secs")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_suggestor_snooze_duration_mins: obj
                            .get("ghost_suggestor_snooze_duration_mins")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_suggestor_offset_x: obj
                            .get("ghost_suggestor_offset_x")
                            .and_then(|v| v.as_i64())
                            .map(|n| n as i32),
                        ghost_suggestor_offset_y: obj
                            .get("ghost_suggestor_offset_y")
                            .and_then(|v| v.as_i64())
                            .map(|n| n as i32),
                        ghost_follower_enabled: obj.get("ghost_follower_enabled").and_then(|v| v.as_bool()),
                        ghost_follower_edge_right: obj.get("ghost_follower_edge_right").and_then(|v| v.as_bool()),
                        ghost_follower_monitor_anchor: obj
                            .get("ghost_follower_monitor_anchor")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_follower_search: None,
                        ghost_follower_hover_preview: obj.get("ghost_follower_hover_preview").and_then(|v| v.as_bool()),
                        ghost_follower_collapse_delay_secs: obj
                            .get("ghost_follower_collapse_delay_secs")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_follower_opacity: obj
                            .get("ghost_follower_opacity")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        clip_history_max_depth: obj
                            .get("clip_history_max_depth")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        script_library_run_disabled: obj
                            .get("script_library_run_disabled")
                            .and_then(|v| v.as_bool()),
                        script_library_run_allowlist: obj
                            .get("script_library_run_allowlist")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                    };
                    self.clone().update_config(cfg).await?;
                    if group == SETTINGS_GROUP_CLIPBOARD_HISTORY {
                        let mut copy_cfg = {
                            let storage = JsonFileStorageAdapter::load();
                            load_copy_to_clipboard_config(
                                &storage,
                                obj.get("clip_history_max_depth")
                                    .and_then(|v| v.as_u64())
                                    .map(|n| n as u32)
                                    .unwrap_or(20),
                            )
                        };
                        if let Some(v) = obj.get("copy_to_clipboard_enabled").and_then(|v| v.as_bool()) {
                            copy_cfg.enabled = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_min_log_length")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32)
                        {
                            copy_cfg.min_log_length = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_cc").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_cc = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_ssn").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_ssn = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_email").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_email = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_blacklist_processes")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.blacklist_processes = v.to_string();
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_json_output_enabled")
                            .and_then(|v| v.as_bool())
                        {
                            copy_cfg.json_output_enabled = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_json_output_dir")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.json_output_dir = v.to_string();
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_image_storage_dir")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.image_storage_dir = v.to_string();
                        }
                        save_copy_to_clipboard_config(&copy_cfg)?;
                    }
                    if group == SETTINGS_GROUP_COPY_TO_CLIPBOARD {
                        let current_depth = {
                            let guard = self.state.lock().map_err(|e| e.to_string())?;
                            guard.clip_history_max_depth as u32
                        };
                        let mut copy_cfg = {
                            let storage = JsonFileStorageAdapter::load();
                            load_copy_to_clipboard_config(&storage, current_depth)
                        };
                        if let Some(v) = obj.get("copy_to_clipboard_enabled").and_then(|v| v.as_bool()) {
                            copy_cfg.enabled = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_min_log_length")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32)
                        {
                            copy_cfg.min_log_length = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_cc").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_cc = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_ssn").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_ssn = v;
                        }
                        if let Some(v) = obj.get("copy_to_clipboard_mask_email").and_then(|v| v.as_bool()) {
                            copy_cfg.mask_email = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_blacklist_processes")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.blacklist_processes = v.to_string();
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_json_output_enabled")
                            .and_then(|v| v.as_bool())
                        {
                            copy_cfg.json_output_enabled = v;
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_json_output_dir")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.json_output_dir = v.to_string();
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_image_storage_dir")
                            .and_then(|v| v.as_str())
                        {
                            copy_cfg.image_storage_dir = v.to_string();
                        }
                        if let Some(v) = obj
                            .get("copy_to_clipboard_max_history_entries")
                            .or_else(|| obj.get("clip_history_max_depth"))
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32)
                        {
                            copy_cfg.max_history_entries = v;
                        }
                        save_copy_to_clipboard_config(&copy_cfg)?;
                        self.clone()
                            .update_config(ConfigUpdateDto {
                                expansion_paused: None,
                                template_date_format: None,
                                template_time_format: None,
                                sync_url: None,
                                discovery_enabled: None,
                                discovery_threshold: None,
                                discovery_lookback: None,
                                discovery_min_len: None,
                                discovery_max_len: None,
                                discovery_excluded_apps: None,
                                discovery_excluded_window_titles: None,
                                ghost_suggestor_enabled: None,
                                ghost_suggestor_debounce_ms: None,
                                ghost_suggestor_display_secs: None,
                                ghost_suggestor_snooze_duration_mins: None,
                                ghost_suggestor_offset_x: None,
                                ghost_suggestor_offset_y: None,
                                ghost_follower_enabled: None,
                                ghost_follower_edge_right: None,
                                ghost_follower_monitor_anchor: None,
                                ghost_follower_search: None,
                                ghost_follower_hover_preview: None,
                                ghost_follower_collapse_delay_secs: None,
                                ghost_follower_opacity: None,
                                clip_history_max_depth: Some(copy_cfg.max_history_entries),
                                script_library_run_disabled: None,
                                script_library_run_allowlist: None,
                            })
                            .await?;
                    }
                    if group == SETTINGS_GROUP_CORE {
                        result.theme = obj.get("theme").and_then(|v| v.as_str()).map(str::to_string);
                        result.autostart_enabled = obj.get("autostart_enabled").and_then(|v| v.as_bool());
                    }
                    result.updated_keys = result.updated_keys.saturating_add(obj.len() as u32);
                }
                SETTINGS_GROUP_APPEARANCE => {
                    let rules_value = obj.get("rules").and_then(|v| v.as_array());
                    let Some(rules_arr) = rules_value else {
                        result.skipped_groups.push(group.clone());
                        result.warnings.push("Appearance group missing 'rules' array.".to_string());
                        continue;
                    };
                    let mut rules = Vec::<AppearanceTransparencyRuleDto>::new();
                    for r in rules_arr {
                        let Some(ro) = r.as_object() else {
                            continue;
                        };
                        let mut app = ro
                            .get("app_process")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .trim()
                            .to_ascii_lowercase();
                        if app.is_empty() {
                            continue;
                        }
                        if !app.ends_with(".exe") {
                            app.push_str(".exe");
                        }
                        let opacity = ro
                            .get("opacity")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32)
                            .unwrap_or(255)
                            .clamp(20, 255);
                        let enabled = ro.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                        rules.push(AppearanceTransparencyRuleDto {
                            app_process: app,
                            opacity,
                            enabled,
                        });
                    }
                    sort_appearance_rules_deterministic(&mut rules);
                    save_appearance_rules(&rules)?;
                    enforce_appearance_transparency_rules();
                    result.appearance_rules_applied = rules.len() as u32;
                    result.updated_keys = result.updated_keys.saturating_add(rules.len() as u32);
                }
                _ => {
                    result.skipped_groups.push(group.clone());
                    result.warnings.push(format!("Unsupported group '{group}'."));
                    continue;
                }
            }

            result.applied_groups.push(group.clone());
            diag_log("info", format!("[SettingsImport] Applied group '{group}'"));
        }

        self.clone().save_settings().await?;
        diag_log(
            "info",
            format!(
                "[SettingsImport] Completed: applied={} skipped={} warnings={}",
                result.applied_groups.len(),
                result.skipped_groups.len(),
                result.warnings.len()
            ),
        );
        Ok(result)
    }

    async fn save_appearance_transparency_rule(
        self,
        app_process: String,
        opacity: u32,
        enabled: bool,
    ) -> Result<(), String> {
        let mut app = app_process.trim().to_ascii_lowercase();
        if app.is_empty() {
            return Err("App process is required".to_string());
        }
        if !app.ends_with(".exe") {
            app.push_str(".exe");
        }
        let app_for_apply = app.clone();
        let opacity = opacity.clamp(20, 255);
        let storage = JsonFileStorageAdapter::load();
        let mut rules = load_appearance_rules(&storage);
        if let Some(existing) = rules
            .iter_mut()
            .find(|r| normalize_process_key(&r.app_process) == normalize_process_key(&app))
        {
            existing.opacity = opacity;
            existing.enabled = enabled;
            existing.app_process = app;
        } else {
            rules.push(AppearanceTransparencyRuleDto {
                app_process: app,
                opacity,
                enabled,
            });
        }
        sort_appearance_rules_deterministic(&mut rules);
        save_appearance_rules(&rules)?;
        if enabled {
            let _ = apply_process_transparency(&app_for_apply, Some(opacity as u8));
        }
        Ok(())
    }

    async fn delete_appearance_transparency_rule(self, app_process: String) -> Result<(), String> {
        let app = normalize_process_key(&app_process);
        if app.is_empty() {
            return Ok(());
        }
        let storage = JsonFileStorageAdapter::load();
        let mut rules = load_appearance_rules(&storage);
        rules.retain(|r| normalize_process_key(&r.app_process) != app);
        save_appearance_rules(&rules)?;
        let _ = apply_process_transparency(&format!("{app}.exe"), None);
        Ok(())
    }

    async fn apply_appearance_transparency_now(
        self,
        app_process: String,
        opacity: u32,
    ) -> Result<u32, String> {
        let alpha = opacity.clamp(20, 255) as u8;
        apply_process_transparency(&app_process, Some(alpha))
    }

    async fn restore_appearance_defaults(self) -> Result<u32, String> {
        let storage = JsonFileStorageAdapter::load();
        let rules = load_appearance_rules(&storage);
        save_appearance_rules(&[])?;

        let mut seen = HashSet::new();
        let mut cleared_windows = 0u32;
        for rule in rules {
            let key = normalize_process_key(&rule.app_process);
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            let app_name = format!("{key}.exe");
            let count = apply_process_transparency(&app_name, None).unwrap_or(0);
            cleared_windows = cleared_windows.saturating_add(count);
        }
        Ok(cleared_windows)
    }

    async fn get_ghost_suggestor_state(self) -> Result<GhostSuggestorStateDto, String> {
        let suggestions = ghost_suggestor::get_suggestions();
        let selected = ghost_suggestor::get_selected_index();
        let has_suggestions = !suggestions.is_empty();
        let first_trigger = suggestions.first().map(|s| s.snippet.trigger.len()).unwrap_or(0);
        log::debug!(
            "[GhostSuggestor] get_ghost_suggestor_state: suggestions={} has_suggestions={} selected={} first_trigger_len={}",
            suggestions.len(),
            has_suggestions,
            selected,
            first_trigger
        );
        let should_auto_hide = ghost_suggestor::should_auto_hide();
        let should_passthrough = should_auto_hide || !has_suggestions;
        if should_auto_hide && has_suggestions {
            ghost_suggestor::dismiss();
        }
        let suggestions = ghost_suggestor::get_suggestions();
        let has_suggestions = !suggestions.is_empty();
        #[cfg(target_os = "windows")]
        let position = {
            let pos = digicore_text_expander::platform::windows_caret::get_caret_screen_position();
            let cfg = ghost_suggestor::get_config();
            let raw = pos.map(|(x, y)| (x + cfg.offset_x, y + cfg.offset_y));
            raw.map(|(x, y)| {
                digicore_text_expander::platform::windows_monitor::clamp_position_to_work_area(
                    x, y, 320, 260,
                )
            })
        };
        #[cfg(not(target_os = "windows"))]
        let position: Option<(i32, i32)> = None;
        log::debug!(
            "[GhostSuggestor] get_ghost_suggestor_state: returning has_suggestions={} position={:?}",
            has_suggestions,
            position
        );
        Ok(GhostSuggestorStateDto {
            has_suggestions,
            suggestions: suggestions
                .into_iter()
                .map(|s| SuggestionDto {
                    trigger: s.snippet.trigger,
                    content_preview: if s.snippet.content.len() > 40 {
                        format!("{}...", &s.snippet.content[..40])
                    } else {
                        s.snippet.content
                    },
                    category: s.category,
                })
                .collect(),
            selected_index: selected as u32,
            position,
            should_passthrough,
        })
    }

    async fn ghost_suggestor_accept(self) -> Result<Option<(String, String)>, String> {
        Ok(ghost_suggestor::accept_selected())
    }

    async fn ghost_suggestor_snooze(self) -> Result<(), String> {
        ghost_suggestor::snooze();
        Ok(())
    }

    async fn ghost_suggestor_dismiss(self) -> Result<(), String> {
        ghost_suggestor::dismiss();
        Ok(())
    }

    async fn ghost_suggestor_ignore(self, phrase: String) -> Result<(), String> {
        discovery::add_ignored_phrase(&phrase);
        ghost_suggestor::dismiss();
        Ok(())
    }

    async fn ghost_suggestor_create_snippet(self) -> Result<Option<(String, String)>, String> {
        let suggestions = ghost_suggestor::get_suggestions();
        let idx = ghost_suggestor::get_selected_index().min(suggestions.len().saturating_sub(1));
        if let Some(s) = suggestions.get(idx) {
            ghost_suggestor::request_create_snippet(
                s.snippet.trigger.clone(),
                s.snippet.content.clone(),
            );
            ghost_suggestor::dismiss();
            Ok(Some((s.snippet.trigger.clone(), s.snippet.content.clone())))
        } else {
            Ok(None)
        }
    }

    async fn ghost_suggestor_cycle_forward(self) -> Result<u32, String> {
        Ok(ghost_suggestor::cycle_selection_forward() as u32)
    }

    async fn get_ghost_follower_state(
        self,
        search_filter: Option<String>,
    ) -> Result<GhostFollowerStateDto, String> {
        let filter = search_filter.as_deref().unwrap_or("");
        let pinned = ghost_follower::get_pinned_snippets(filter);
        let cfg = ghost_follower::get_config();
        let enabled = ghost_follower::is_enabled();
        log::debug!(
            "[GhostFollower] get_ghost_follower_state: enabled={}, pinned_count={}, filter_len={}",
            enabled,
            pinned.len(),
            filter.len()
        );

        #[cfg(target_os = "windows")]
        let (position, saved_position) = {
            let saved = self
                .state
                .lock()
                .ok()
                .and_then(|g| g.ghost_follower_position);
            let use_saved = saved.map_or(false, |(x, y)| {
                x >= -20000 && x <= 20000 && y >= -20000 && y <= 20000
            });
            if use_saved {
                (saved, true)
            } else {
                let work = match cfg.monitor_anchor {
                    ghost_follower::MonitorAnchor::Primary => {
                        digicore_text_expander::platform::windows_monitor::get_primary_monitor_work_area()
                    }
                    ghost_follower::MonitorAnchor::Secondary => {
                        digicore_text_expander::platform::windows_monitor::get_secondary_monitor_work_area()
                            .unwrap_or_else(digicore_text_expander::platform::windows_monitor::get_primary_monitor_work_area)
                    }
                    ghost_follower::MonitorAnchor::Current => {
                        digicore_text_expander::platform::windows_monitor::get_current_monitor_work_area()
                    }
                };
                let (x, _y) = match cfg.edge {
                    ghost_follower::FollowerEdge::Right => (work.right - 280, work.top + 20),
                    ghost_follower::FollowerEdge::Left => (work.left, work.top + 20),
                };
                (Some((x, work.top + 20)), false)
            }
        };
        #[cfg(not(target_os = "windows"))]
        let (position, saved_position): (Option<(i32, i32)>, bool) = (None, false);

        let clip_max = self
            .state
            .lock()
            .map_err(|e| e.to_string())?
            .clip_history_max_depth as u32;

        let collapse_delay = cfg.collapse_delay_secs as u32;
        let should_collapse = ghost_follower::should_collapse(cfg.collapse_delay_secs);

        let opacity = self
            .state
            .lock()
            .map(|g| (g.ghost_follower_opacity as f64 / 100.0).clamp(0.1, 1.0))
            .unwrap_or(1.0);

        Ok(GhostFollowerStateDto {
            enabled,
            pinned: pinned
                .into_iter()
                .map(|(s, cat, idx)| PinnedSnippetDto {
                    trigger: s.trigger.clone(),
                    content: s.content.clone(),
                    content_preview: if s.content.len() > 40 {
                        format!("{}...", &s.content[..40])
                    } else {
                        s.content.clone()
                    },
                    category: cat,
                    snippet_idx: idx as u32,
                })
                .collect(),
            search_filter: ghost_follower::get_search_filter(),
            position,
            edge_right: cfg.edge == ghost_follower::FollowerEdge::Right,
            monitor_primary: cfg.monitor_anchor == ghost_follower::MonitorAnchor::Primary,
            clip_history_max_depth: clip_max,
            should_collapse,
            collapse_delay_secs: collapse_delay,
            opacity,
            saved_position,
        })
    }

    async fn ghost_follower_insert(self, _trigger: String, content: String) -> Result<(), String> {
        log::debug!(
            "[QuickSearchInsert] ghost_follower_insert invoked content_len={}",
            content.len()
        );
        digicore_text_expander::drivers::hotstring::request_expansion_from_ghost_follower(content);
        Ok(())
    }

    async fn bring_main_window_to_foreground(self) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        Ok(())
    }

    async fn ghost_follower_restore_always_on_top(self) -> Result<(), String> {
        if let Some(ghost) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = ghost.set_always_on_top(true);
        }
        Ok(())
    }

    async fn ghost_follower_capture_target_window(self) -> Result<(), String> {
        log::debug!("[QuickSearchInsert] ghost_follower_capture_target_window invoked");
        digicore_text_expander::application::ghost_follower::capture_target_window();
        Ok(())
    }

    async fn ghost_follower_touch(self) -> Result<(), String> {
        ghost_follower::touch();
        Ok(())
    }

    async fn ghost_follower_set_collapsed(self, collapsed: bool) -> Result<(), String> {
        ghost_follower::set_collapsed(collapsed);
        Ok(())
    }

    async fn ghost_follower_set_size(self, width: f64, height: f64) -> Result<(), String> {
        use tauri::LogicalSize;
        if let Some(win) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = win.set_size(LogicalSize::new(width, height));
        }
        Ok(())
    }

    async fn ghost_follower_set_opacity(self, opacity_pct: u32) -> Result<(), String> {
        let val = opacity_pct.clamp(10, 100);
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower_opacity = val;
        }
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn ghost_follower_save_position(self, x: i32, y: i32) -> Result<(), String> {
        let sane = x >= -20000 && x <= 20000 && y >= -20000 && y <= 20000;
        if !sane {
            return Ok(());
        }
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower_position = Some((x, y));
        }
        let mut storage = JsonFileStorageAdapter::load();
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, &x.to_string());
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, &y.to_string());
        let _ = storage.persist_if_safe().map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn ghost_follower_hide(self) -> Result<(), String> {
        if let Some(win) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = win.hide();
        }
        Ok(())
    }

    async fn ghost_follower_set_search(self, filter: String) -> Result<(), String> {
        ghost_follower::set_search_filter(&filter);
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn ghost_follower_request_view_full(self, content: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit("ghost-follower-view-full", content);
        Ok(())
    }

    async fn ghost_follower_request_edit(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit(
            "ghost-follower-edit",
            serde_json::json!({ "category": category, "snippetIdx": snippet_idx as usize }),
        );
        Ok(())
    }

    async fn ghost_follower_request_promote(
        self,
        content: String,
        trigger: String,
    ) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit(
            "ghost-follower-promote",
            serde_json::json!({ "content": content, "trigger": trigger }),
        );
        Ok(())
    }

    async fn ghost_follower_toggle_pin(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        let snippets = guard
            .library
            .get_mut(&category)
            .ok_or_else(|| "Category not found".to_string())?;
        let s = snippets
            .get_mut(snippet_idx as usize)
            .ok_or_else(|| "Snippet not found".to_string())?;
        let new_pinned = if s.pinned.eq_ignore_ascii_case("true") {
            "false"
        } else {
            "true"
        };
        s.pinned = new_pinned.to_string();
        guard.try_save_library().map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn get_pending_variable_input(self) -> Result<Option<PendingVarDto>, String> {
        if let Some((content, vars, values, choice_indices, checkbox_checked)) =
            variable_input::get_viewport_modal_display()
        {
            Ok(Some(PendingVarDto {
                content,
                vars: vars
                    .iter()
                    .map(|v| InteractiveVarDto {
                        tag: v.tag.clone(),
                        label: v.label.clone(),
                        var_type: var_type_to_string(&v.var_type).to_string(),
                        options: v.options.clone(),
                    })
                    .collect(),
                values,
                choice_indices: choice_indices
                    .into_iter()
                    .map(|(k, v)| (k, v as u32))
                    .collect(),
                checkbox_checked,
            }))
        } else {
            Ok(None)
        }
    }

    async fn submit_variable_input(self, values: HashMap<String, String>) -> Result<(), String> {
        if let Some(state) = variable_input::take_viewport_modal() {
            let clip_history: Vec<String> = clipboard_history::get_entries()
                .iter()
                .map(|e| e.content.clone())
                .collect();
            let processed = template_processor::process_with_user_vars(
                &state.content,
                None,
                &clip_history,
                Some(&values),
            );
            let hwnd = state.target_hwnd;
            if let Some(ref tx) = state.response_tx {
                let _ = tx.send((Some(processed), hwnd));
            } else {
                digicore_text_expander::drivers::hotstring::request_expansion(processed);
            }
        }
        Ok(())
    }

    async fn cancel_variable_input(self) -> Result<(), String> {
        if let Some(state) = variable_input::take_viewport_modal() {
            if let Some(ref tx) = state.response_tx {
                let _ = tx.send((None, None));
            }
        }
        Ok(())
    }

    async fn get_expansion_stats(self) -> Result<ExpansionStatsDto, String> {
        let stats = expansion_stats::get_stats();
        Ok(ExpansionStatsDto {
            total_expansions: stats.total_expansions as u32,
            total_chars_saved: stats.total_chars_saved as u32,
            estimated_time_saved_secs: stats.estimated_time_saved_secs(),
            top_triggers: stats
                .top_triggers(10)
                .into_iter()
                .map(|(s, c)| (s, c as u32))
                .collect(),
        })
    }

    async fn reset_expansion_stats(self) -> Result<(), String> {
        expansion_stats::reset_stats();
        Ok(())
    }

    async fn get_diagnostic_logs(self) -> Result<Vec<DiagnosticEntryDto>, String> {
        let entries = expansion_diagnostics::get_recent();
        Ok(entries
            .into_iter()
            .map(|e| DiagnosticEntryDto {
                timestamp_ms: e.timestamp_ms as u32,
                level: e.level,
                message: e.message,
            })
            .collect())
    }

    async fn clear_diagnostic_logs(self) -> Result<(), String> {
        expansion_diagnostics::clear();
        Ok(())
    }

    async fn test_snippet_logic(
        self,
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Ok(SnippetLogicTestResultDto {
                result: String::new(),
                requires_input: false,
                vars: Vec::new(),
            });
        }

        let vars = template_processor::collect_interactive_vars(trimmed);
        let vars_dto: Vec<InteractiveVarDto> = vars
            .iter()
            .map(|v| InteractiveVarDto {
                tag: v.tag.clone(),
                label: v.label.clone(),
                var_type: var_type_to_string(&v.var_type).to_string(),
                options: v.options.clone(),
            })
            .collect();

        if !vars_dto.is_empty() && user_values.is_none() {
            return Ok(SnippetLogicTestResultDto {
                result: String::new(),
                requires_input: true,
                vars: vars_dto,
            });
        }

        let current_clipboard = arboard::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get_text().ok());
        let clip_history: Vec<String> = clipboard_history::get_entries()
            .into_iter()
            .map(|e| e.content)
            .collect();

        let result = template_processor::process_for_preview(
            trimmed,
            current_clipboard.as_deref(),
            &clip_history,
            user_values.as_ref(),
        );
        let requires_input = !vars_dto.is_empty() && user_values.is_none();

        Ok(SnippetLogicTestResultDto {
            result,
            requires_input,
            vars: vars_dto,
        })
    }

    async fn get_weather_location_suggestions(
        self,
        city_query: String,
        country: Option<String>,
        region: Option<String>,
    ) -> Result<Vec<String>, String> {
        let query = city_query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let registry = digicore_text_expander::application::scripting::get_registry();
        digicore_text_expander::application::scripting::weather_location_suggestions(
            query,
            country.as_deref(),
            region.as_deref(),
            registry.http_fetcher.as_ref(),
        )
        .map(|v| v.into_iter().take(10).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_copy_to_clipboard_config, effective_rules_for_enforcement, normalize_clipboard_path_or_default,
        normalize_process_key, normalized_selected_groups,
        normalized_selected_scripting_profile_groups, parse_scripting_profile_bundle,
        sort_appearance_rules_deterministic, write_clipboard_text_json_record,
    };
    #[cfg(target_os = "windows")]
    use super::process_name_matches;
    use crate::AppearanceTransparencyRuleDto;
    use std::io::Write;

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_exact_and_case_insensitive() {
        assert!(process_name_matches("cursor.exe", "Cursor.EXE"));
        assert!(process_name_matches("CURSOR.EXE", "cursor.exe"));
        assert!(process_name_matches("cursor", "cursor"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_with_optional_exe_suffix() {
        assert!(process_name_matches("cursor", "cursor.exe"));
        assert!(process_name_matches("cursor.exe", "cursor"));
        assert!(!process_name_matches("cursor", "code.exe"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_rejects_empty_values() {
        assert!(!process_name_matches("", "cursor.exe"));
        assert!(!process_name_matches("cursor.exe", ""));
        assert!(!process_name_matches("   ", "cursor.exe"));
    }

    #[test]
    fn normalize_process_key_handles_exe_suffix_and_case() {
        assert_eq!(normalize_process_key("Cursor.EXE"), "cursor");
        assert_eq!(normalize_process_key("cursor"), "cursor");
        assert_eq!(normalize_process_key("  Code.ExE  "), "code");
    }

    #[test]
    fn appearance_rules_sort_is_deterministic() {
        let mut rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "zeta.exe".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 180,
                enabled: false,
            },
            AppearanceTransparencyRuleDto {
                app_process: "alpha.exe".to_string(),
                opacity: 140,
                enabled: true,
            },
        ];
        sort_appearance_rules_deterministic(&mut rules);
        assert_eq!(rules[0].app_process, "alpha.exe");
        assert_eq!(rules[1].app_process, "zeta.exe");
        assert_eq!(rules[2].app_process, "cursor");
    }

    #[test]
    fn selected_groups_defaults_to_all_when_empty() {
        let groups = normalized_selected_groups(&[]);
        assert!(groups.contains(&"templates".to_string()));
        assert!(groups.contains(&"appearance".to_string()));
        assert!(groups.len() >= 9);
    }

    #[test]
    fn selected_groups_normalizes_aliases_and_deduplicates() {
        let input = vec![
            "Ghost Follower".to_string(),
            "ghost-follower".to_string(),
            "appearance".to_string(),
            "invalid".to_string(),
        ];
        let groups = normalized_selected_groups(&input);
        assert_eq!(groups, vec!["ghost_follower".to_string(), "appearance".to_string()]);
    }

    #[test]
    fn scripting_profile_groups_default_to_all_when_empty() {
        let groups = normalized_selected_scripting_profile_groups(&[]);
        assert!(groups.contains(&"javascript".to_string()));
        assert!(groups.contains(&"run".to_string()));
        assert_eq!(groups.len(), 6);
    }

    #[test]
    fn scripting_profile_groups_normalize_aliases_and_deduplicate() {
        let input = vec![
            "JS".to_string(),
            "javascript".to_string(),
            "http-weather".to_string(),
            "Run Security".to_string(),
            "invalid".to_string(),
        ];
        let groups = normalized_selected_scripting_profile_groups(&input);
        assert_eq!(
            groups,
            vec![
                "javascript".to_string(),
                "http".to_string(),
                "run".to_string()
            ]
        );
    }

    #[test]
    fn scripting_profile_preview_migrates_legacy_schema_1x() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        let payload = serde_json::json!({
            "schema_version": "1.0.0",
            "selected_groups": ["javascript"],
            "groups": {
                "javascript": { "library": "function a(){return 1;}" }
            }
        });
        write!(tmp, "{}", payload).expect("write payload");
        let parsed =
            parse_scripting_profile_bundle(tmp.path().to_str().expect("path str")).expect("parse");
        assert!(parsed.valid);
        assert!(!parsed.signed_bundle);
        assert!(!parsed.signature_valid);
        assert_eq!(parsed.migrated_from_schema.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn scripting_profile_preview_rejects_v2_without_integrity() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        let payload = serde_json::json!({
            "schema_version": "2.0.0",
            "selected_groups": ["javascript"],
            "groups": {
                "javascript": { "library": "function a(){return 1;}" }
            }
        });
        write!(tmp, "{}", payload).expect("write payload");
        let parsed =
            parse_scripting_profile_bundle(tmp.path().to_str().expect("path str")).expect("parse");
        assert!(!parsed.valid);
        assert!(parsed
            .warnings
            .iter()
            .any(|w| w.contains("Missing integrity block")));
    }

    #[test]
    fn appearance_integration_save_restart_reenforce_is_deterministic() {
        let saved_rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor.exe".to_string(),
                opacity: 120,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "code.exe".to_string(),
                opacity: 180,
                enabled: true,
            },
        ];

        // Simulate startup/enforcement after restart by recomputing effective rules.
        let first_start = effective_rules_for_enforcement(saved_rules.clone());
        let second_start = effective_rules_for_enforcement(saved_rules);
        assert_eq!(first_start.len(), 2);
        assert_eq!(second_start.len(), 2);
        assert_eq!(first_start[0].app_process, second_start[0].app_process);
        assert_eq!(first_start[0].opacity, second_start[0].opacity);
        assert_eq!(first_start[1].app_process, second_start[1].app_process);
        assert_eq!(first_start[1].opacity, second_start[1].opacity);
    }

    #[test]
    fn appearance_stress_many_rules_remains_stable() {
        let mut rules = Vec::new();
        for i in 0..5000u32 {
            let base = format!("app{}", i % 300);
            rules.push(AppearanceTransparencyRuleDto {
                app_process: if i % 2 == 0 { base.clone() } else { format!("{base}.exe") },
                opacity: 20 + (i % 236),
                enabled: i % 5 != 0,
            });
        }

        let first = effective_rules_for_enforcement(rules.clone());
        let second = effective_rules_for_enforcement(rules);
        assert_eq!(first.len(), second.len());
        for idx in 0..first.len() {
            assert_eq!(first[idx].app_process, second[idx].app_process);
            assert_eq!(first[idx].opacity, second[idx].opacity);
            assert!(first[idx].enabled);
        }
        assert!(first.len() <= 300);
    }

    #[test]
    fn copy_to_clipboard_defaults_include_output_directories() {
        let cfg = default_copy_to_clipboard_config(5000);
        assert!(!cfg.json_output_dir.trim().is_empty());
        assert!(!cfg.image_storage_dir.trim().is_empty());
    }

    #[test]
    fn clipboard_text_json_record_is_written_to_selected_directory() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let out = tmp.path().join("json-output");
        let out_s = out.to_string_lossy().to_string();
        write_clipboard_text_json_record(&out_s, "hello world", "Cursor.exe", "Editor")
            .expect("json write");
        let entries = std::fs::read_dir(&out)
            .expect("read output dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("dir entries");
        assert_eq!(entries.len(), 1);
        let payload = std::fs::read_to_string(entries[0].path()).expect("read json file");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("parse json");
        assert_eq!(parsed.get("entry_type").and_then(|v| v.as_str()), Some("text"));
        assert_eq!(parsed.get("content").and_then(|v| v.as_str()), Some("hello world"));

        let fallback = normalize_clipboard_path_or_default("   ", out.clone());
        assert_eq!(fallback, out);
    }
}
