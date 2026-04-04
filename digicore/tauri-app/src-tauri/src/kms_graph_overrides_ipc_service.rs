//! Bounded service for per-vault graph override CRUD IPC orchestration.

use crate::kms_graph_service;

use crate::app_settings_storage::persist_settings_for_state;
use super::ApiImpl;
use super::ipc_error;

pub(crate) async fn get_vault_graph_overrides_json(host: ApiImpl) -> Result<String, String> {
    let vault = host.get_vault_path();
    let key = kms_graph_service::vault_graph_settings_key(&vault);
    let guard = host
        .state
        .lock()
        .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&guard.kms_graph_vault_overrides_json).unwrap_or_default();
    let entry = map.get(&key).cloned().unwrap_or(serde_json::json!({}));
    serde_json::to_string_pretty(&entry)
        .map_err(|e| ipc_error("KMS_VAULT_OVERRIDES_SERIALIZE", e.to_string(), None))
}

pub(crate) async fn set_vault_graph_overrides_json(host: ApiImpl, json: String) -> Result<(), String> {
    let vault = host.get_vault_path();
    let key = kms_graph_service::vault_graph_settings_key(&vault);
    let patch: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| ipc_error("KMS_VAULT_OVERRIDES_JSON", format!("Invalid JSON: {e}"), None))?;
    if !patch.is_object() {
        return Err(ipc_error(
            "KMS_VAULT_OVERRIDES_SHAPE",
            "Overrides must be a JSON object (e.g. {\"kms_graph_k_means_max_k\":8}).",
            None,
        ));
    }
    let mut guard = host
        .state
        .lock()
        .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
    let mut map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&guard.kms_graph_vault_overrides_json).unwrap_or_default();
    map.insert(key, patch);
    guard.kms_graph_vault_overrides_json = serde_json::to_string(&map)
        .map_err(|e| ipc_error("KMS_VAULT_OVERRIDES_SERIALIZE", e.to_string(), None))?;
    drop(guard);
    persist_settings_for_state(&host.state)?;
    Ok(())
}

pub(crate) async fn clear_vault_graph_overrides_json(host: ApiImpl) -> Result<(), String> {
    let vault = host.get_vault_path();
    let key = kms_graph_service::vault_graph_settings_key(&vault);
    let mut guard = host
        .state
        .lock()
        .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
    let mut map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&guard.kms_graph_vault_overrides_json).unwrap_or_default();
    map.remove(&key);
    guard.kms_graph_vault_overrides_json = serde_json::to_string(&map)
        .map_err(|e| ipc_error("KMS_VAULT_OVERRIDES_SERIALIZE", e.to_string(), None))?;
    drop(guard);
    persist_settings_for_state(&host.state)?;
    Ok(())
}

