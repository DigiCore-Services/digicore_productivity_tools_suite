//! JsonFileStorageAdapter - implements StoragePort using a JSON file.
//!
//! Used for non-eframe runtimes (Tauri, Iced, etc.). Path: %APPDATA%/DigiCore/text_expander_state.json
//! (or config_dir/DigiCore/text_expander_state.json).

use crate::ports::StoragePort;
use std::collections::HashMap;
use std::path::PathBuf;

/// Path for JSON state file.
fn state_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DigiCore")
        .join("text_expander_state.json")
}

/// Adapter that persists to a JSON file. Load on init; persist on save.
pub struct JsonFileStorageAdapter {
    cache: HashMap<String, String>,
    path: PathBuf,
}

impl JsonFileStorageAdapter {
    /// Create and load from file. Creates parent dir if needed.
    pub fn load() -> Self {
        let path = state_file_path();
        let cache = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        Self { cache, path }
    }

    /// Persist cache to file. Call when saving.
    pub fn persist(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.cache).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        std::fs::write(&self.path, json)
    }
}

impl StoragePort for JsonFileStorageAdapter {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        self.cache.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::storage_keys;

    #[test]
    fn test_json_storage_get_set_persist() {
        let temp = std::env::temp_dir().join("digicore_text_expander_test");
        std::fs::create_dir_all(&temp).unwrap();
        let path = temp.join("test_state.json");
        let _ = std::fs::remove_file(&path);

        let mut storage = JsonFileStorageAdapter {
            cache: HashMap::new(),
            path: path.clone(),
        };
        assert!(storage.get(storage_keys::LIBRARY_PATH).is_none());
        storage.set(storage_keys::LIBRARY_PATH, "/path/to/lib.json");
        assert_eq!(storage.get(storage_keys::LIBRARY_PATH).as_deref(), Some("/path/to/lib.json"));
        storage.set(storage_keys::SYNC_URL, "https://example.com");
        storage.persist().unwrap();
        assert!(path.exists());

        let loaded = JsonFileStorageAdapter {
            cache: serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            path: path.clone(),
        };
        assert_eq!(loaded.get(storage_keys::LIBRARY_PATH).as_deref(), Some("/path/to/lib.json"));
        assert_eq!(loaded.get(storage_keys::SYNC_URL).as_deref(), Some("https://example.com"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_json_storage_missing_file_empty() {
        let temp = std::env::temp_dir().join("digicore_text_expander_test_nonexistent");
        let path = temp.join("nonexistent_state.json");
        let _ = std::fs::remove_file(&path);

        let storage = JsonFileStorageAdapter {
            cache: HashMap::new(),
            path: path.clone(),
        };
        assert!(storage.get(storage_keys::LIBRARY_PATH).is_none());
    }

    #[test]
    fn test_json_storage_ui_prefs() {
        let temp = std::env::temp_dir().join("digicore_text_expander_test_ui");
        std::fs::create_dir_all(&temp).unwrap();
        let path = temp.join("ui_prefs_state.json");
        let _ = std::fs::remove_file(&path);

        let mut storage = JsonFileStorageAdapter {
            cache: HashMap::new(),
            path: path.clone(),
        };
        storage.set(storage_keys::UI_LAST_TAB, "2");
        storage.set(
            storage_keys::UI_COLUMN_ORDER,
            "Profile,Category,Trigger,Content Preview,AppLock,Options,Last Modified",
        );
        storage.persist().unwrap();

        let loaded = JsonFileStorageAdapter {
            cache: serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap(),
            path: path.clone(),
        };
        assert_eq!(loaded.get(storage_keys::UI_LAST_TAB).as_deref(), Some("2"));
        assert_eq!(
            loaded.get(storage_keys::UI_COLUMN_ORDER).as_deref(),
            Some("Profile,Category,Trigger,Content Preview,AppLock,Options,Last Modified")
        );

        let _ = std::fs::remove_file(&path);
    }
}
