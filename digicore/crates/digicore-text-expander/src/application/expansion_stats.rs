//! Expansion statistics - tracks expansions, chars saved, and top triggers.
//!
//! Persists to %APPDATA%/DigiCore/expansion_stats.json.
//! Used by Tauri command get_expansion_stats for the Analytics dashboard.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// WPM (words per minute) for time-saved estimate. ~40 WPM typical typing.
const TYPING_WPM: f64 = 40.0;
/// Average chars per word.
const CHARS_PER_WORD: f64 = 5.0;

/// Serializable expansion stats for persistence and API.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ExpansionStats {
    pub total_expansions: u64,
    pub total_chars_saved: u64,
    /// Map of trigger -> count. "ghost_follower" for Ghost Follower expansions.
    #[serde(default)]
    pub trigger_counts: HashMap<String, u64>,
}

impl ExpansionStats {
    /// Estimated time saved in seconds (chars_saved / (WPM * chars_per_word / 60)).
    pub fn estimated_time_saved_secs(&self) -> f64 {
        if self.total_chars_saved == 0 {
            return 0.0;
        }
        let words = self.total_chars_saved as f64 / CHARS_PER_WORD;
        words / (TYPING_WPM / 60.0)
    }

    /// Top triggers sorted by count descending.
    pub fn top_triggers(&self, limit: usize) -> Vec<(String, u64)> {
        let mut v: Vec<_> = self.trigger_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().take(limit).collect()
    }
}

fn stats_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DigiCore")
        .join("expansion_stats.json")
}

static STATS: Mutex<Option<ExpansionStats>> = Mutex::new(None);

fn load_stats() -> ExpansionStats {
    let path = stats_file_path();
    if path.exists() {
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(stats) = serde_json::from_str(&s) {
                return stats;
            }
        }
    }
    ExpansionStats::default()
}

fn save_stats(stats: &ExpansionStats) {
    if let Some(parent) = stats_file_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(stats) {
        let _ = std::fs::write(stats_file_path(), json);
    }
}

/// Record an expansion. Call from do_expand and do_request_expansion.
/// trigger: snippet trigger if known, or "ghost_follower" for Ghost Follower.
/// expansion_len: length of expanded content.
/// trigger_len: length of trigger typed (0 for Ghost Follower).
pub fn record_expansion(trigger: Option<&str>, expansion_len: usize, trigger_len: usize) {
    let chars_saved = expansion_len.saturating_sub(trigger_len) as u64;
    let trigger_key = trigger.unwrap_or("ghost_follower").to_string();

    let mut guard = STATS.lock().unwrap();
    let stats = guard.get_or_insert_with(load_stats);
    stats.total_expansions += 1;
    stats.total_chars_saved += chars_saved;
    *stats.trigger_counts.entry(trigger_key).or_insert(0) += 1;
    save_stats(stats);
}

/// Get current stats for Tauri command.
pub fn get_stats() -> ExpansionStats {
    let mut guard = STATS.lock().unwrap();
    let stats = guard.get_or_insert_with(load_stats);
    stats.clone()
}

/// Reset stats (for testing or user action).
pub fn reset_stats() {
    let mut guard = STATS.lock().unwrap();
    let stats = ExpansionStats::default();
    *guard = Some(stats.clone());
    save_stats(&stats);
}
