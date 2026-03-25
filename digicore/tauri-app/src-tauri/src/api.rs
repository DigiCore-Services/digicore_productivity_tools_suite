//! TauRPC API - type-safe IPC procedures for DigiCore Text Expander.
// Triggering bindings regeneration.


use crate::{
    clipboard_repository,
    kms_repository,
    kms_service::KmsService,
    kms_diagnostic_service::KmsDiagnosticService,
    embedding_service,
    indexing_service,
    skill_sync,
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
use digicore_text_expander::application::ghost_follower::{
    self, ExpandTrigger, FollowerEdge, FollowerMode, MonitorAnchor,
};
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
use digicore_text_expander::services::extraction_service::create_extraction_service;
use digicore_core::domain::{ExtractionSource, ExtractionMimeType};
use chrono;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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
use notify::{Watcher, RecursiveMode, Config, Event};

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
        image_capture_enabled: true,
        min_log_length: 1,
        mask_cc: false,
        mask_ssn: false,
        mask_email: false,
        blacklist_processes: String::new(),
        max_history_entries,
        json_output_enabled: true,
        json_output_dir: json_dir.to_string_lossy().to_string(),
        image_storage_dir: image_dir.to_string_lossy().to_string(),
        ocr_enabled: true,
    }
}

fn load_copy_to_clipboard_config(storage: &JsonFileStorageAdapter, max_history_entries: u32) -> CopyToClipboardConfigDto {
    let mut cfg = default_copy_to_clipboard_config(max_history_entries);
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_ENABLED) {
        cfg.enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_IMAGE_ENABLED) {
        cfg.image_capture_enabled = v == "true";
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
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_OCR_ENABLED) {
        cfg.ocr_enabled = v == "true";
    }
    cfg
}

fn save_copy_to_clipboard_config(config: &CopyToClipboardConfigDto) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::COPY_TO_CLIPBOARD_ENABLED, &config.enabled.to_string());
    storage.set(storage_keys::COPY_TO_CLIPBOARD_IMAGE_ENABLED, &config.image_capture_enabled.to_string());
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
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_OCR_ENABLED,
        &config.ocr_enabled.to_string(),
    );
    let result = storage.persist_if_safe().map(|_| ()).map_err(|e| e.to_string());
    if result.is_ok() {
        clipboard_history::update_config(ClipboardHistoryConfig {
            enabled: config.enabled || config.image_capture_enabled,
            max_depth: if config.max_history_entries == 0 {
                usize::MAX
            } else {
                config.max_history_entries as usize
            },
        });
    }
    result
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
    file_list: Option<Vec<String>>,
) -> Result<Option<u32>, String> {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled {
        return Ok(None);
    }
    if process_is_blacklisted(process_name, &cfg.blacklist_processes) {
        return Ok(None);
    }
    if content.trim().chars().count() < cfg.min_log_length as usize {
        return Ok(None);
    }
    let masked = apply_masking(content.to_string(), &cfg);
    let inserted_id = clipboard_repository::insert_entry(&masked, process_name, window_title, file_list)?;
    log::info!("[Clipboard] clipboard_repository::insert_entry returned {:?}", inserted_id);
    if let Some(_id) = inserted_id {
        if cfg.json_output_enabled {
            if let Err(err) = write_clipboard_text_json_record(
                &cfg.json_output_dir,
                &masked,
                process_name,
                window_title,
            ) {
                diag_log("warn", format!("[Clipboard][json.write_err] {err}"));
            }
        }
        if cfg.max_history_entries > 0 {
            let _ = clipboard_repository::trim_to_depth(cfg.max_history_entries);
        }
    }
    Ok(inserted_id)
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
        &state.ghost_follower.config.enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MODE,
        &format!("{:?}", state.ghost_follower.config.mode),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EDGE_RIGHT,
        &(state.ghost_follower.config.edge == digicore_text_expander::application::ghost_follower::FollowerEdge::Right).to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR,
        &match state.ghost_follower.config.monitor_anchor {
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Primary => 0u32,
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Secondary => 1u32,
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Current => 2u32,
        }.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EXPAND_TRIGGER,
        &format!("{:?}", state.ghost_follower.config.expand_trigger),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EXPAND_DELAY_MS,
        &state.ghost_follower.config.expand_delay_ms.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW,
        &state.ghost_follower.config.hover_preview.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS,
        &state.ghost_follower.config.collapse_delay_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_CLIPBOARD_DEPTH,
        &state.ghost_follower.config.clipboard_depth.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_OPACITY,
        &state.ghost_follower.config.opacity.to_string(),
    );
    if let Some((px, py)) = state.ghost_follower.config.position {
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
    
    storage.set(storage_keys::CORPUS_ENABLED, &state.corpus_enabled.to_string());
    storage.set(storage_keys::CORPUS_OUTPUT_DIR, &state.corpus_output_dir);
    storage.set(storage_keys::CORPUS_SNAPSHOT_DIR, &state.corpus_snapshot_dir);
    storage.set(storage_keys::CORPUS_SHORTCUT_MODIFIERS, &state.corpus_shortcut_modifiers.to_string());
    storage.set(storage_keys::CORPUS_SHORTCUT_KEY, &state.corpus_shortcut_key.to_string());

    storage.set(storage_keys::EXTRACTION_ROW_OVERLAP_TOLERANCE, &state.extraction_row_overlap_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_CLUSTER_THRESHOLD_FACTOR, &state.extraction_cluster_threshold_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ZONE_PROXIMITY, &state.extraction_zone_proximity.to_string());
    storage.set(storage_keys::EXTRACTION_CROSS_ZONE_GAP_FACTOR, &state.extraction_cross_zone_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_SAME_ZONE_GAP_FACTOR, &state.extraction_same_zone_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_SIGNIFICANT_GAP_GATE, &state.extraction_significant_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_CHAR_WIDTH_FACTOR, &state.extraction_char_width_factor.to_string());
    storage.set(storage_keys::EXTRACTION_BRIDGED_THRESHOLD, &state.extraction_bridged_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_WORD_SPACING_FACTOR, &state.extraction_word_spacing_factor.to_string());

    storage.set(storage_keys::EXTRACTION_FOOTER_TRIGGERS, &state.extraction_footer_triggers);
    storage.set(storage_keys::EXTRACTION_TABLE_MIN_CONTIGUOUS_ROWS, &state.extraction_table_min_contiguous_rows.to_string());
    storage.set(storage_keys::EXTRACTION_TABLE_MIN_AVG_SEGMENTS, &state.extraction_table_min_avg_segments.to_string());

    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CLUSTER_FACTOR, &state.extraction_adaptive_plaintext_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_GAP_GATE, &state.extraction_adaptive_plaintext_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CLUSTER_FACTOR, &state.extraction_adaptive_table_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_GAP_GATE, &state.extraction_adaptive_table_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CLUSTER_FACTOR, &state.extraction_adaptive_column_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_GAP_GATE, &state.extraction_adaptive_column_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CROSS_FACTOR, &state.extraction_adaptive_plaintext_cross_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CROSS_FACTOR, &state.extraction_adaptive_table_cross_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CROSS_FACTOR, &state.extraction_adaptive_column_cross_factor.to_string());


    storage.set(storage_keys::EXTRACTION_REFINEMENT_ENTROPY_THRESHOLD, &state.extraction_refinement_entropy_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_REFINEMENT_CLUSTER_THRESHOLD_MODIFIER, &state.extraction_refinement_cluster_threshold_modifier.to_string());
    storage.set(storage_keys::EXTRACTION_REFINEMENT_CROSS_ZONE_GAP_MODIFIER, &state.extraction_refinement_cross_zone_gap_modifier.to_string());

    storage.set(storage_keys::EXTRACTION_CLASSIFIER_GUTTER_WEIGHT, &state.extraction_classifier_gutter_weight.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_DENSITY_WEIGHT, &state.extraction_classifier_density_weight.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_MULTICOLUMN_DENSITY_MAX, &state.extraction_classifier_multicolumn_density_max.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_TABLE_DENSITY_MIN, &state.extraction_classifier_table_density_min.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_TABLE_ENTROPY_MIN, &state.extraction_classifier_table_entropy_min.to_string());

    storage.set(storage_keys::EXTRACTION_COLUMNS_MIN_CONTIGUOUS_ROWS, &state.extraction_columns_min_contiguous_rows.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_GUTTER_GAP_FACTOR, &state.extraction_columns_gutter_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_GUTTER_VOID_TOLERANCE, &state.extraction_columns_gutter_void_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_EDGE_MARGIN_TOLERANCE, &state.extraction_columns_edge_margin_tolerance.to_string());

    storage.set(storage_keys::EXTRACTION_HEADERS_MAX_WIDTH_RATIO, &state.extraction_headers_max_width_ratio.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_CENTERED_TOLERANCE, &state.extraction_headers_centered_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H1_SIZE_MULTIPLIER, &state.extraction_headers_h1_size_multiplier.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H2_SIZE_MULTIPLIER, &state.extraction_headers_h2_size_multiplier.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H3_SIZE_MULTIPLIER, &state.extraction_headers_h3_size_multiplier.to_string());

    storage.set(storage_keys::EXTRACTION_SCORING_JITTER_PENALTY_WEIGHT, &state.extraction_scoring_jitter_penalty_weight.to_string());
    storage.set(storage_keys::EXTRACTION_SCORING_SIZE_PENALTY_WEIGHT, &state.extraction_scoring_size_penalty_weight.to_string());
    storage.set(storage_keys::EXTRACTION_SCORING_LOW_CONFIDENCE_THRESHOLD, &state.extraction_scoring_low_confidence_threshold.to_string());
    
    storage.set(storage_keys::EXTRACTION_LAYOUT_ROW_LOOKBACK, &state.extraction_layout_row_lookback.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_TABLE_BREAK_THRESHOLD, &state.extraction_layout_table_break_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_PARAGRAPH_BREAK_THRESHOLD, &state.extraction_layout_paragraph_break_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_MAX_SPACE_CLAMP, &state.extraction_layout_max_space_clamp.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_COLUMN_JITTER_TOLERANCE, &state.extraction_tables_column_jitter_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MAX, &state.extraction_tables_merge_y_gap_max.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MIN, &state.extraction_tables_merge_y_gap_min.to_string());

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

