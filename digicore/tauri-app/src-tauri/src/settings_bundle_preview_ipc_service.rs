//! Helper module for settings bundle preview orchestration.

use crate::settings_bundle_model::{
    normalize_settings_group, settings_bundle_schema_supported, SETTINGS_BUNDLE_SCHEMA_V1,
    SETTINGS_BUNDLE_SCHEMA_V1_1,
};

use super::*;

pub(crate) async fn preview_settings_bundle_from_file(
    _host: ApiImpl,
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

    if !settings_bundle_schema_supported(&schema) {
        valid = false;
        warnings.push(format!(
            "Unsupported schema_version '{schema}'. Expected '{}' or '{}'.",
            SETTINGS_BUNDLE_SCHEMA_V1, SETTINGS_BUNDLE_SCHEMA_V1_1
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

