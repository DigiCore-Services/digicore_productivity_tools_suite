//! Scripting engine profile export/import, preview, dry-run, and detached signatures.

use crate::app_settings_storage::persist_settings_to_storage;
use crate::scripting_signer_registry::{
    load_scripting_signer_registry, normalize_fingerprint, now_unix_secs_string,
    upsert_trust_on_first_use_signer,
};

use super::*;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const SCRIPTING_PROFILE_GROUP_JAVASCRIPT: &str = "javascript";
const SCRIPTING_PROFILE_GROUP_PYTHON: &str = "python";
const SCRIPTING_PROFILE_GROUP_LUA: &str = "lua";
const SCRIPTING_PROFILE_GROUP_HTTP: &str = "http";
const SCRIPTING_PROFILE_GROUP_DSL: &str = "dsl";
const SCRIPTING_PROFILE_GROUP_RUN: &str = "run";
const SCRIPTING_PROFILE_SCHEMA_V2: &str = "2.0.0";
const SCRIPTING_PROFILE_SIGN_ALGO: &str = "ed25519-sha256-v1";
const SCRIPTING_PROFILE_SIGNING_KEY_STORAGE: &str = "scripting_profile_signing_key_b64";

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

pub(crate) fn normalized_selected_scripting_profile_groups(groups: &[String]) -> Vec<String> {
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
pub(crate) struct ParsedScriptingProfileBundle {
    pub(crate) schema_version_used: String,
    pub(crate) selected_groups: Vec<String>,
    pub(crate) groups_obj: serde_json::Map<String, serde_json::Value>,
    pub(crate) warnings: Vec<String>,
    pub(crate) valid: bool,
    pub(crate) signed_bundle: bool,
    pub(crate) signature_valid: bool,
    pub(crate) migrated_from_schema: Option<String>,
    pub(crate) signature_key_id: Option<String>,
    pub(crate) signer_fingerprint: Option<String>,
    pub(crate) signer_trusted: bool,
    pub(crate) signer_unknown: bool,
    pub(crate) trust_on_first_use: bool,
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

pub(crate) fn parse_scripting_profile_bundle(path: &str) -> Result<ParsedScriptingProfileBundle, String> {
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

pub(crate) async fn export_scripting_profile_to_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
) -> Result<u32, String> {
    let groups = normalized_selected_scripting_profile_groups(&selected_groups);
    if groups.is_empty() {
        return Err("No valid scripting profile groups selected for export.".to_string());
    }

    let js_library = super::scripting_ipc_service::get_script_library_js(host.clone()).await?;
    let py_library = super::scripting_ipc_service::get_script_library_py(host.clone()).await?;
    let lua_library = super::scripting_ipc_service::get_script_library_lua(host.clone()).await?;
    let cfg = get_scripting_config();
    let guard = host.state.lock().map_err(|e| e.to_string())?;

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
    super::diag_log(
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

pub(crate) async fn export_scripting_profile_with_detached_signature_to_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
) -> Result<ScriptingDetachedSignatureExportDto, String> {
    export_scripting_profile_to_file(host.clone(), path.clone(), selected_groups).await?;

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

    super::diag_log(
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

pub(crate) async fn preview_scripting_profile_from_file(
    _host: ApiImpl,
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
        super::diag_log(
            "info",
            format!(
                "[ScriptingProfilePreview] OK path={} groups={}",
                path,
                available_groups.len()
            ),
        );
    } else {
        super::diag_log(
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

pub(crate) async fn dry_run_import_scripting_profile_from_file(
    host: ApiImpl,
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

    let current_js = super::scripting_ipc_service::get_script_library_js(host.clone()).await?;
    let current_py = super::scripting_ipc_service::get_script_library_py(host.clone()).await?;
    let current_lua = super::scripting_ipc_service::get_script_library_lua(host.clone()).await?;
    let cfg = get_scripting_config();
    let guard = host.state.lock().map_err(|e| e.to_string())?;

    let mut changed = BTreeSet::new();
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

pub(crate) async fn import_scripting_profile_from_file(
    host: ApiImpl,
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
        super::diag_log("error", format!("[ScriptingProfileImport] {msg}"));
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
                    super::scripting_ipc_service::save_script_library_js(host.clone(), library.to_string()).await?;
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
                    super::scripting_ipc_service::save_script_library_py(host.clone(), library.to_string()).await?;
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
                    super::scripting_ipc_service::save_script_library_lua(host.clone(), library.to_string()).await?;
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
                let mut guard = host.state.lock().map_err(|e| e.to_string())?;
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
        super::diag_log(
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
        super::diag_log(
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

#[cfg(test)]
mod tests {
    use super::{normalized_selected_scripting_profile_groups, parse_scripting_profile_bundle};
    use std::io::Write;

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
}