#[taurpc::ipc_type]
pub struct RichTextDto {


    pub plain: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
}

// Export to frontend src/ (outside src-tauri) to avoid watcher rebuild loop

#[taurpc::procedures]


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
    async fn get_clipboard_rich_text() -> Result<RichTextDto, String>;
    async fn get_image_gallery(
        search: Option<String>,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ClipEntryDto>, u32), String>;
    async fn get_child_entries(parent_id: u32) -> Result<Vec<ClipEntryDto>, String>;
    async fn get_copy_to_clipboard_config() -> Result<CopyToClipboardConfigDto, String>;
    async fn save_copy_to_clipboard_config(config: CopyToClipboardConfigDto) -> Result<(), String>;
    async fn get_copy_to_clipboard_stats() -> Result<CopyToClipboardStatsDto, String>;
    async fn copy_to_clipboard(text: String) -> Result<(), String>;
    async fn copy_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn save_clipboard_image_by_id(id: u32, path: String) -> Result<(), String>;
    async fn open_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn get_clip_entry_by_id(id: u32) -> Result<Option<ClipEntryDto>, String>;
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
    async fn kms_launch() -> Result<(), String>;
    async fn kms_initialize() -> Result<String, String>;
    async fn kms_list_notes() -> Result<Vec<KmsNoteDto>, String>;
    async fn kms_load_note(path: String) -> Result<String, String>;
    async fn kms_save_note(path: String, content: String) -> Result<(), String>;
    async fn kms_delete_note(path: String) -> Result<(), String>;
    async fn kms_rename_note(old_path: String, new_name: String) -> Result<String, String>;
    async fn kms_rename_folder(old_path: String, new_name: String) -> Result<String, String>;
    async fn kms_delete_folder(path: String) -> Result<(), String>;
    async fn kms_move_item(path: String, new_parent_path: String) -> Result<String, String>;
    async fn kms_create_folder(path: String) -> Result<(), String>;
    async fn kms_search_semantic(query: String, modality: Option<String>, limit: u32, search_mode: Option<String>) -> Result<Vec<SearchResultDto>, String>;
    async fn kms_reindex_all() -> Result<(), String>;
    async fn kms_reindex_note(path: String) -> Result<(), String>;
    async fn kms_repair_database() -> Result<(), String>;
    async fn kms_get_vault_path() -> Result<String, String>;
    async fn kms_set_vault_path(new_path: String, migrate: bool) -> Result<(), String>;
    async fn kms_reindex_type(provider_id: String) -> Result<u32, String>;
    async fn kms_get_indexing_status() -> Result<Vec<IndexingStatusDto>, String>;
    async fn kms_get_indexing_details(provider_id: String) -> Result<Vec<KmsIndexStatusRow>, String>;
    async fn kms_retry_item(provider_id: String, entity_id: String) -> Result<(), String>;
    async fn kms_retry_failed(provider_id: String) -> Result<(), String>;
    async fn kms_get_note_links(path: String) -> Result<KmsLinksDto, String>;
    async fn kms_get_logs(limit: u32) -> Result<Vec<KmsLogDto>, String>;
    async fn kms_clear_logs() -> Result<(), String>;
    async fn kms_get_vault_structure() -> Result<KmsFileSystemItemDto, String>;

    // --- Skill Hub ---
    async fn kms_list_skills() -> Result<Vec<SkillDto>, String>;
    async fn kms_get_skill(name: String) -> Result<Option<SkillDto>, String>;
    async fn kms_save_skill(skill: SkillDto, overwrite: bool) -> Result<(), String>;
    async fn kms_delete_skill(name: String) -> Result<(), String>;
    async fn kms_sync_skills() -> Result<(), String>;
    async fn kms_add_skill_resource(skill_name: String, source_path: String, target_subdir: Option<String>) -> Result<SkillResourceDto, String>;
    async fn kms_remove_skill_resource(skill_name: String, rel_path: String) -> Result<(), String>;
    async fn kms_check_skill_conflicts(skill_name: String, sync_targets: Vec<String>) -> Result<Vec<SyncConflictDto>, String>;
}

#[taurpc::ipc_type]
pub struct SkillDto {
    pub metadata: SkillMetadataDto,
    pub path: Option<String>,
    pub instructions: Option<String>,
    pub resources: Vec<SkillResourceDto>,
}

#[taurpc::ipc_type]
pub struct SkillResourceDto {
    pub name: String,
    pub r#type: String, // "Script" | "Template" | "Reference" | "Other"
    pub rel_path: String,
}

#[taurpc::ipc_type]
pub struct SkillMetadataDto {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<String>, // JSON string for arbitrary KV
    pub disable_model_invocation: Option<bool>,
    pub scope: String, // "Global" | "Project"
    pub sync_targets: Vec<String>,
}

#[taurpc::ipc_type]
pub struct SyncConflictDto {
    pub target: String,
    pub existing_name: String,
    pub conflict_type: String, // "NameCollision" | "ContentMismatch"
}

#[taurpc::ipc_type]
pub struct KmsNoteDto {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub preview: Option<String>,
    pub last_modified: Option<String>,
    pub is_favorite: bool,
    pub sync_status: String,
}

#[taurpc::ipc_type]
pub struct KmsLogDto {
    pub id: i32,
    pub level: String,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: String,
}

#[taurpc::ipc_type]
pub struct KmsFileSystemItemDto {
    pub name: String,
    pub path: String,
    pub rel_path: String,
    pub item_type: String, // "file" | "directory"
    pub children: Option<Vec<KmsFileSystemItemDto>>,
    pub note: Option<KmsNoteDto>,
}

#[taurpc::ipc_type]
pub struct KmsLinksDto {
    pub outgoing: Vec<KmsNoteDto>,
    pub incoming: Vec<KmsNoteDto>,
}

#[taurpc::ipc_type]
pub struct SearchResultDto {
    pub entity_type: String, // 'note', 'snippet', 'clip'
    pub entity_id: String,
    pub distance: f32,
    pub modality: String, // 'text', 'image'
    pub metadata: Option<String>,
    pub snippet: Option<String>,
}

#[taurpc::ipc_type]
pub struct IndexingStatusDto {
    pub category: String, // "notes", "snippets", "clipboard"
    pub indexed_count: u32,
    pub failed_count: u32,
    pub total_count: u32,
    pub last_error: Option<String>,
}

#[taurpc::ipc_type]
pub struct KmsIndexStatusRow {
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct ApiImpl {
    pub state: Arc<Mutex<digicore_text_expander::application::app_state::AppState>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    pub clipboard: Arc<dyn digicore_core::domain::ports::ClipboardPort>,
}

impl ApiImpl {

    fn get_vault_path(&self) -> PathBuf {
        let app = get_app(&self.app_handle);
        let app_state = app.state::<Arc<Mutex<AppState>>>();
        let state = app_state.lock().unwrap();
        
        if state.kms_vault_path.is_empty() {
             // Fallback to default documents dir
             let docs = app.path().document_dir().unwrap_or_else(|_| PathBuf::from(""));
             docs.join("DigiCore Notes")
        } else {
             PathBuf::from(&state.kms_vault_path)
        }
    }

    fn resolve_absolute_path(&self, relative_path: &str) -> PathBuf {
        self.get_vault_path().join(relative_path)
    }

