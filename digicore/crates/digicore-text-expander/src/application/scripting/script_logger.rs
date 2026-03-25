//! Script execution logger for GUI console.
//! SE-34: Structured diagnostics.

use once_cell::sync::Lazy;
use std::sync::Mutex;
use chrono::Local;

#[derive(Clone, Debug, serde::Serialize)]
pub struct ScriptLogEntry {
    pub timestamp: String,
    pub script_type: String,
    pub message: String,
    pub duration_ms: u128,
    pub code_len: usize,
    pub is_error: bool,
}

static LOG_BUFFER: Lazy<Mutex<Vec<ScriptLogEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));
const MAX_LOGS: usize = 100;

pub fn log_script_execution(entry: ScriptLogEntry) {
    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        buffer.push(entry);
        if buffer.len() > MAX_LOGS {
            buffer.remove(0);
        }
    }
}

pub fn get_logs() -> Vec<ScriptLogEntry> {
    LOG_BUFFER.lock().map(|b| b.clone()).unwrap_or_default()
}

pub fn clear_logs() {
    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        buffer.clear();
    }
}

/// Helper for registry dispatch to create entries.
pub fn create_log_entry(
    script_type: &str,
    message: String,
    duration_ms: u128,
    code_len: usize,
    is_error: bool,
) -> ScriptLogEntry {
    ScriptLogEntry {
        timestamp: Local::now().format("%H:%M:%S").to_string(),
        script_type: script_type.to_string(),
        message,
        duration_ms,
        code_len,
        is_error,
    }
}
