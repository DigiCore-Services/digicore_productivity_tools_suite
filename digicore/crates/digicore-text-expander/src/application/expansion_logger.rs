use serde::{Serialize, Deserialize};
use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::{DateTime, Local};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionEvent {
    pub timestamp: DateTime<Local>,
    pub trigger: Option<String>,
    pub content_len: usize,
    pub window_title: String,
    pub process_name: String,
    pub success: bool,
    pub error: Option<String>,
    pub method: String,
}

static LOG_PATH: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

/// Set the global expansion log path.
pub fn set_log_path(path: String) {
    if let Ok(mut g) = LOG_PATH.write() {
        *g = if path.trim().is_empty() { None } else { Some(path) };
    }
}

/// Get the effective log path.
pub fn get_log_path() -> PathBuf {
    if let Ok(g) = LOG_PATH.read() {
        if let Some(ref path) = *g {
            return PathBuf::from(path);
        }
    }
    // Default fallback
    crate::ports::data_path_resolver::DataPathResolver::logs_dir().join("digicore_expansion.json")
}

/// Log an expansion event to the persistent JSON log file.
pub fn log_event(event: ExpansionEvent) -> Result<()> {
    let log_path = get_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json = serde_json::to_string(&event)?;
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    writeln!(file, "{}", json)?;
    Ok(())
}

/// Helper to create and log a success event.
pub fn log_success(
    trigger: Option<&str>,
    content_len: usize,
    window_title: &str,
    process_name: &str,
    method: &str,
) {
    let event = ExpansionEvent {
        timestamp: Local::now(),
        trigger: trigger.map(|s| s.to_string()),
        content_len,
        window_title: window_title.to_string(),
        process_name: process_name.to_string(),
        success: true,
        error: None,
        method: method.to_string(),
    };
    let _ = log_event(event);
}

/// Helper to create and log a failure event.
pub fn log_failure(
    trigger: Option<&str>,
    window_title: &str,
    process_name: &str,
    error: &str,
) {
    let event = ExpansionEvent {
        timestamp: Local::now(),
        trigger: trigger.map(|s| s.to_string()),
        content_len: 0,
        window_title: window_title.to_string(),
        process_name: process_name.to_string(),
        success: false,
        error: Some(error.to_string()),
        method: "unknown".to_string(),
    };
    let _ = log_event(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_log_event_serialization() {
        let event = ExpansionEvent {
            timestamp: Local::now(),
            trigger: Some(";;test".to_string()),
            content_len: 10,
            window_title: "Test Window".to_string(),
            process_name: "test.exe".to_string(),
            success: true,
            error: None,
            method: "paste".to_string(),
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("\";;test\""));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_log_file_creation() {
        let event = ExpansionEvent {
            timestamp: Local::now(),
            trigger: Some(";;file_test".to_string()),
            content_len: 5,
            window_title: "File Test".to_string(),
            process_name: "file.exe".to_string(),
            success: true,
            error: None,
            method: "type".to_string(),
        };

        let result = log_event(event);
        assert!(result.is_ok());

        let log_dir = crate::ports::data_path_resolver::DataPathResolver::logs_dir();
        let log_path = log_dir.join("digicore_expansion.json");
        assert!(log_path.exists());

        let content = fs::read_to_string(log_path).expect("Failed to read log file");
        assert!(content.contains(";;file_test"));
    }
}