    fn get_relative_path(&self, absolute_path: &Path) -> Result<String, String> {
        let vault = self.get_vault_path();
        absolute_path
            .strip_prefix(&vault)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .map_err(|_| format!("Path {} is not within the vault {}", absolute_path.display(), vault.display()))
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

// Ensure KmsIndexingService is imported for the trait methods to find it
use crate::indexing_service::KmsIndexingService;

pub(crate) fn get_app(app: &Arc<Mutex<Option<AppHandle>>>) -> AppHandle {
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

pub(crate) fn sync_runtime_clipboard_entries_to_sqlite(app: &tauri::AppHandle) {
    let entries = clipboard_history::get_entries();
    if entries.is_empty() {
        sync_current_clipboard_image_to_sqlite(String::new(), String::new(), Some(app));
        return;
    }
    for entry in entries.into_iter().rev() {
        let _ = persist_clipboard_entry_with_settings(
            &entry.content,
            &entry.process_name,
            &entry.window_title,
            entry.file_list.clone(),
        );
    }
    sync_current_clipboard_image_to_sqlite(String::new(), String::new(), Some(app));
}

pub(crate) fn sync_current_clipboard_image_to_sqlite(process_name: String, window_title: String, app: Option<&tauri::AppHandle>) {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled && !cfg.image_capture_enabled {
        return;
    }
    if !cfg.image_capture_enabled {
        return;
    }
    if process_is_blacklisted(&process_name, &cfg.blacklist_processes) {
        return;
    }
    let mut image_opt = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
            Ok(img) => {
                log::info!("[Clipboard][capture.image] Detected image: {}x{} ({} bytes) on attempt {}", img.width, img.height, img.bytes.len(), attempt + 1);
                image_opt = Some(img);
                break;
            },
            Err(e) => {
                if attempt == 2 {
                    log::warn!("[Clipboard][capture.image] Final failed to get image from clipboard: {}", e);
                } else {
                    log::debug!("[Clipboard][capture.image] Retryable failure to get image (attempt {}): {}", attempt + 1, e);
                }
            },
        }
    }

    let image = match image_opt {
        Some(img) => img,
        None => return,
    };
    if image.width == 0 || image.height == 0 || image.bytes.is_empty() {
        return;
    }

    // Capture initial values for async task
    let rgba_bytes = image.bytes.to_vec();
    let width = image.width;
    let height = image.height;
    let proc = process_name.clone();
    let win = window_title.clone();
    let ocr_enabled = cfg.ocr_enabled;

    let inserted_id = clipboard_repository::insert_image_entry_returning_id(
        &rgba_bytes,
        width as u32,
        height as u32,
        &process_name,
        &window_title,
        Some("image/png"),
        &cfg.image_storage_dir,
    ).unwrap_or(0);

    if inserted_id > 0 {
        if let Some(handle) = app {
            let h = handle.clone();
            let service = h.state::<Arc<crate::indexing_service::KmsIndexingService>>().inner().clone();
            let entity_id = inserted_id.to_string();
            tauri::async_runtime::spawn(async move {
                let _ = service.index_single_item(&h, "clipboard", &entity_id).await;
            });
        }

        if cfg.max_history_entries > 0 {
            if let Ok(deleted_ids) = clipboard_repository::trim_to_depth(cfg.max_history_entries) {
                for id in deleted_ids {
                    let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
                }
            }
        }
        diag_log("info", "[Clipboard][capture.image] persisted clipboard image");

        // Spawn OCR if enabled
        let app_handle_for_ocr = app.cloned();
        if ocr_enabled {
            tauri::async_runtime::spawn(async move {
                let dispatcher = create_extraction_service();
                // Encode raw pixels to PNG before OCR, as Windows Native OCR (WIC) 
                // requires a container format (PNG/JPG) for its BitmapDecoder.
                let mut png_bytes = Vec::new();
                match image::RgbaImage::from_raw(width as u32, height as u32, rgba_bytes) {
                    Some(img) => {
                        if let Err(e) = image::DynamicImage::ImageRgba8(img)
                            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png) {
                            log::error!("[Clipboard][OCR] Failed to encode PNG for OCR: {}", e);
                            return;
                        }
                    },
                    None => {
                        log::error!("[Clipboard][OCR] Failed to construct RgbaImage from buffer ({}x{})", width, height);
                        return;
                    }
                }

                let source = ExtractionSource::Buffer(png_bytes);
                let mime = ExtractionMimeType::Png; 
                
                log::info!("[Clipboard][OCR] Starting background OCR for parent_id: {}", inserted_id);
                match dispatcher.extract(source, mime).await {
                    Ok(result) => {
                        if !result.text.trim().is_empty() {
                            let text_id = clipboard_repository::insert_extracted_text_entry(
                                &result.text,
                                &proc,
                                &win,
                                inserted_id,
                                &result.metadata,
                            ).unwrap_or(0);
                            
                            if text_id > 0 {
                                if let Some(h) = app_handle_for_ocr {
                                    let service = h.state::<Arc<crate::indexing_service::KmsIndexingService>>().inner().clone();
                                    let entity_id = text_id.to_string();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = service.index_single_item(&h, "clipboard", &entity_id).await;
                                    });
                                }
                            }
                            log::info!("[Clipboard][OCR] OCR completed and saved for parent_id: {}", inserted_id);
                        } else {
                            log::info!("[Clipboard][OCR] OCR completed but no text found for parent_id: {}", inserted_id);
                        }
                    },
                    Err(e) => {
                        log::error!("[Clipboard][OCR] OCR failed for parent_id {}: {}", inserted_id, e);
                    }
                }
            });
        }
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
        let trigger = snippet.trigger.clone();
        {
            let mut guard = self.state.lock().map_err(|e| e.to_string())?;
            guard.add_snippet(&category, &snippet);
            update_library(guard.library.clone());
        }
        
