//! JSON Logger - structured logging for machine consumption.
//!
//! Enabled via env var: DIGICORE_LOG_JSON=1.

use log::{Level, Metadata, Record};
use serde_json::json;

pub struct JsonLogger {
    level: Level,
}

impl JsonLogger {
    /// Initialize the global logger with a maximum level.
    pub fn init(level: Level) -> anyhow::Result<()> {
        let logger = Box::new(JsonLogger { level });
        log::set_boxed_logger(logger)?;
        log::set_max_level(level.to_level_filter());
        Ok(())
    }
}

impl log::Log for JsonLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log_entry = json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": record.level().to_string(),
                "target": record.target(),
                "message": record.args().to_string(),
                "file": record.file(),
                "line": record.line(),
            });
            // Using eprintln for logs to keep stdout clean for potential pipe redirection.
            eprintln!("{}", log_entry);
        }
    }

    fn flush(&self) {}
}
