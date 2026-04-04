//! Settings bundle export/import: group ids, schema versions, and selection normalization.

pub const SETTINGS_GROUP_TEMPLATES: &str = "templates";
pub const SETTINGS_GROUP_SYNC: &str = "sync";
pub const SETTINGS_GROUP_DISCOVERY: &str = "discovery";
pub const SETTINGS_GROUP_GHOST_SUGGESTOR: &str = "ghost_suggestor";
pub const SETTINGS_GROUP_GHOST_FOLLOWER: &str = "ghost_follower";
pub const SETTINGS_GROUP_CLIPBOARD_HISTORY: &str = "clipboard_history";
pub const SETTINGS_GROUP_COPY_TO_CLIPBOARD: &str = "copy_to_clipboard";
pub const SETTINGS_GROUP_CORE: &str = "core";
pub const SETTINGS_GROUP_SCRIPT_RUNTIME: &str = "script_runtime";
pub const SETTINGS_GROUP_APPEARANCE: &str = "appearance";
pub const SETTINGS_GROUP_KMS_GRAPH: &str = "kms_graph";

/// Settings bundle baseline.
pub const SETTINGS_BUNDLE_SCHEMA_V1: &str = "1.0.0";
/// Adds `kms_graph_vault_overrides` in the `kms_graph` group (same payload shape as late 1.0.0 exports).
pub const SETTINGS_BUNDLE_SCHEMA_V1_1: &str = "1.1.0";

pub fn settings_bundle_schema_supported(schema: &str) -> bool {
    schema == SETTINGS_BUNDLE_SCHEMA_V1 || schema == SETTINGS_BUNDLE_SCHEMA_V1_1
}

pub fn all_settings_groups() -> Vec<&'static str> {
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
        SETTINGS_GROUP_KMS_GRAPH,
    ]
}

pub fn normalize_settings_group(raw: &str) -> Option<&'static str> {
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
        "kms_graph" | "kms-graph" | "knowledge_graph" | "knowledge graph" => {
            Some(SETTINGS_GROUP_KMS_GRAPH)
        }
        _ => None,
    }
}

pub fn normalized_selected_groups(groups: &[String]) -> Vec<String> {
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
