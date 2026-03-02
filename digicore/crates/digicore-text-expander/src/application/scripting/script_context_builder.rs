//! ScriptContext builder (SE-14): Dedicated service for building context from TemplateConfig.
//!
//! Uses platform adapters (chrono for date/time) for consistent resolution.
//! Caller passes date_format, time_format, clipboard, clip_history from TemplateConfig.

use super::ScriptContext;
use chrono::Local;
use std::collections::HashMap;

/// Build ScriptContext from template config fields and clipboard.
pub fn build_from_template_config(
    date_format: &str,
    time_format: &str,
    clipboard: &str,
    clip_history: &[String],
    user_vars: Option<&HashMap<String, String>>,
) -> ScriptContext {
    ScriptContext {
        clipboard: clipboard.to_string(),
        clip_history: clip_history.to_vec(),
        date: Local::now().format(date_format).to_string(),
        time: Local::now().format(time_format).to_string(),
        user_vars: user_vars.map(|m| m.clone()).unwrap_or_default(),
    }
}