        let handle = crate::api::get_app(&self.app_handle);
        let service = handle.state::<Arc<crate::indexing_service::KmsIndexingService>>().inner().clone();
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            let _ = service.index_single_item(&handle_clone, "snippets", &trigger).await;
        });

        let _ = crate::api::get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn update_snippet(
        self,
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String> {
        let new_trigger = snippet.trigger.clone();
        let old_trigger = {
            let guard = self.state.lock().map_err(|e| e.to_string())?;
            guard
                .library
                .get(&category)
                .and_then(|v| v.get(snippet_idx as usize))
                .map(|s| s.trigger.clone())
        };

        {
            let mut guard = self.state.lock().map_err(|e| e.to_string())?;
            guard
                .update_snippet(&category, snippet_idx as usize, &snippet)
                .map_err(|e| e.to_string())?;
            update_library(guard.library.clone());
        }

        if let Some(old) = old_trigger {
            if old != new_trigger {
                let _ = crate::kms_repository::delete_embedding("text", "snippet", &old);
                let _ = crate::kms_repository::update_index_status("snippets", &old, "deleted", None);
            }
        }

        let handle = crate::api::get_app(&self.app_handle);
        let service = handle.state::<Arc<crate::indexing_service::KmsIndexingService>>().inner().clone();
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            let _ = service.index_single_item(&handle_clone, "snippets", &new_trigger).await;
        });

        let _ = crate::api::get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }


    async fn delete_snippet(self, category: String, snippet_idx: u32) -> Result<(), String> {
        let trigger = {
            let guard = self.state.lock().map_err(|e| e.to_string())?;
            guard
                .library
                .get(&category)
                .and_then(|v| v.get(snippet_idx as usize))
                .map(|s| s.trigger.clone())
        };

        {
            let mut guard = self.state.lock().map_err(|e| e.to_string())?;
            guard
                .delete_snippet(&category, snippet_idx as usize)
                .map_err(|e| e.to_string())?;
            update_library(guard.library.clone());
        }

        if let Some(t) = trigger {
            let _ = crate::kms_repository::delete_embedding("text", "snippet", &t);
            let _ = crate::kms_repository::update_index_status("snippets", &t, "deleted", None);
        }

        let _ = crate::api::get_app(&self.app_handle).emit("ghost-follower-update", ());
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
        if let Some(ref v) = config.expansion_log_path {
            guard.expansion_log_path = v.clone();
            digicore_text_expander::application::expansion_logger::set_log_path(v.clone());
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
            guard.ghost_follower.config.enabled = v;
        }
        if let Some(v) = config.ghost_follower_edge_right {
            guard.ghost_follower.config.edge = if v { FollowerEdge::Right } else { FollowerEdge::Left };
        }
        if let Some(v) = config.ghost_follower_monitor_anchor {
            guard.ghost_follower.config.monitor_anchor = match v {
                1 => MonitorAnchor::Secondary,
                2 => MonitorAnchor::Current,
                _ => MonitorAnchor::Primary,
            };
        }
        if let Some(ref v) = config.ghost_follower_search {
            guard.ghost_follower.search_filter = v.clone();
        }
        if let Some(v) = config.ghost_follower_hover_preview {
            guard.ghost_follower.config.hover_preview = v;
        }
        if let Some(v) = config.ghost_follower_collapse_delay_secs {
            guard.ghost_follower.config.collapse_delay_secs = v as u64;
        }
        if let Some(v) = config.ghost_follower_opacity {
            guard.ghost_follower.config.opacity = v.clamp(10, 100);
        }
        if let Some(ref v) = config.ghost_follower_mode {
            if v == "Bubble" || v == "FloatingBubble" {
                guard.ghost_follower.config.mode = FollowerMode::FloatingBubble;
            } else {
                guard.ghost_follower.config.mode = FollowerMode::EdgeAnchored;
            }
        }
        if let Some(ref v) = config.ghost_follower_expand_trigger {
            if v == "Hover" {
                guard.ghost_follower.config.expand_trigger = ExpandTrigger::Hover;
            } else {
                guard.ghost_follower.config.expand_trigger = ExpandTrigger::Click;
            }
        }
        if let Some(v) = config.ghost_follower_expand_delay_ms {
            guard.ghost_follower.config.expand_delay_ms = v as u64;
        }
        if let Some(v) = config.ghost_follower_clipboard_depth {
            guard.ghost_follower.config.clipboard_depth = v as usize;
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
                if let Ok(deleted_ids) = clipboard_repository::trim_to_depth(depth as u32) {
                    for id in deleted_ids {
                        let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
                    }
                }
            }
        }
        if let Some(v) = config.script_library_run_disabled {
            guard.script_library_run_disabled = v;
        }
        if let Some(ref v) = config.script_library_run_allowlist {
            guard.script_library_run_allowlist = v.clone();
        }

        if let Some(v) = config.corpus_enabled { guard.corpus_enabled = v; }
        if let Some(ref v) = config.corpus_output_dir { guard.corpus_output_dir = v.clone(); }
        if let Some(ref v) = config.corpus_snapshot_dir { guard.corpus_snapshot_dir = v.clone(); }
        if let Some(v) = config.corpus_shortcut_modifiers { guard.corpus_shortcut_modifiers = v as u16; }
        if let Some(v) = config.corpus_shortcut_key { guard.corpus_shortcut_key = v as u16; }

        if let Some(v) = config.extraction_row_overlap_tolerance { guard.extraction_row_overlap_tolerance = v; }
        if let Some(v) = config.extraction_cluster_threshold_factor { guard.extraction_cluster_threshold_factor = v; }
        if let Some(v) = config.extraction_zone_proximity { guard.extraction_zone_proximity = v; }
        if let Some(v) = config.extraction_cross_zone_gap_factor { guard.extraction_cross_zone_gap_factor = v; }
        if let Some(v) = config.extraction_same_zone_gap_factor { guard.extraction_same_zone_gap_factor = v; }
        if let Some(v) = config.extraction_significant_gap_gate { guard.extraction_significant_gap_gate = v; }
        if let Some(v) = config.extraction_char_width_factor { guard.extraction_char_width_factor = v; }
        if let Some(v) = config.extraction_bridged_threshold { guard.extraction_bridged_threshold = v; }
        if let Some(v) = config.extraction_word_spacing_factor { guard.extraction_word_spacing_factor = v; }

        if let Some(ref v) = config.extraction_footer_triggers { guard.extraction_footer_triggers = v.clone(); }
        if let Some(v) = config.extraction_table_min_contiguous_rows { guard.extraction_table_min_contiguous_rows = v as usize; }
        if let Some(v) = config.extraction_table_min_avg_segments { guard.extraction_table_min_avg_segments = v; }

        if let Some(v) = config.extraction_adaptive_plaintext_cluster_factor { guard.extraction_adaptive_plaintext_cluster_factor = v; }
        if let Some(v) = config.extraction_adaptive_plaintext_gap_gate { guard.extraction_adaptive_plaintext_gap_gate = v; }
        if let Some(v) = config.extraction_adaptive_table_cluster_factor { guard.extraction_adaptive_table_cluster_factor = v; }
        if let Some(v) = config.extraction_adaptive_table_gap_gate { guard.extraction_adaptive_table_gap_gate = v; }
        if let Some(v) = config.extraction_adaptive_column_cluster_factor { guard.extraction_adaptive_column_cluster_factor = v; }
        if let Some(v) = config.extraction_adaptive_column_gap_gate { guard.extraction_adaptive_column_gap_gate = v; }
        if let Some(v) = config.extraction_adaptive_plaintext_cross_factor { guard.extraction_adaptive_plaintext_cross_factor = v; }
        if let Some(v) = config.extraction_adaptive_table_cross_factor { guard.extraction_adaptive_table_cross_factor = v; }
        if let Some(v) = config.extraction_adaptive_column_cross_factor { guard.extraction_adaptive_column_cross_factor = v; }

        if let Some(v) = config.extraction_refinement_entropy_threshold { guard.extraction_refinement_entropy_threshold = v; }
        if let Some(v) = config.extraction_refinement_cluster_threshold_modifier { guard.extraction_refinement_cluster_threshold_modifier = v; }
        if let Some(v) = config.extraction_refinement_cross_zone_gap_modifier { guard.extraction_refinement_cross_zone_gap_modifier = v; }

        if let Some(v) = config.extraction_classifier_gutter_weight { guard.extraction_classifier_gutter_weight = v; }
        if let Some(v) = config.extraction_classifier_density_weight { guard.extraction_classifier_density_weight = v; }
        if let Some(v) = config.extraction_classifier_multicolumn_density_max { guard.extraction_classifier_multicolumn_density_max = v; }
        if let Some(v) = config.extraction_classifier_table_density_min { guard.extraction_classifier_table_density_min = v; }
        if let Some(v) = config.extraction_classifier_table_entropy_min { guard.extraction_classifier_table_entropy_min = v; }

        if let Some(v) = config.extraction_columns_min_contiguous_rows { guard.extraction_columns_min_contiguous_rows = v as usize; }
        if let Some(v) = config.extraction_columns_gutter_gap_factor { guard.extraction_columns_gutter_gap_factor = v; }
        if let Some(v) = config.extraction_columns_gutter_void_tolerance { guard.extraction_columns_gutter_void_tolerance = v; }
        if let Some(v) = config.extraction_columns_edge_margin_tolerance { guard.extraction_columns_edge_margin_tolerance = v; }

        if let Some(v) = config.extraction_headers_max_width_ratio { guard.extraction_headers_max_width_ratio = v; }
        if let Some(v) = config.extraction_headers_centered_tolerance { guard.extraction_headers_centered_tolerance = v; }
        if let Some(v) = config.extraction_headers_h1_size_multiplier { guard.extraction_headers_h1_size_multiplier = v; }
        if let Some(v) = config.extraction_headers_h2_size_multiplier { guard.extraction_headers_h2_size_multiplier = v; }
        if let Some(v) = config.extraction_headers_h3_size_multiplier { guard.extraction_headers_h3_size_multiplier = v; }

        if let Some(v) = config.extraction_scoring_jitter_penalty_weight { guard.extraction_scoring_jitter_penalty_weight = v; }
        if let Some(v) = config.extraction_scoring_size_penalty_weight { guard.extraction_scoring_size_penalty_weight = v; }
        if let Some(v) = config.extraction_scoring_low_confidence_threshold { guard.extraction_scoring_low_confidence_threshold = v; }

        if let Some(v) = config.extraction_layout_row_lookback { guard.extraction_layout_row_lookback = v as usize; }
        if let Some(v) = config.extraction_layout_table_break_threshold { guard.extraction_layout_table_break_threshold = v; }
        if let Some(v) = config.extraction_layout_paragraph_break_threshold { guard.extraction_layout_paragraph_break_threshold = v; }
        if let Some(v) = config.extraction_layout_max_space_clamp { guard.extraction_layout_max_space_clamp = v as usize; }
        if let Some(v) = config.extraction_tables_column_jitter_tolerance { guard.extraction_tables_column_jitter_tolerance = v; }
        if let Some(v) = config.extraction_tables_merge_y_gap_max { guard.extraction_tables_merge_y_gap_max = v; }
        if let Some(v) = config.extraction_tables_merge_y_gap_min { guard.extraction_tables_merge_y_gap_min = v; }

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
            follower_enabled: guard.ghost_follower.config.enabled,
            follower_edge_right: guard.ghost_follower.config.edge == FollowerEdge::Right,
            follower_monitor_anchor: match guard.ghost_follower.config.monitor_anchor {
                MonitorAnchor::Secondary => 1,
                MonitorAnchor::Current => 2,
                _ => 0,
            },
            follower_search: guard.ghost_follower.search_filter.clone(),
            follower_hover_preview: guard.ghost_follower.config.hover_preview,
            follower_collapse_delay_secs: guard.ghost_follower.config.collapse_delay_secs,
        });
        set_expansion_paused(guard.expansion_paused);
        {
            use digicore_text_expander::adapters::corpus::{FileSystemCorpusStorageAdapter, OcrBaselineAdapter};
            use digicore_text_expander::application::corpus_generator::CorpusService;
            use digicore_core::domain::value_objects::CorpusConfig;
            let corpus_config = CorpusConfig {
                enabled: guard.corpus_enabled,
                output_dir: guard.corpus_output_dir.clone(),
                snapshot_dir: guard.corpus_snapshot_dir.clone(),
                shortcut_modifiers: guard.corpus_shortcut_modifiers,
                shortcut_key: guard.corpus_shortcut_key,
            };
            let corpus_storage = std::sync::Arc::new(FileSystemCorpusStorageAdapter::new(corpus_config.output_dir.clone()));
            let ocr_config = digicore_text_expander::adapters::extraction::RuntimeConfig::load_from_json_adapter(&JsonFileStorageAdapter::load());
            let corpus_baseline = std::sync::Arc::new(OcrBaselineAdapter::new(corpus_config.snapshot_dir.clone(), ocr_config));
            let corpus_service = std::sync::Arc::new(CorpusService::new(corpus_config, corpus_storage, corpus_baseline));
            digicore_text_expander::drivers::hotstring::update_corpus_service(Some(corpus_service));
        }
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        
        // Persist all AppState fields to storage
        persist_settings_to_storage(&guard)?;
        
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
                parent_id: r.parent_id,
                metadata: r.metadata,
                file_list: r.file_list,
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
                parent_id: r.parent_id,
                metadata: r.metadata,
                file_list: r.file_list,
            })
            .collect())
    }

    async fn delete_clip_entry(self, index: u32) -> Result<(), String> {
        let rows = clipboard_repository::list_entries(None, index.saturating_add(1))?;
        if let Some(row) = rows.get(index as usize) {
            let id = row.id;
            clipboard_repository::delete_entry_by_id(id)?;
            let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
            diag_log(
                "info",
                format!("[Clipboard][delete] removed entry id={} via index", id),
            );
        }
        clipboard_history::delete_entry_at(index as usize);
        Ok(())
    }

    async fn delete_clip_entry_by_id(self, id: u32) -> Result<(), String> {
        clipboard_repository::delete_entry_by_id(id)?;
        let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
        diag_log("info", format!("[Clipboard][delete] removed entry id={id}"));
        Ok(())
    }

    async fn clear_clipboard_history(self) -> Result<(), String> {
        clipboard_repository::clear_all()?;
        clipboard_history::clear_all();
        let _ = kms_repository::delete_all_embeddings_for_type("clipboard");
        diag_log("info", "[Clipboard][clear] cleared all clipboard history");
        Ok(())
    }

    async fn get_clipboard_rich_text(self) -> Result<RichTextDto, String> {
        let (plain, html, rtf) = self.clipboard.get_rich_text().map_err(|e| e.to_string())?;
        Ok(RichTextDto { plain, html, rtf })
    }

    async fn get_image_gallery(
        self,
        search: Option<String>,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ClipEntryDto>, u32), String> {
        let (rows, total) =
            clipboard_repository::list_image_entries(search.as_deref(), page, page_size)?;
        let dtos = rows
            .into_iter()
            .map(|r| {
                ClipEntryDto {
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
                    parent_id: r.parent_id,
                    metadata: r.metadata,
                    file_list: r.file_list,
                }
            })
            .collect();
        Ok((dtos, total))
    }

    async fn get_child_entries(self, parent_id: u32) -> Result<Vec<ClipEntryDto>, String> {
        let rows = clipboard_repository::get_child_entries(parent_id)?;
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
                parent_id: r.parent_id,
                metadata: r.metadata,
                file_list: r.file_list,
            })
            .collect())
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
                enabled: normalized.enabled || normalized.image_capture_enabled,
                max_depth: if normalized.max_history_entries == 0 {
                    usize::MAX
                } else {
                    normalized.max_history_entries as usize
                },
            });
        }
        let deleted_ids = if normalized.max_history_entries > 0 {
            clipboard_repository::trim_to_depth(normalized.max_history_entries).unwrap_or_default()
        } else {
            Vec::new()
        };
        let trimmed = deleted_ids.len();
        for id in deleted_ids {
            let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
        }
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
        self.clipboard.set_text(&text).map_err(|e| e.to_string())?;
        diag_log("info", "[Clipboard][copy] copied text to system clipboard via adapter");
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

    async fn get_clip_entry_by_id(self, id: u32) -> Result<Option<ClipEntryDto>, String> {
        let entry_opt = clipboard_repository::get_entry_by_id(id)?;
        let dto_opt = entry_opt.map(|r| {
            ClipEntryDto {
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
                parent_id: r.parent_id,
                metadata: r.metadata,
                file_list: r.file_list,
            }
        });
        Ok(dto_opt)
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
                            "ghost_follower_enabled": guard.ghost_follower.config.enabled,
                            "ghost_follower_edge_right": guard.ghost_follower.config.edge == FollowerEdge::Right,
                            "ghost_follower_monitor_anchor": match guard.ghost_follower.config.monitor_anchor {
                                MonitorAnchor::Secondary => 1,
                                MonitorAnchor::Current => 2,
                                _ => 0,
                            },
                            "ghost_follower_hover_preview": guard.ghost_follower.config.hover_preview,
                            "ghost_follower_collapse_delay_secs": guard.ghost_follower.config.collapse_delay_secs,
                            "ghost_follower_opacity": guard.ghost_follower.config.opacity,
                            "ghost_follower_mode": format!("{:?}", guard.ghost_follower.config.mode),
                            "ghost_follower_expand_trigger": format!("{:?}", guard.ghost_follower.config.expand_trigger),
                            "ghost_follower_expand_delay_ms": guard.ghost_follower.config.expand_delay_ms,
                            "ghost_follower_clipboard_depth": guard.ghost_follower.config.clipboard_depth
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
                            script_library_run_allowlist: None,
                            ..Default::default()
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
                            script_library_run_allowlist: None,
                            ..Default::default()
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
                        ghost_follower_mode: obj.get("ghost_follower_mode").and_then(|v| v.as_str()).map(str::to_string),
                        ghost_follower_expand_trigger: obj.get("ghost_follower_expand_trigger").and_then(|v| v.as_str()).map(str::to_string),
                        ghost_follower_expand_delay_ms: obj.get("ghost_follower_expand_delay_ms").and_then(|v| v.as_u64()).map(|n| n as u32),
                        ghost_follower_clipboard_depth: obj.get("ghost_follower_clipboard_depth").and_then(|v| v.as_u64()).map(|n| n as u32),
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
                        ..Default::default()
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
                                script_library_run_allowlist: None,
                                ..Default::default()
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
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        let gf_state = &guard.ghost_follower;
        let pinned = ghost_follower::get_pinned_snippets(gf_state, filter);
        let cfg = gf_state.config.clone();
        let enabled = cfg.enabled;
        log::debug!(
            "[GhostFollower] get_ghost_follower_state: enabled={}, pinned_count={}, filter_len={}",
            enabled,
            pinned.len(),
            filter.len()
        );

        #[cfg(target_os = "windows")]
        let (position, saved_position) = {
            let saved = guard.ghost_follower.config.position;
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

        let ghost_follower_opacity = gf_state.config.opacity;
        let _ghost_follower_position = gf_state.config.position;

        let collapse_delay = cfg.collapse_delay_secs as u32;
        let should_collapse = gf_state.should_collapse();

        let opacity = (ghost_follower_opacity as f64 / 100.0).clamp(0.1, 1.0);

        Ok(GhostFollowerStateDto {
            enabled,
            mode: format!("{:?}", gf_state.config.mode),
            expand_trigger: format!("{:?}", gf_state.config.expand_trigger),
            expand_delay_ms: gf_state.config.expand_delay_ms as u32,
            clipboard_depth: gf_state.config.clipboard_depth as u32,
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
            search_filter: gf_state.search_filter.clone(),
            position,
            edge_right: cfg.edge == FollowerEdge::Right,
            monitor_primary: cfg.monitor_anchor == MonitorAnchor::Primary,
            clip_history_max_depth: guard.clip_history_max_depth as u32,
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
        ghost_follower::capture_target_window_global();
        Ok(())
    }

    async fn ghost_follower_touch(self) -> Result<(), String> {
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower.touch();
        }
        Ok(())
    }

    async fn ghost_follower_set_collapsed(self, collapsed: bool) -> Result<(), String> {
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower.collapsed = collapsed;
        }
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
            guard.ghost_follower.config.opacity = val;
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
            guard.ghost_follower.config.position = Some((x, y));
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
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower.search_filter = filter;
        }
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
        let library_clone = guard.library.clone();
        guard.ghost_follower.update_library(&library_clone);
        update_library(library_clone);
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn get_pending_variable_input(self) -> Result<Option<PendingVarDto>, String> {
        let display = variable_input::get_viewport_modal_display();
        log::info!("[Api] get_pending_variable_input: has_display={}", display.is_some());
        if let Some((content, vars, values, choice_indices, checkbox_checked)) = display {
            log::info!("[Api] get_pending_variable_input: content_len={}, vars_count={}", content.len(), vars.len());
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

    async fn kms_launch(self) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let vault = self.get_vault_path();

        if let Some(win) = app.get_webview_window("kms") {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        } else {
            let _win = tauri::WebviewWindowBuilder::new(
                &app,
                "kms",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("DigiCore Knowledge Management Suite")
            .inner_size(1000.0, 700.0)
            .min_inner_size(800.0, 500.0)
            .build()
            .map_err(|e| e.to_string())?;
        }

        // Initialize DB if needed
        kms_repository::init_database()?;
        
        // Background sync
        let app_clone = app.clone();
        tokio::spawn(async move {
            let _ = sync_vault_files_to_db_internal(&app_clone, &vault).await;
        });

        Ok(())
    }

    async fn kms_initialize(self) -> Result<String, String> {
        let app = get_app(&self.app_handle);
        let vault_path = self.get_vault_path();

        if !vault_path.exists() {
            std::fs::create_dir_all(&vault_path).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(vault_path.join("notes")).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(vault_path.join("attachments")).map_err(|e| e.to_string())?;

            // Create a welcome note
            let welcome_content = "# Welcome to DigiCore KMS\n\nThis is your local-first knowledge base.\n\n- **Private**: All notes are stored as flat Markdown files.\n- **Connected**: Use `[[Links]]` to build your knowledge graph.\n- **Unified**: Access your snippets and clipboard history directly.";
            std::fs::write(vault_path.join("notes").join("Welcome.md"), welcome_content)
                .map_err(|e| e.to_string())?;
        }

        // Ensure repository is initialized
        kms_repository::init_database()?;

        // Background sync to avoid blocking app initialization
        let app_clone = app.clone();
        let vault_clone = vault_path.clone();
        tokio::spawn(async move {
            let _ = app_clone.emit("kms-sync-status", "Indexing...");
            let _ = sync_vault_files_to_db_internal(&app_clone, &vault_clone).await;
            let _ = app_clone.emit("kms-sync-status", "Idle");
            let _ = app_clone.emit("kms-sync-complete", ());
        });

        // Initialize filesystem watcher
        start_kms_watcher(app.clone(), vault_path.clone());

        Ok(vault_path.to_string_lossy().to_string())
    }

    async fn kms_get_note_links(self, path: String) -> Result<KmsLinksDto, String> {
        let path_buf = PathBuf::from(&path);
        let rel_path = self.get_relative_path(&path_buf)?;
        let (outgoing_rows, incoming_rows) = kms_repository::get_links_for_note(&rel_path)?;

        let map_to_dto = |r: kms_repository::KmsNoteRow| {
            let abs_path = self.resolve_absolute_path(&r.path).to_string_lossy().to_string();
            KmsNoteDto {
                id: r.id,
                path: abs_path,
                title: r.title,
                preview: r.content_preview,
                last_modified: r.last_modified,
                is_favorite: r.is_favorite,
                sync_status: r.sync_status,
            }
        };

        Ok(KmsLinksDto {
            outgoing: outgoing_rows.into_iter().map(map_to_dto).collect(),
            incoming: incoming_rows.into_iter().map(map_to_dto).collect(),
        })
    }

    async fn kms_list_notes(self) -> Result<Vec<KmsNoteDto>, String> {
        let rows = kms_repository::list_notes()?;
        Ok(rows
            .into_iter()
            .map(|r| {
                // Return absolute path to UI for editor convenience, but DB uses relative
                let abs_path = self.resolve_absolute_path(&r.path).to_string_lossy().to_string();
                KmsNoteDto {
                    id: r.id,
                    path: abs_path,
                    title: r.title,
                    preview: r.content_preview,
                    last_modified: r.last_modified,
                    is_favorite: r.is_favorite,
                    sync_status: r.sync_status,
                }
            })
            .collect())
    }

    async fn kms_load_note(self, path: String) -> Result<String, String> {
        let path_buf = PathBuf::from(&path);
        if !path_buf.exists() {
            return Err("Note file not found on disk".to_string());
        }
        std::fs::read_to_string(path_buf).map_err(|e| e.to_string())
    }

    async fn kms_save_note(self, path: String, content: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        KmsService::save_note(&app, &path, &content)
            .await
            .map_err(|e| e.to_string())?;
            
        let path_buf = PathBuf::from(&path);
        let rel_path = self.get_relative_path(&path_buf)?;
        let title = path_buf.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        sync_note_index_internal(&rel_path, &title, &content).await?;

        // Background sync to ensure all backlinks/links are updated across the vault
        let vault = self.get_vault_path();
        tokio::spawn(async move {
            let _ = sync_vault_files_to_db_internal(&app, &vault).await;
        });
            
        Ok(())
    }

    async fn kms_delete_note(self, path: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        KmsService::delete_note(&app, &path)
            .await
            .map_err(|e| e.to_string())
    }

    async fn kms_rename_note(self, old_path: String, new_name: String) -> Result<String, String> {
        let app = get_app(&self.app_handle);
        KmsService::rename_note(&app, &old_path, &new_name)
            .await
            .map_err(|e| e.to_string())
    }

    async fn kms_rename_folder(self, old_path: String, new_name: String) -> Result<String, String> {
        let app = get_app(&self.app_handle);
        KmsService::rename_folder(&app, &old_path, &new_name)
            .await
            .map_err(|e| e.to_string())
    }

    async fn kms_delete_folder(self, path: String) -> Result<(), String> {
        let abs_path = PathBuf::from(&path);
        if !abs_path.exists() {
            return Err("Folder does not exist".to_string());
        }
        if !abs_path.is_dir() {
            return Err("Path is not a folder".to_string());
        }
        
        let rel_path = self.get_relative_path(&abs_path)?;
        
        kms_repository::delete_folder_recursive(&rel_path)?;
        std::fs::remove_dir_all(&abs_path).map_err(|e| format!("Failed to delete folder: {e}"))?;
        
        Ok(())
    }

    async fn kms_move_item(self, path: String, new_parent_path: String) -> Result<String, String> {
        let app = get_app(&self.app_handle);
        KmsService::move_item(&app, &path, &new_parent_path)
            .await
            .map_err(|e| e.to_string())
    }

    async fn kms_get_logs(self, limit: u32) -> Result<Vec<KmsLogDto>, String> {
        let logs = kms_repository::list_logs(limit)?;
        Ok(logs.into_iter().map(|l| KmsLogDto {
            id: l.id,
            level: l.level,
            message: l.message,
            details: l.details,
            timestamp: l.timestamp,
        }).collect())
    }

    async fn kms_clear_logs(self) -> Result<(), String> {
        kms_repository::clear_logs()
    }

    async fn kms_create_folder(self, path: String) -> Result<(), String> {
        let path_buf = PathBuf::from(&path);
        if path_buf.exists() {
            return Err("Folder already exists".to_string());
        }
        std::fs::create_dir_all(&path_buf).map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn kms_get_vault_structure(self) -> Result<KmsFileSystemItemDto, String> {
        let vault_root = self.get_vault_path();
        if !vault_root.exists() {
            return Err("Vault not initialized".to_string());
        }

        let db_notes = kms_repository::list_notes()?;
        let mut note_map: HashMap<String, KmsNoteDto> = db_notes
            .into_iter()
            .map(|r| {
                let abs_path = self.resolve_absolute_path(&r.path).to_string_lossy().to_string();
                (
                    r.path.replace('\\', "/"),
                    KmsNoteDto {
                        id: r.id,
                        path: abs_path,
                        title: r.title,
                        preview: r.content_preview,
                        last_modified: r.last_modified,
                        is_favorite: r.is_favorite,
                        sync_status: r.sync_status,
                    },
                )
            })
            .collect();

        fn build_tree(
            dir: &Path,
            root: &Path,
            note_map: &mut HashMap<String, KmsNoteDto>,
        ) -> Vec<KmsFileSystemItemDto> {
            let mut items = Vec::new();
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    if name.starts_with('.') {
                        continue;
                    }

                    if path.is_dir() {
                        let children = build_tree(&path, root, note_map);
                        let rel_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                        items.push(KmsFileSystemItemDto {
                            name,
                            path: path.to_string_lossy().to_string(),
                            rel_path,
                            item_type: "directory".to_string(),
                            children: Some(children),
                            note: None,
                        });
                    } else if path.extension().map(|e| e == "md" || e == "markdown").unwrap_or(false) {
                        let rel_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                        let note = note_map.remove(&rel_path);
                        items.push(KmsFileSystemItemDto {
                            name,
                            path: path.to_string_lossy().to_string(),
                            rel_path,
                            item_type: "file".to_string(),
                            children: None,
                            note,
                        });
                    }
                }
            }
            // Sort: directories first, then alphabetically
            items.sort_by(|a, b| {
                if a.item_type != b.item_type {
                    b.item_type.cmp(&a.item_type) // "directory" < "file" alphabetically, but we want dir first
                } else {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                }
            });
            items
        }

        let children = build_tree(&vault_root, &vault_root, &mut note_map);
        
        Ok(KmsFileSystemItemDto {
            name: "Vault".to_string(),
            path: vault_root.to_string_lossy().to_string(),
            rel_path: "".to_string(),
            item_type: "directory".to_string(),
            children: Some(children),
            note: None,
        })
    }

    async fn kms_search_semantic(
        self,
        query: String,
        modality: Option<String>,
        limit: u32,
        search_mode: Option<String>,
    ) -> Result<Vec<SearchResultDto>, String> {
        let modality = modality.unwrap_or_else(|| "text".to_string());
        let search_mode = search_mode.unwrap_or_else(|| "Hybrid".to_string());
        
        tokio::task::spawn_blocking(move || {
            let vector = embedding_service::generate_text_embedding(&query, None)
                .map_err(|e| format!("Embedding error: {e}"))?;

            let results = kms_repository::search_hybrid(&query, &modality, vector, &search_mode, limit)?;
            
            Ok(results
                .into_iter()
                .map(|r| {
                    // Resolve relative ID to absolute path for the UI convenience ONLY for notes
                    let final_id = if r.entity_type == "note" {
                        self.resolve_absolute_path(&r.entity_id).to_string_lossy().to_string()
                    } else {
                        r.entity_id
                    };

                    let mut snippet = None;
                    if r.entity_type == "note" {
                        if let Ok(content) = std::fs::read_to_string(&final_id) {
                            snippet = Some(KmsService::extract_contextual_snippet(&content, &query));
                        }
                    } else if r.entity_type == "snippet" || r.entity_type == "clipboard" {
                        // For snippets/clipboard, we always want to show the content
                        if let Some(meta_str) = &r.metadata {
                            if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(meta_str) {
                                snippet = meta_json.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
                            }
                        }
                        
                        // Fallback if metadata extraction failed or snippet is still empty
                        if snippet.is_none() {
                            snippet = r.metadata.clone();
                        }
                    }

                    SearchResultDto {
                        entity_type: r.entity_type,
                        entity_id: final_id,
                        distance: r.distance,
                        modality: r.modality,
                        metadata: r.metadata,
                        snippet,
                    }
                })
                .collect())
        }).await.map_err(|e| format!("Task execution error: {}", e))?
    }

    async fn kms_reindex_all(self) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let service = app.state::<Arc<KmsIndexingService>>().inner().clone();
        
        let app_clone = app.clone();
        tokio::spawn(async move {
            let _ = service.index_all_providers(&app_clone).await;
        });
        
        log::info!("[KMS] Global reindexing started in background.");
        Ok(())
    }

    async fn kms_reindex_type(self, provider_id: String) -> Result<u32, String> {
        let app = get_app(&self.app_handle);
        let service = app.state::<Arc<indexing_service::KmsIndexingService>>();
        
        match service.index_provider_by_id(&app, &provider_id).await {
            Ok(count) => Ok(count as u32),
            Err(e) => Err(e),
        }
    }

    async fn kms_get_indexing_status(self) -> Result<Vec<IndexingStatusDto>, String> {
        let mut results = Vec::new();
        let categories = ["notes", "snippets", "clipboard"];
        
        for cat in categories {
            let (indexed, failed, total) = match kms_repository::get_category_counts(cat) {
                Ok(counts) => counts,
                Err(e) => {
                    log::warn!("[Api] Failed to get counts for category '{}': {}. Returning zeros.", cat, e);
                    (0, 0, 0)
                }
            };
            
            // For categories with failures, try to get the last error
            let last_error = if failed > 0 {
                kms_repository::get_detailed_status(cat).ok()
                    .and_then(|details| details.first().and_then(|r| r.error.clone()))
            } else {
                None
            };

            results.push(IndexingStatusDto {
                category: cat.to_string(),
                indexed_count: indexed,
                failed_count: failed,
                total_count: total,
                last_error,
            });
        }
        
        Ok(results)
    }

    async fn kms_get_indexing_details(self, provider_id: String) -> Result<Vec<KmsIndexStatusRow>, String> {
        let rows = kms_repository::get_detailed_status(&provider_id)?;
        Ok(rows.into_iter().map(|r| KmsIndexStatusRow {
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            status: r.status,
            error: r.error,
            updated_at: r.updated_at,
        }).collect())
    }

    async fn kms_retry_item(self, provider_id: String, entity_id: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let service = app.state::<Arc<indexing_service::KmsIndexingService>>();
        
        service.index_single_item(&app, &provider_id, &entity_id).await?;
        Ok(())
    }

    async fn kms_retry_failed(self, provider_id: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let service = app.state::<Arc<indexing_service::KmsIndexingService>>();
        
        let failures = kms_repository::get_detailed_status(&provider_id)?;
        for fail in failures {
            let _ = service.index_single_item(&app, &provider_id, &fail.entity_id).await;
        }
        Ok(())
    }

    async fn kms_repair_database(self) -> Result<(), String> {
        kms_repository::repair_database()?;
        Ok(())
    }

    async fn kms_reindex_note(self, rel_path: String) -> Result<(), String> {
        let abs_path = self.resolve_absolute_path(&rel_path);
        
        if !abs_path.exists() {
            return Err("File not found on disk".into());
        }
        
        let current_title = abs_path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
            
        // Mark as pending
        let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None);
        
        let app = get_app(&self.app_handle);
        let _ = app.emit("kms-sync-status", "Indexing...");
        let _ = app.emit("kms-sync-complete", ()); // To update UI immediately to pending state
        
        match std::fs::read_to_string(&abs_path) {
            Ok(content) => {
                if let Err(e) = sync_note_index_internal(&rel_path, &current_title, &content).await {
                     log::error!("[KMS][Sync] Failed to reindex note {}: {}", rel_path, e);
                     let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e));
                }
            }
            Err(e) => {
                let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e.to_string()));
            }
        }
        
        let _ = app.emit("kms-sync-complete", ());
        let _ = app.emit("kms-sync-status", "Idle");
        Ok(())
    }

    async fn kms_get_vault_path(self) -> Result<String, String> {
        Ok(self.get_vault_path().to_string_lossy().to_string())
    }

    async fn kms_set_vault_path(self, new_path: String, migrate: bool) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let old_path = self.get_vault_path();
        let new_path_buf = PathBuf::from(&new_path);
        
        if migrate && old_path.exists() && old_path != new_path_buf {
            // Perform physical migration
            if !new_path_buf.exists() {
                std::fs::create_dir_all(&new_path_buf).map_err(|e| e.to_string())?;
            }
            
            // Perform physical migration recursively
            fn move_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
                if src.is_dir() {
                    if !dest.exists() {
                        return std::fs::rename(src, dest);
                    }
                    for entry in std::fs::read_dir(src)? {
                        let entry = entry?;
                        move_recursive(&entry.path(), &dest.join(entry.file_name()))?;
                    }
                } else if !dest.exists() {
                    std::fs::rename(src, dest)?;
                }
                Ok(())
            }

            if let Ok(entries) = std::fs::read_dir(&old_path) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    let file_name = entry.file_name();
                    let dest = new_path_buf.join(&file_name);
                    
                    if file_name.to_string_lossy().starts_with('.') {
                        continue;
                    }

                    let _ = move_recursive(&src, &dest);
                }
            }
        }
        
        // Update AppState
        {
            let app_state = app.state::<Arc<Mutex<AppState>>>();
            let mut state = app_state.lock().unwrap();
            state.kms_vault_path = new_path.clone();
            
            // Persist to storage
            let mut storage = JsonFileStorageAdapter::load();
            storage.set(storage_keys::KMS_VAULT_PATH, &new_path);
            let _ = storage.persist();
        }
        
        // Trigger sync with new path
        let app_clone = app.clone();
        let sync_path = new_path_buf.clone();
        tokio::spawn(async move {
            let _ = app_clone.emit("kms-sync-status", "Indexing...");
            let _ = sync_vault_files_to_db_internal(&app_clone, &sync_path).await;
            let _ = app_clone.emit("kms-sync-status", "Idle");
            let _ = app_clone.emit("kms-sync-complete", ());
        });
        
        // Re-initialize filesystem watcher
        start_kms_watcher(app.clone(), new_path_buf);

        // Emit event to frontend
        let _ = app.emit("kms-vault-path-changed", new_path);
        
        Ok(())
    }

    // --- Skill Hub ---
    async fn kms_list_skills(self) -> Result<Vec<SkillDto>, String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        let repo = kms_repository::KmsSkillRepository;
        let mut skills = repo.list_skills().await.map_err(|e| e.to_string())?;
        
        let mut dtos = Vec::new();
        for s in &mut skills {
            let _ = s.refresh_resources();
            dtos.push(SkillDto {
                metadata: SkillMetadataDto {
                    name: s.metadata.name.clone(),
                    description: s.metadata.description.clone(),
                    version: s.metadata.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
                    author: s.metadata.author.clone(),
                    tags: s.metadata.tags.clone(),
                    license: s.metadata.license.clone(),
                    compatibility: s.metadata.compatibility.clone(),
                    metadata: s.metadata.extra_metadata.as_ref().map(|v| v.to_string()),
                    disable_model_invocation: s.metadata.disable_model_invocation,
                    scope: match s.metadata.scope {
                        digicore_core::domain::entities::skill::SkillScope::Global => "Global".to_string(),
                        digicore_core::domain::entities::skill::SkillScope::Project => "Project".to_string(),
                    },
                    sync_targets: s.metadata.sync_targets.clone(),
                },
                path: Some(s.path.to_string_lossy().to_string()),
                instructions: Some(s.instructions.clone()),
                resources: s.resources.iter().map(|r| SkillResourceDto {
                    name: r.name.clone(),
                    r#type: format!("{:?}", r.r#type),
                    rel_path: r.path.strip_prefix(&s.path).unwrap_or(&r.path).to_string_lossy().replace('\\', "/"),
                }).collect(),
            });
        }
        Ok(dtos)
    }

    async fn kms_get_skill(self, name: String) -> Result<Option<SkillDto>, String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        let repo = kms_repository::KmsSkillRepository;
        let skill_opt = repo.get_skill(&name).await.map_err(|e| e.to_string())?;
        
        if let Some(mut s) = skill_opt {
            let _ = s.refresh_resources();
            Ok(Some(SkillDto {
                metadata: SkillMetadataDto {
                    name: s.metadata.name,
                    description: s.metadata.description,
                    version: s.metadata.version.unwrap_or_else(|| "1.0.0".to_string()),
                    author: s.metadata.author,
                    tags: s.metadata.tags,
                    license: s.metadata.license,
                    compatibility: s.metadata.compatibility,
                    metadata: s.metadata.extra_metadata.map(|v| v.to_string()),
                    disable_model_invocation: s.metadata.disable_model_invocation,
                    scope: match s.metadata.scope {
                        digicore_core::domain::entities::skill::SkillScope::Global => "Global".to_string(),
                        digicore_core::domain::entities::skill::SkillScope::Project => "Project".to_string(),
                    },
                    sync_targets: s.metadata.sync_targets.clone(),
                },
                path: Some(s.path.to_string_lossy().to_string()),
                instructions: Some(s.instructions),
                resources: s.resources.into_iter().map(|r| SkillResourceDto {
                    name: r.name,
                    r#type: format!("{:?}", r.r#type),
                    rel_path: r.path.strip_prefix(&s.path).unwrap_or(&r.path).to_string_lossy().replace('\\', "/"),
                }).collect(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn kms_save_skill(self, skill: SkillDto, overwrite: bool) -> Result<(), String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        use digicore_core::domain::entities::skill::{Skill, SkillMetadata, SkillScope};
        
        let repo = kms_repository::KmsSkillRepository;
        
        let scope = if skill.metadata.scope == "Project" {
            SkillScope::Project
        } else {
            SkillScope::Global
        };

        let skill_entity = Skill {
            metadata: SkillMetadata {
                name: skill.metadata.name,
                description: skill.metadata.description,
                version: Some(skill.metadata.version),
                author: skill.metadata.author,
                tags: skill.metadata.tags,
                license: skill.metadata.license,
                compatibility: skill.metadata.compatibility,
                extra_metadata: skill.metadata.metadata.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                disable_model_invocation: skill.metadata.disable_model_invocation,
                scope,
                sync_targets: skill.metadata.sync_targets,
            },
            instructions: skill.instructions.unwrap_or_default(),
            resources: Vec::new(), // Populated by refresh_resources
            path: PathBuf::from(skill.path.unwrap_or_default()),
        };
        
        repo.save_skill(&skill_entity).await.map_err(|e| e.to_string())?;
        
        // Sync to external targets
        let _ = skill_sync::sync_skill_to_targets(&skill_entity, overwrite).await.map_err(|e| e.to_string());
        
        // Trigger reindexing
        let app = get_app(&self.app_handle);
        let indexing_service = app.state::<std::sync::Arc<indexing_service::KmsIndexingService>>();
        let _ = indexing_service.index_single_item(&app, "skills", &skill_entity.metadata.name).await;
        
        Ok(())
    }

    async fn kms_add_skill_resource(self, skill_name: String, source_path: String, target_subdir: Option<String>) -> Result<SkillResourceDto, String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        let repo = kms_repository::KmsSkillRepository;
        let mut skill = repo.get_skill(&skill_name).await.map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Skill {} not found", skill_name))?;

        let source = PathBuf::from(&source_path);
        if !source.exists() {
            return Err(format!("Source path {} does not exist", source_path));
        }

        let target_dir = if let Some(sub) = target_subdir {
            let d = skill.path.join(sub);
            std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            d
        } else {
            skill.path.clone()
        };

        let filename = source.file_name().ok_or("Invalid source filename")?;
        let target_path = target_dir.join(filename);

        if source.is_dir() {
            copy_dir_recursive(&source, &target_path).map_err(|e| e.to_string())?;
        } else {
            std::fs::copy(&source, &target_path).map_err(|e| e.to_string())?;
        }

        skill.refresh_resources().map_err(|e| e.to_string())?;
        
        let resource = skill.resources.iter().find(|r| r.path == target_path)
            .ok_or("Failed to identify newly added resource")?;

        Ok(SkillResourceDto {
            name: resource.name.clone(),
            r#type: format!("{:?}", resource.r#type),
            rel_path: resource.path.strip_prefix(&skill.path).unwrap_or(&resource.path).to_string_lossy().replace('\\', "/"),
        })
    }

    async fn kms_remove_skill_resource(self, skill_name: String, rel_path: String) -> Result<(), String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        let repo = kms_repository::KmsSkillRepository;
        let skill = repo.get_skill(&skill_name).await.map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Skill {} not found", skill_name))?;

        let abs_path = skill.path.join(rel_path.replace('/', "\\"));
        
        if !abs_path.exists() {
            return Ok(());
        }

        if abs_path.is_dir() {
            std::fs::remove_dir_all(&abs_path).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(&abs_path).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    async fn kms_delete_skill(self, name: String) -> Result<(), String> {
        use digicore_text_expander::ports::skill::SkillRepository;
        let repo = kms_repository::KmsSkillRepository;
        repo.delete_skill(&name).await.map_err(|e| e.to_string())?;
        
        // Cleanup embeddings
        let _ = kms_repository::delete_embeddings_for_entity("skill", &name);
        let _ = kms_repository::update_index_status("skills", &name, "deleted", None);
        
        Ok(())
    }

    async fn kms_sync_skills(self) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        let service = app.state::<Arc<indexing_service::KmsIndexingService>>();
        
        // This will trigger the SkillIndexProvider::index_all which currently lists skills from DB
        // In Phase 7, we'll make this scan the filesystem first.
        let _ = service.index_provider_by_id(&app, "skills").await?;
        Ok(())
    }

    async fn kms_check_skill_conflicts(self, skill_name: String, sync_targets: Vec<String>) -> Result<Vec<SyncConflictDto>, String> {
        let mut conflicts = Vec::new();
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        
        for target in &sync_targets {
             // Basic resolution for hidden folders in home dir
             let base_path = if target.starts_with('.') {
                 home.join(target)
             } else {
                 PathBuf::from(target)
             };
             
             let skill_path = base_path.join(&skill_name);
             if skill_path.exists() {
                 conflicts.push(SyncConflictDto {
                     target: target.clone(),
                     existing_name: skill_name.clone(),
                     conflict_type: "NameCollision".to_string(),
                 });
             }
        }
        
        Ok(conflicts)
    }
}

static KMS_WATCHER: OnceLock<Mutex<Option<notify::RecommendedWatcher>>> = OnceLock::new();

fn stop_kms_watcher() {
    if let Some(guard_mutex) = KMS_WATCHER.get() {
        if let Ok(mut guard) = guard_mutex.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn start_kms_watcher(app: tauri::AppHandle, path: PathBuf) {
    stop_kms_watcher();
    
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    
    // Watcher thread
    let watcher_res = notify::RecommendedWatcher::new(move |res: notify::Result<Event>| {
        if let Ok(_) = res {
            let _ = tx.blocking_send(());
        }
    }, Config::default());

    if let Ok(mut watcher) = watcher_res {
        let _ = watcher.watch(&path, RecursiveMode::Recursive);
        
        let watcher_mutex = KMS_WATCHER.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = watcher_mutex.lock() {
            *guard = Some(watcher);
        }

        // Debounce task
        tokio::spawn(async move {
            let mut last_event = std::time::Instant::now();
            let mut pending = false;
            
            loop {
                tokio::select! {
                    res = rx.recv() => {
                        if res.is_none() { break; }
                        last_event = std::time::Instant::now();
                        pending = true;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                        if pending && last_event.elapsed() >= std::time::Duration::from_millis(1000) {
                            log::info!("[KMS][Watcher] Change detected, triggering sync...");
                            let _ = app.emit("kms-sync-status", "Syncing...");
                            let _ = sync_vault_files_to_db_internal(&app, &path).await;
                            let _ = app.emit("kms-sync-status", "Idle");
                            let _ = app.emit("kms-sync-complete", ());
                            pending = false;
                        }
                    }
                }
            }
        });
    }
}

pub(crate) async fn sync_note_index_internal(rel_path_raw: &str, title: &str, content: &str) -> Result<(), String> {
    let rel_path = &rel_path_raw.replace('\\', "/");
    let preview = content.chars().take(200).collect::<String>();
    
    KmsDiagnosticService::debug(&format!("Indexing note: {}", rel_path), None);
    
    kms_repository::upsert_note(rel_path, title, &preview, "indexed", None)?;

    // Link Extraction & Resolution
    let _ = kms_repository::delete_links_for_source(rel_path);
    let candidates = extract_links_from_markdown(content);
    
    if !candidates.is_empty() {
        if let Ok(all_notes) = kms_repository::list_notes() {
            // Build title-to-path and path-to-path maps for resolution
            let title_map: HashMap<String, String> = all_notes.iter()
                .map(|n| (n.title.to_lowercase(), n.path.clone())).collect();
            let path_map: HashSet<String> = all_notes.iter()
                .map(|n| n.path.replace('\\', "/")).collect();

            let source_path = PathBuf::from(rel_path.replace('\\', "/"));
            let source_parent = source_path.parent().unwrap_or(Path::new(""));

            for candidate in candidates {
                match candidate {
                    LinkCandidate::Wiki(target_title) => {
                        if let Some(target_path) = title_map.get(&target_title.to_lowercase()) {
                            let _ = kms_repository::upsert_link(rel_path, target_path);
                        }
                    }
                    LinkCandidate::Path(mut target_path_str) => {
                        // Strip anchors
                        if let Some(hash_idx) = target_path_str.find('#') {
                            target_path_str.truncate(hash_idx);
                        }
                        
                        // Normalize target_path_str separators
                        let target_path_str = target_path_str.replace('\\', "/");
                        if target_path_str.is_empty() { continue; }

                        // Resolve relative paths if necessary
                        let resolved_path = if target_path_str.starts_with("./") || target_path_str.starts_with("../") {
                            source_parent.join(&target_path_str)
                                .components()
                                .fold(PathBuf::new(), |mut acc, comp| {
                                    match comp {
                                        std::path::Component::CurDir => {}
                                        std::path::Component::ParentDir => { acc.pop(); }
                                        std::path::Component::Normal(c) => { acc.push(c); }
                                        _ => { acc.push(comp); }
                                    }
                                    acc
                                })
                        } else {
                            PathBuf::from(&target_path_str)
                        };

                        let resolved_str = resolved_path.to_string_lossy().replace('\\', "/");
                        
                        // Check if it exists in our known paths (case-insensitive for convenience)
                        if path_map.contains(&resolved_str) {
                             let _ = kms_repository::upsert_link(rel_path, &resolved_str);
                        } else {
                            // Try title-based resolution as fallback (sometimes people link to MD but target title matters)
                            let stem = resolved_path.file_stem()
                                .map(|s| s.to_string_lossy().to_string().to_lowercase())
                                .unwrap_or_default();
                            if let Some(target_path) = title_map.get(&stem) {
                                let _ = kms_repository::upsert_link(rel_path, target_path);
                            }
                        }
                    }
                }
            }
        }
    }

    // Embedding (OFFLOADED to prevent IPC hangups)
    let content_to_embed = content.to_string();
    let rel_path_clone = rel_path.to_string();
    tokio::task::spawn_blocking(move || {
        if let Ok(vector) = embedding_service::generate_text_embedding(&content_to_embed, None) {
            let _ = kms_repository::upsert_embedding("text", "note", &rel_path_clone, vector, None);
        }
    });
    
    Ok(())
}

enum LinkCandidate {
    Wiki(String),
    Path(String),
}

fn extract_links_from_markdown(content: &str) -> Vec<LinkCandidate> {
    let mut candidates = Vec::new();

    // Wikilinks: [[Link]] or [[Link|Alias]]
    let wiki_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    for cap in wiki_re.captures_iter(content) {
        candidates.push(LinkCandidate::Wiki(cap[1].trim().to_string()));
    }

    // Standard Links: [Text](Path)
    let md_re = Regex::new(r"(?i)\[[^\]]+\]\(([^\)]+)\)").unwrap();
    for cap in md_re.captures_iter(content) {
        let path = cap[1].trim().to_string();
        // Skip external links
        if !path.starts_with("http") && !path.starts_with("mailto:") {
            candidates.push(LinkCandidate::Path(path));
        }
    }

    candidates
}

/// Robustly synchronizes the local filesystem vault with the database index.
/// Scans for new files, removes stale records, and refreshes AI embeddings.
pub(crate) async fn sync_vault_files_to_db_internal(_app: &tauri::AppHandle, vault_path: &Path) -> Result<(), String> {
    if !vault_path.exists() {
        return Ok(());
    }

    // 1. Get current DB state
    let db_notes = kms_repository::list_notes()?;
    let mut db_paths: HashMap<String, (String, String, Option<String>)> = db_notes.into_iter()
        .map(|n| (n.path, (n.title, n.sync_status, n.last_modified))).collect();

    // 2. Scan disk recursively
    let mut disk_files = Vec::new();
    fn scan_recursive(dir: &Path, root: &Path, files: &mut Vec<(PathBuf, String)>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_recursive(&path, root, files);
                } else if path.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
                    .unwrap_or(false) 
                {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        files.push((path, rel_str));
                    }
                }
            }
        }
    }
    scan_recursive(vault_path, vault_path, &mut disk_files);

    for (abs_path, rel_path) in disk_files {
        let current_title = abs_path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let db_info = db_paths.get(&rel_path);
        let status = db_info.map(|i| i.1.as_str()).unwrap_or("");
        let db_last_modified = db_info.and_then(|i| i.2.as_ref());

        // External change detection: Check if disk file is newer than DB record
        let mut disk_newer = false;
        if let Ok(metadata) = abs_path.metadata() {
            if let Ok(modified) = metadata.modified() {
                let disk_time: chrono::DateTime<chrono::Utc> = modified.into();
                let disk_time_str = disk_time.to_rfc3339();
                if let Some(db_time_str) = db_last_modified {
                    if disk_time_str > *db_time_str {
                        log::info!("[KMS][Sync] External change detected for: {}. Disk: {}, DB: {}", rel_path, disk_time_str, db_time_str);
                        disk_newer = true;
                    }
                }
            }
        }

        let needs_index = db_info.is_none() || status == "failed" || status == "pending" || disk_newer;
        let needs_rename = db_info.map(|t| t.0 != current_title).unwrap_or(false);

        if needs_index || needs_rename {
            KmsDiagnosticService::info(&format!("Syncing: {}", rel_path), None);
            
            // 1. Mark as pending in DB first (so user sees it's being worked on)
            let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None);

            // 2. Attempt to read content
            match std::fs::read_to_string(&abs_path) {
                Ok(content) => {
                    if let Err(e) = sync_note_index_internal(&rel_path, &current_title, &content).await {
                         KmsDiagnosticService::error(&format!("Failed to sync {}: {}", rel_path, e), None);
                    }
                }
                Err(e) => {
                    KmsDiagnosticService::error(&format!("Failed to read {}: {}", rel_path, e), None);
                    // Mark as failed in DB so user sees it
                    let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e.to_string()));
                }
            }
        }
        
        // Mark as found on disk (prevent stale cleanup)
        db_paths.remove(&rel_path);
    }

    // 3. Cleanup stale records (files deleted on disk)
    for (stale_rel_path, _) in db_paths {
        log::info!("[KMS][Sync] Cleaning up stale DB record: {}", stale_rel_path);
        let _ = kms_repository::delete_note(&stale_rel_path);
    }

    Ok(())
}

