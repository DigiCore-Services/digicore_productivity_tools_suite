//! Bounded inbound service for settings bundle RPC orchestration.

use super::*;

pub(crate) async fn export_settings_bundle_to_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
    theme: Option<String>,
    autostart_enabled: Option<bool>,
) -> Result<u32, String> {
    super::settings_bundle_export_ipc_service::export_settings_bundle_to_file(
        host,
        path,
        selected_groups,
        theme,
        autostart_enabled,
    )
    .await
}

pub(crate) async fn preview_settings_bundle_from_file(
    host: ApiImpl,
    path: String,
) -> Result<SettingsBundlePreviewDto, String> {
    super::settings_bundle_preview_ipc_service::preview_settings_bundle_from_file(host, path).await
}

pub(crate) async fn import_settings_bundle_from_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
) -> Result<SettingsImportResultDto, String> {
    super::settings_bundle_import_ipc_service::import_settings_bundle_from_file(
        host,
        path,
        selected_groups,
    )
    .await
}

