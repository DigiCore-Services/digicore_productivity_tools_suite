use crate::kms_repository;
use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

pub struct KmsDiagnosticService;

impl KmsDiagnosticService {
    pub fn log(level: LogLevel, message: &str, details: Option<String>) {
        let level_str = level.as_str();
        
        // Always log to standard internal logs first
        match level {
            LogLevel::Info => log::info!("[KMS] {}", message),
            LogLevel::Warn => log::warn!("[KMS] {}", message),
            LogLevel::Error => log::error!("[KMS] {}", message),
            LogLevel::Debug => log::debug!("[KMS] {}", message),
        }

        // Persist to database for UI visibility
        let _ = kms_repository::insert_log(level_str, message, details.as_deref());
    }

    pub fn info(message: &str, details: Option<String>) {
        Self::log(LogLevel::Info, message, details);
    }

    pub fn warn(message: &str, details: Option<String>) {
        Self::log(LogLevel::Warn, message, details);
    }

    pub fn error(message: &str, details: Option<String>) {
        Self::log(LogLevel::Error, message, details);
    }

    pub fn debug(message: &str, details: Option<String>) {
        Self::log(LogLevel::Debug, message, details);
    }
}
