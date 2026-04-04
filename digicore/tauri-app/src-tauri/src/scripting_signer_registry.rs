//! Scripting profile signer registry storage and trust-on-first-use updates.

use std::collections::HashMap;

use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::ports::StoragePort;

const SCRIPTING_PROFILE_SIGNER_REGISTRY_STORAGE: &str = "scripting_profile_signer_registry_json";

#[derive(Clone, Debug)]
pub(crate) struct ScriptingSignerRegistryState {
    pub allow_unknown_signers: bool,
    pub trust_on_first_use: bool,
    pub trusted_fingerprints: Vec<String>,
    pub blocked_fingerprints: Vec<String>,
    pub trusted_first_seen_utc: HashMap<String, String>,
}

pub(crate) fn normalize_fingerprint(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
}

pub(crate) fn load_scripting_signer_registry() -> ScriptingSignerRegistryState {
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

pub(crate) fn save_scripting_signer_registry(dto: &ScriptingSignerRegistryState) -> Result<(), String> {
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

pub(crate) fn upsert_trust_on_first_use_signer(signer_fingerprint: &str, source: &str) -> Result<bool, String> {
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
    crate::app_diagnostics::diag_log(
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

pub(crate) fn now_unix_secs_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
