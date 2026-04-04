//! Shared diagnostics bridge: expansion diagnostics buffer and leveled logging.

use digicore_text_expander::application::expansion_diagnostics;

pub(crate) fn diag_log(level: &str, message: impl Into<String>) {
    let msg = message.into();
    expansion_diagnostics::push(level, msg.clone());
    match level {
        "error" => log::error!("{msg}"),
        "warn" => log::warn!("{msg}"),
        _ => log::info!("{msg}"),
    }
}
