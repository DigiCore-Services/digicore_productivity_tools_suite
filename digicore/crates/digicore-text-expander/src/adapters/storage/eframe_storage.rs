//! EframeStorageAdapter - implements StoragePort using eframe::Storage.
//!
//! Loads from eframe storage at creation; persists to eframe storage when
//! save_to_eframe is called from App::save. Used by egui GUI.

use crate::ports::StoragePort;
use std::collections::HashMap;

/// Adapter that wraps eframe::Storage for framework-agnostic StoragePort.
///
/// Holds an in-memory cache. Load from eframe at init; persist to eframe on save.
pub struct EframeStorageAdapter {
    cache: HashMap<String, String>,
}

impl EframeStorageAdapter {
    /// Create adapter and load values from eframe storage (if available).
    pub fn load_from(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut cache = HashMap::new();
        if let Some(s) = storage {
            for key in &[
                crate::ports::storage_keys::LIBRARY_PATH,
                crate::ports::storage_keys::SYNC_URL,
                crate::ports::storage_keys::TEMPLATE_DATE_FORMAT,
                crate::ports::storage_keys::TEMPLATE_TIME_FORMAT,
                crate::ports::storage_keys::SCRIPT_LIBRARY_RUN_DISABLED,
                crate::ports::storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST,
                crate::ports::storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS,
            ] {
                if let Some(v) = s.get_string(key) {
                    cache.insert((*key).to_string(), v);
                }
            }
        }
        Self { cache }
    }

    /// Persist cache to eframe storage. Call from App::save.
    pub fn save_to_eframe(&self, storage: &mut dyn eframe::Storage) {
        for (k, v) in &self.cache {
            storage.set_string(k, v.clone());
        }
    }
}

impl StoragePort for EframeStorageAdapter {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        self.cache.insert(key.to_string(), value.to_string());
    }
}
