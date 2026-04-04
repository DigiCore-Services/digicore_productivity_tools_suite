//! Recent KMS graph build summaries for `kms_export_graph_diagnostics` (works without opening the graph tab).

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

const CAP: usize = 32;

#[derive(Clone, Serialize)]
pub struct KmsGraphBuildRingEntry {
    pub kind: String,
    pub recorded_at_unix_ms: u64,
    pub request_id: String,
    pub build_time_ms: u32,
    pub node_count: usize,
    pub edge_count: usize,
    pub beam_count: usize,
    pub warning_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<serde_json::Value>,
    /// First few DTO warning strings (trimmed) for support exports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings_tail: Option<Vec<String>>,
}

static RING: Mutex<VecDeque<KmsGraphBuildRingEntry>> = Mutex::new(VecDeque::new());

/// First up to 5 warnings, each trimmed and capped for export payload size.
pub fn truncate_warnings_for_ring(warnings: &[String]) -> Option<Vec<String>> {
    if warnings.is_empty() {
        return None;
    }
    let v: Vec<String> = warnings
        .iter()
        .take(5)
        .map(|s| s.trim().chars().take(220).collect::<String>())
        .filter(|s| !s.is_empty())
        .collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

pub fn unix_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn push_graph_build_entry(entry: KmsGraphBuildRingEntry) {
    let mut guard = RING.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() >= CAP {
        guard.pop_front();
    }
    guard.push_back(entry);
}

/// Oldest-first (FIFO), same order as insertion.
pub fn snapshot_ring_oldest_first() -> Vec<KmsGraphBuildRingEntry> {
    let guard = RING.lock().unwrap_or_else(|e| e.into_inner());
    guard.iter().cloned().collect()
}

