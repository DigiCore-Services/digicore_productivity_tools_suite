//! Expansion diagnostics - ring buffer of expansion events for the Log tab.
//!
//! Used by Tauri command get_diagnostic_logs for power-user debugging
//! (why a snippet didn't expand: AppLock, no match, paused, etc.).

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ENTRIES: usize = 500;

#[derive(Clone, Debug, serde::Serialize)]
pub struct DiagnosticEntry {
    pub timestamp_ms: u64,
    pub level: String,
    pub message: String,
}

static DIAG: Mutex<VecDeque<DiagnosticEntry>> = Mutex::new(VecDeque::new());

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Push a diagnostic entry. Call from hotstring driver.
pub fn push(level: &str, message: impl Into<String>) {
    if let Ok(mut g) = DIAG.lock() {
        g.push_back(DiagnosticEntry {
            timestamp_ms: timestamp_ms(),
            level: level.to_string(),
            message: message.into(),
        });
        while g.len() > MAX_ENTRIES {
            g.pop_front();
        }
    }
}

/// Get recent entries for Tauri command (newest last).
pub fn get_recent() -> Vec<DiagnosticEntry> {
    if let Ok(g) = DIAG.lock() {
        g.iter().cloned().collect()
    } else {
        Vec::new()
    }
}

/// Clear diagnostics (for testing or user action).
pub fn clear() {
    if let Ok(mut g) = DIAG.lock() {
        g.clear();
    }
}