/// Synchronizes a single note file to the database.
pub(crate) async fn sync_single_note_to_db_internal(_app: &tauri::AppHandle, abs_path: &Path) -> Result<(), String> {
    let vault_path = kms_repository::get_vault_path()?;
    let rel_path = abs_path
        .strip_prefix(&vault_path)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("Path {} is not in vault {}", abs_path.display(), vault_path.display()))?;

    let current_title = abs_path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    // 1. Mark as pending
    let _ = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None);

    // 2. Attempt to read content
    match std::fs::read_to_string(abs_path) {
        Ok(content) => {
            sync_note_index_internal(&rel_path, &current_title, &content).await?;
            Ok(())
        }
        Err(e) => {
            log::warn!("[KMS][Sync] Failed to read {}: {}. Marking as failed.", rel_path, e);
            kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e.to_string()))?;
            Err(e.to_string())
        }
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

    #[test]
    fn test_extract_links_from_markdown() {
        let content = "Check [[WikiLink]] and [StandardLink](./Note.md#anchor) and [External](https://google.com)";
        let candidates = super::extract_links_from_markdown(content);
        assert_eq!(candidates.len(), 2);
        match &candidates[0] {
            super::LinkCandidate::Wiki(t) => assert_eq!(t, "WikiLink"),
            _ => panic!("Expected WikiLink"),
        }
        match &candidates[1] {
            super::LinkCandidate::Path(p) => assert_eq!(p, "./Note.md#anchor"),
            _ => panic!("Expected Path"),
        }
    }
}
