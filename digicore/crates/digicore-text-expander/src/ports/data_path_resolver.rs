use std::path::PathBuf;

/// Centralized resolver for all application data paths.
/// Standardizes on `%AppData%\com.digicore.text-expander`.
pub struct DataPathResolver;

impl DataPathResolver {
    /// Root directory for all application data.
    pub fn root() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("com.digicore.text-expander")
    }

    /// Path to the primary SQLite database.
    pub fn db_path() -> PathBuf {
        Self::root().join("digicore.db")
    }

    /// Directory for configuration files.
    pub fn config_dir() -> PathBuf {
        Self::root().join("config")
    }

    /// Directory for user scripts.
    pub fn scripts_dir() -> PathBuf {
        Self::root().join("scripts")
    }

    /// Directory for clipboard JSON history.
    pub fn clipboard_json_dir() -> PathBuf {
        Self::root().join("clipboard-json")
    }

    /// Directory for clipboard images and thumbnails.
    pub fn clipboard_images_dir() -> PathBuf {
        Self::root().join("clipboard-images")
    }

    /// Path to the UI preferences/state JSON file.
    pub fn state_file_path() -> PathBuf {
        Self::root().join("text_expander_state.json")
    }

    /// Path to the expansion statistics JSON file.
    pub fn stats_file_path() -> PathBuf {
        Self::root().join("expansion_stats.json")
    }

    /// Path to the script library JSON file.
    pub fn script_library_path() -> PathBuf {
        Self::root().join("text_expansion_library.json")
    }
}
