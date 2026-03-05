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
    /// True when file existed but JSON parse failed. Never persist in that case.
    parse_failed: bool,
}

impl JsonFileStorageAdapter {
    /// Create and load from file. Creates parent dir if needed.
    pub fn load() -> Self {
        let path = state_file_path();
        let (cache, parse_failed) = if path.exists() {
            match std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
            {
                Some(c) => (c, false),
                None => (HashMap::new(), true),
            }
        } else {
            (HashMap::new(), false)
        };
        Self {
            cache,
            path,
            parse_failed,
        }
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

    /// Persist only if safe. Returns false and skips persist when:
    /// - File existed but JSON parse failed (would overwrite valid data with partial cache)
    /// - File exists and cache is empty (parse failed, cache would wipe file)
    pub fn persist_if_safe(&self) -> std::io::Result<bool> {
        if self.parse_failed {
            return Ok(false);
        }
        if self.path.exists() && self.cache.is_empty() {
            return Ok(false);
        }
        self.persist()?;
        Ok(true)
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
            parse_failed: false,
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
            parse_failed: false,
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
            parse_failed: false,
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
            parse_failed: false,
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
            parse_failed: false,
        };
        assert_eq!(loaded.get(storage_keys::UI_LAST_TAB).as_deref(), Some("2"));
        assert_eq!(
            loaded.get(storage_keys::UI_COLUMN_ORDER).as_deref(),
            Some("Profile,Category,Trigger,Content Preview,AppLock,Options,Last Modified")
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_persist_if_safe_skips_when_parse_failed() {
        let temp = std::env::temp_dir().join("digicore_text_expander_test_parse_fail");
        std::fs::create_dir_all(&temp).unwrap();
        let path = temp.join("parse_fail_state.json");
        std::fs::write(&path, r#"{"library_path":"/path/to/snippets.json"}"#).unwrap();

        let mut storage = JsonFileStorageAdapter {
            cache: HashMap::new(),
            path: path.clone(),
            parse_failed: true,
        };
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, "100");
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, "200");
        let ok = storage.persist_if_safe().unwrap();
        assert!(!ok, "persist_if_safe must skip when parse_failed");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("library_path"), "file must not be overwritten");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_appearance_rules_create_update_delete_cycle() {
        let temp = std::env::temp_dir().join("digicore_text_expander_test_appearance_rules");
        std::fs::create_dir_all(&temp).unwrap();
        let path = temp.join("appearance_rules_state.json");
        let _ = std::fs::remove_file(&path);

        let mut storage = JsonFileStorageAdapter {
            cache: HashMap::new(),
            path: path.clone(),
            parse_failed: false,
        };

        // Create
        let create_payload = r#"[{"app_process":"cursor.exe","opacity":200,"enabled":true}]"#;
        storage.set(
            storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON,
            create_payload,
        );
        storage.persist().unwrap();

        let create_map: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let create_rules_raw = create_map
            .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
            .unwrap();
        let create_rules: serde_json::Value = serde_json::from_str(create_rules_raw).unwrap();
        assert_eq!(create_rules.as_array().unwrap().len(), 1);
        assert_eq!(create_rules[0]["app_process"], "cursor.exe");

        // Update
        let update_payload =
            r#"[{"app_process":"cursor.exe","opacity":140,"enabled":false},{"app_process":"code.exe","opacity":220,"enabled":true}]"#;
        storage.set(
            storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON,
            update_payload,
        );
        storage.persist().unwrap();

        let update_map: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let update_rules_raw = update_map
            .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
            .unwrap();
        let update_rules: serde_json::Value = serde_json::from_str(update_rules_raw).unwrap();
        assert_eq!(update_rules.as_array().unwrap().len(), 2);
        assert_eq!(update_rules[0]["opacity"], 140);
        assert_eq!(update_rules[0]["enabled"], false);

        // Delete
        let delete_payload = r#"[]"#;
        storage.set(
            storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON,
            delete_payload,
        );
        storage.persist().unwrap();

        let delete_map: HashMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let delete_rules_raw = delete_map
            .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
            .unwrap();
        let delete_rules: serde_json::Value = serde_json::from_str(delete_rules_raw).unwrap();
        assert_eq!(delete_rules.as_array().unwrap().len(), 0);

        let _ = std::fs::remove_file(&path);
    }
}
