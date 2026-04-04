//! Persistent KMS embedding diagnostic log (WARN/ERROR and session markers).
//!
//! File path: `%APPDATA%\\DigiCore\\logs\\kms_embedding.log` (see [`default_log_file_path`]).
//! Console filtering still uses `RUST_LOG=kms_embed=debug`; this file captures failures for offline review.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static WRITE_MUTEX: Mutex<()> = Mutex::new(());

/// Log target for KMS text embedding console output (matches `embedding_service` usage).
pub const KMS_EMBED_LOG_TARGET: &str = "kms_embed";

fn sanitize_one_line(s: &str) -> String {
    s.replace('\r', "").replace('\n', " | ")
}

/// Resolved path to `kms_embedding.log`, or `None` if the OS has no config directory.
pub fn default_log_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("DigiCore").join("logs").join("kms_embedding.log"))
}

/// Append one line to the diagnostic file (no console). Thread-safe.
pub fn append_file_only(level: &str, source: &str, message: &str) {
    let Some(path) = default_log_file_path() else {
        return;
    };
    let line = sanitize_one_line(message);
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let row = format!("[{ts}] [{level}] [{source}] {line}\n");
    let Ok(_guard) = WRITE_MUTEX.lock() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(row.as_bytes());
    }
}

/// `log::warn!` to `kms_embed` and append the same text to the diagnostic file.
pub fn warn_emit(source: &str, message: impl std::fmt::Display) {
    let s = sanitize_one_line(&message.to_string());
    log::warn!(target: KMS_EMBED_LOG_TARGET, "[{}] {}", source, s);
    append_file_only("WARN", source, &s);
}

/// `log::error!` to `kms_embed` and append to the diagnostic file.
pub fn error_emit(source: &str, message: impl std::fmt::Display) {
    let s = sanitize_one_line(&message.to_string());
    log::error!(target: KMS_EMBED_LOG_TARGET, "[{}] {}", source, s);
    append_file_only("ERROR", source, &s);
}

/// When `KMS_EMBED_LOG_FILE_DEBUG=1`, append DEBUG lines to the file only (no extra console).
pub fn debug_file_if_enabled(source: &str, message: impl std::fmt::Display) {
    if std::env::var("KMS_EMBED_LOG_FILE_DEBUG").ok().as_deref() != Some("1") {
        return;
    }
    append_file_only("DEBUG", source, &sanitize_one_line(&message.to_string()));
}

/// Session header written when D6 / full-vault re-embed starts.
pub fn session_d6_start(
    gen: u64,
    vault: &std::path::Path,
    model: &str,
    chunk_enabled: bool,
    chunk_max: u32,
    chunk_overlap: u32,
    sig_preview: &str,
) {
    append_file_only(
        "INFO",
        "session",
        &format!("======== KMS D6 / embedding session start gen={gen} ========"),
    );
    append_file_only("INFO", "session", &format!("vault={}", vault.display()));
    append_file_only("INFO", "session", &format!("model_id={}", model));
    append_file_only(
        "INFO",
        "session",
        &format!(
            "chunk_policy enabled={} max_chars={} overlap_chars={}",
            chunk_enabled, chunk_max, chunk_overlap
        ),
    );
    append_file_only(
        "INFO",
        "session",
        &format!("expected_policy_sig_prefix={}", sig_preview),
    );
    if let Some(p) = default_log_file_path() {
        append_file_only(
            "INFO",
            "session",
            &format!("this_log_file={}", p.display()),
        );
    }
}
