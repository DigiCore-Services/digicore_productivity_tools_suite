//! Main-window UI tab and column order preferences (JSON storage).

use super::*;

pub(crate) async fn get_ui_prefs(_host: ApiImpl) -> Result<UiPrefsDto, String> {
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

pub(crate) async fn save_ui_prefs(
    _host: ApiImpl,
    last_tab: u32,
    column_order: Vec<String>,
) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::UI_LAST_TAB, &last_tab.to_string());
    storage.set(storage_keys::UI_COLUMN_ORDER, &column_order.join(","));
    storage.persist().map_err(|e| e.to_string())
}

