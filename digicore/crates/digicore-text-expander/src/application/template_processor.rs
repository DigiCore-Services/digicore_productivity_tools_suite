//! Template processor (F11-F25): Resolves placeholders in snippet content.
//!
//! P0 placeholders (non-UI):
//! - {date} - current date (configurable format)
//! - {time} - current time (default format)
//! - {time:fmt} - current time with format
//! - {clipboard} - current clipboard content
//! - {clip:1}..{clip:N} - clipboard history (1=most recent)
//! - {env:VAR} - environment variable
//! - {tz}, {timezone} - timezone abbreviation / full name
//! - {am/pm} - AM or PM
//! - {uuid} - UUID v4 (e.g. 550e8400-e29b-41d4-a716-446655440000)
//! - {random:N} - N random chars A-Z
//! - {js:...} - JavaScript expression (via Scripting Engine)
//! - {http:url|path} - HTTP GET with optional JSON path
//! - {run:cmd} - Shell command (allowlist + disable; Phase D)

use crate::application::scripting::{
    dispatch_script_placeholder, parse_placeholder_at, ParsedPlaceholder, SCRIPT_TYPE_PREFIXES,
};
use crate::platform::timezone;
use chrono::Local;
use rand::Rng;
use std::sync::Mutex;
use uuid::Uuid;

/// User-configurable template settings (GUI Configuration tab).
#[derive(Clone, Debug)]
pub struct TemplateConfig {
    /// Default date format (chrono strftime, e.g. %Y-%m-%d).
    pub date_format: String,
    /// Default time format (chrono strftime, e.g. %H:%M).
    pub time_format: String,
    /// Max clip history index for {clip:N} (1..=clip_max_depth).
    pub clip_max_depth: usize,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M".to_string(),
            clip_max_depth: 5,
        }
    }
}

static TEMPLATE_CONFIG: Mutex<Option<TemplateConfig>> = Mutex::new(None);

/// Set template config (called from GUI when user changes settings).
pub fn set_config(config: TemplateConfig) {
    if let Ok(mut g) = TEMPLATE_CONFIG.lock() {
        *g = Some(config);
    }
}

/// Get current template config.
pub fn get_config() -> TemplateConfig {
    TEMPLATE_CONFIG
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

/// Interactive variable type for VariableInputModal.
#[derive(Clone, Debug)]
pub enum InteractiveVarType {
    Edit,       // {var:label} - text input
    Choice,     // {choice:label|opt1|opt2} - dropdown
    Checkbox,  // {checkbox:label|value} - checked -> value, unchecked -> ""
    DatePicker, // {date_picker:label} - date input (YYYYMMDD)
    FilePicker, // {file_picker:label} - file path input
}

/// A single interactive variable to collect from user.
#[derive(Clone, Debug)]
pub struct InteractiveVar {
    pub tag: String,
    pub label: String,
    pub var_type: InteractiveVarType,
    /// For Choice: options as Vec
    pub options: Vec<String>,
}

/// Collect all interactive variables from content (var, choice). Used to show VariableInputModal.
pub fn collect_interactive_vars(content: &str) -> Vec<InteractiveVar> {
    let mut vars = Vec::new();
    let mut i = 0;
    let bytes = content.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some((tag, end)) = parse_interactive_var(&content[i..]) {
                if !vars.iter().any(|v: &InteractiveVar| v.tag == tag) {
                    vars.push(tag_to_interactive_var(&tag));
                }
                i += end;
                continue;
            }
        }
        let ch = content[i..].chars().next().unwrap_or('\0');
        i += ch.len_utf8();
    }
    vars
}

fn parse_interactive_var(s: &str) -> Option<(String, usize)> {
    let end = s.find('}')?;
    let inner = &s[1..end];
    let full_len = end + 1;
    if let Some(label) = inner.strip_prefix("var:") {
        return Some((format!("{{var:{}}}", label.trim()), full_len));
    }
    if let Some(raw) = inner.strip_prefix("choice:") {
        return Some((format!("{{choice:{}}}", raw.trim()), full_len));
    }
    if let Some(raw) = inner.strip_prefix("checkbox:") {
        return Some((format!("{{checkbox:{}}}", raw.trim()), full_len));
    }
    if let Some(label) = inner.strip_prefix("date_picker:") {
        return Some((format!("{{date_picker:{}}}", label.trim()), full_len));
    }
    if let Some(raw) = inner.strip_prefix("file_picker:") {
        return Some((format!("{{file_picker:{}}}", raw.trim()), full_len));
    }
    None
}

fn tag_to_interactive_var(tag: &str) -> InteractiveVar {
    if let Some(inner) = tag.strip_prefix("{var:").and_then(|s| s.strip_suffix('}')) {
        return InteractiveVar {
            tag: tag.to_string(),
            label: inner.trim().to_string(),
            var_type: InteractiveVarType::Edit,
            options: Vec::new(),
        };
    }
    if let Some(inner) = tag.strip_prefix("{choice:").and_then(|s| s.strip_suffix('}')) {
        let parts: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
        let (label, options) = if parts.len() >= 2 {
            (parts[0].to_string(), parts[1..].iter().map(|s| (*s).to_string()).collect())
        } else {
            (inner.to_string(), Vec::new())
        };
        return InteractiveVar {
            tag: tag.to_string(),
            label,
            var_type: InteractiveVarType::Choice,
            options,
        };
    }
    if let Some(inner) = tag.strip_prefix("{checkbox:").and_then(|s| s.strip_suffix('}')) {
        let parts: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
        let (label, value) = if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (inner.to_string(), String::new())
        };
        return InteractiveVar {
            tag: tag.to_string(),
            label,
            var_type: InteractiveVarType::Checkbox,
            options: vec![value],
        };
    }
    if let Some(inner) = tag.strip_prefix("{date_picker:").and_then(|s| s.strip_suffix('}')) {
        return InteractiveVar {
            tag: tag.to_string(),
            label: inner.trim().to_string(),
            var_type: InteractiveVarType::DatePicker,
            options: Vec::new(),
        };
    }
    if let Some(inner) = tag.strip_prefix("{file_picker:").and_then(|s| s.strip_suffix('}')) {
        let parts: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
        let (label, filter) = if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (inner.trim().to_string(), "All Files (*.*)".to_string())
        };
        return InteractiveVar {
            tag: tag.to_string(),
            label,
            var_type: InteractiveVarType::FilePicker,
            options: vec![filter],
        };
    }
    InteractiveVar {
        tag: tag.to_string(),
        label: String::new(),
        var_type: InteractiveVarType::Edit,
        options: Vec::new(),
    }
}

/// Process snippet content, resolving all placeholders.
///
/// # Arguments
/// * `content` - Raw snippet content (may contain {placeholder}s)
/// * `current_clipboard` - Current clipboard text (for {clipboard})
/// * `clip_history` - Clipboard history entries, most recent first (for {clip:N})
/// * `user_vars` - Optional map of tag -> value for {var:}, {choice:} (from VariableInputModal)
pub fn process(
    content: &str,
    current_clipboard: Option<&str>,
    clip_history: &[String],
) -> String {
    process_with_user_vars(content, current_clipboard, clip_history, None)
}

/// Process with user-provided values for interactive vars. If user_vars is None, {var:} and {choice:} are left as-is.
pub fn process_with_user_vars(
    content: &str,
    current_clipboard: Option<&str>,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let config = get_config();
    process_with_config_and_user_vars(
        content,
        &config,
        current_clipboard,
        clip_history,
        user_vars,
    )
}

/// Substitute {key:...} and {wait:...} with [KEY:...] and [WAIT:...ms] for preview display (AHK-style).
fn substitute_key_wait_for_preview(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let rest = &s[i..];
            if let Some(inner) = rest.strip_prefix("{key:") {
                if let Some(end) = inner.find('}') {
                    let key_name = inner[..end].trim();
                    result.push_str(&format!("[KEY:{}]", key_name));
                    i += 5 + end + 1;
                    continue;
                }
            }
            if let Some(inner) = rest.strip_prefix("{wait:") {
                if let Some(end) = inner.find('}') {
                    let ms = inner[..end].trim();
                    result.push_str(&format!("[WAIT:{}ms]", ms));
                    i += 6 + end + 1;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap_or('\0');
        let ch_len = ch.len_utf8();
        result.push(ch);
        i += ch_len;
    }
    result
}

/// Process content for Preview Expansion: resolves placeholders, then substitutes {key:}/{wait:} as [KEY:...]/[WAIT:...ms].
/// {run:} executes with allowlist (same as expansion).
pub fn process_for_preview(
    content: &str,
    current_clipboard: Option<&str>,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let processed = process_with_user_vars(content, current_clipboard, clip_history, user_vars);
    substitute_key_wait_for_preview(&processed)
}

/// Process with explicit config (for tests).
pub fn process_with_config(
    content: &str,
    config: &TemplateConfig,
    current_clipboard: Option<&str>,
    clip_history: &[String],
) -> String {
    process_with_config_and_user_vars(content, config, current_clipboard, clip_history, None)
}

fn process_with_config_and_user_vars(
    content: &str,
    config: &TemplateConfig,
    current_clipboard: Option<&str>,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let mut result = String::with_capacity(content.len());
    let mut i = 0;
    let bytes = content.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(parsed) = parse_placeholder_at(&content[i..], SCRIPT_TYPE_PREFIXES) {
                let len = parsed.len();
                if let Some(replacement) = resolve_placeholder(
                    &parsed,
                    config,
                    current_clipboard,
                    clip_history,
                    user_vars,
                ) {
                    result.push_str(&replacement);
                    i += len;
                    continue;
                }
            }
            // Unrecognized placeholder - keep literal, advance past '{'
        }
        let ch = content[i..].chars().next().unwrap_or('\0');
        let ch_len = ch.len_utf8();
        result.push(ch);
        i += ch_len;
    }
    result
}

/// Resolve a parsed placeholder to replacement string. Returns None for unknown placeholders.
fn resolve_placeholder(
    parsed: &ParsedPlaceholder<'_>,
    config: &TemplateConfig,
    current_clipboard: Option<&str>,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> Option<String> {
    match parsed {
        ParsedPlaceholder::Script { prefix, inner, .. } => {
            let clipboard = current_clipboard.unwrap_or("");
            dispatch_script_placeholder(
                prefix,
                inner,
                &config.date_format,
                &config.time_format,
                clipboard,
                clip_history,
                user_vars,
            )
        }
        ParsedPlaceholder::Simple { inner, .. } => resolve_simple_placeholder(
            inner,
            config,
            current_clipboard,
            clip_history,
            user_vars,
        ),
    }
}

/// Resolve simple placeholder inner content to replacement string.
fn resolve_simple_placeholder(
    inner: &str,
    config: &TemplateConfig,
    current_clipboard: Option<&str>,
    clip_history: &[String],
    user_vars: Option<&std::collections::HashMap<String, String>>,
) -> Option<String> {
    let replacement = if inner == "date" {
        Local::now().format(&config.date_format).to_string()
    } else if inner == "time" {
        Local::now().format(&config.time_format).to_string()
    } else if let Some(fmt) = inner.strip_prefix("time:") {
        Local::now().format(fmt.trim()).to_string()
    } else if inner == "clipboard" {
        current_clipboard.unwrap_or("").to_string()
    } else if let Some(n_str) = inner.strip_prefix("clip:") {
        let n: usize = n_str.trim().parse().ok()?;
        if n >= 1 && n <= config.clip_max_depth {
            clip_history.get(n - 1).cloned().unwrap_or_default()
        } else {
            String::new()
        }
    } else if let Some(var) = inner.strip_prefix("env:") {
        std::env::var(var.trim()).unwrap_or_default()
    } else if inner == "tz" {
        timezone::get_timezone_abbrev()
    } else if inner == "timezone" {
        timezone::get_timezone_full()
    } else if inner == "am/pm" {
        Local::now().format("%p").to_string()
    } else if inner == "uuid" {
        Uuid::new_v4().to_string()
    } else if let Some(n_str) = inner.strip_prefix("random:") {
        let n: usize = n_str.trim().parse().ok().unwrap_or(0);
        let n = n.min(64);
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| (b'A' + rng.gen_range(0..26)) as char)
            .collect::<String>()
    } else if let Some(label) = inner.strip_prefix("var:") {
        let tag = format!("{{var:{}}}", label.trim());
        user_vars
            .and_then(|uv| uv.get(&tag))
            .cloned()
            .unwrap_or_else(|| tag)
    } else if let Some(raw) = inner.strip_prefix("choice:") {
        let tag = format!("{{choice:{}}}", raw.trim());
        user_vars
            .and_then(|uv| uv.get(&tag))
            .cloned()
            .unwrap_or_else(|| tag)
    } else if let Some(raw) = inner.strip_prefix("checkbox:") {
        let tag = format!("{{checkbox:{}}}", raw.trim());
        user_vars
            .and_then(|uv| uv.get(&tag))
            .cloned()
            .unwrap_or_else(|| tag)
    } else if let Some(label) = inner.strip_prefix("date_picker:") {
        let tag = format!("{{date_picker:{}}}", label.trim());
        user_vars
            .and_then(|uv| uv.get(&tag))
            .cloned()
            .unwrap_or_else(|| tag)
    } else if let Some(label) = inner.strip_prefix("file_picker:") {
        let tag = format!("{{file_picker:{}}}", label.trim());
        user_vars
            .and_then(|uv| uv.get(&tag))
            .cloned()
            .unwrap_or_else(|| tag)
    } else {
        return None;
    };

    Some(replacement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_placeholder() {
        let config = TemplateConfig {
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M".to_string(),
            clip_max_depth: 5,
        };
        let out = process_with_config("{date}", &config, None, &[]);
        assert!(out.len() >= 10);
        assert!(out.chars().all(|c| c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn test_time_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{time}", &config, None, &[]);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_time_with_format() {
        let config = TemplateConfig::default();
        let out = process_with_config("{time:%Y}", &config, None, &[]);
        assert!(out.len() == 4);
        assert!(out.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_clipboard_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{clipboard}", &config, Some("hello"), &[]);
        assert_eq!(out, "hello");
    }

    #[test]
    fn test_clip_placeholder() {
        let config = TemplateConfig {
            clip_max_depth: 5,
            ..Default::default()
        };
        let history = vec!["first".to_string(), "second".to_string()];
        let out = process_with_config("{clip:1}", &config, None, &history);
        assert_eq!(out, "first");
        let out = process_with_config("{clip:2}", &config, None, &history);
        assert_eq!(out, "second");
    }

    #[test]
    fn test_env_placeholder() {
        std::env::set_var("TEST_TEMPLATE_VAR", "env_value");
        let config = TemplateConfig::default();
        let out = process_with_config("{env:TEST_TEMPLATE_VAR}", &config, None, &[]);
        assert_eq!(out, "env_value");
        std::env::remove_var("TEST_TEMPLATE_VAR");
    }

    #[test]
    fn test_uuid_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{uuid}", &config, None, &[]);
        assert_eq!(out.len(), 36);
        assert!(out.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        assert_eq!(out.matches('-').count(), 4);
    }

    #[test]
    fn test_random_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{random:5}", &config, None, &[]);
        assert_eq!(out.len(), 5);
        assert!(out.chars().all(|c| c.is_ascii_uppercase()));
    }

    #[test]
    #[serial_test::serial]
    fn test_run_placeholder_disabled() {
        use crate::application::scripting::{get_scripting_config, set_scripting_config, ScriptingConfig};

        let mut cfg = get_scripting_config();
        cfg.run.disabled = true;
        set_scripting_config(cfg);

        let config = TemplateConfig::default();
        let out = process_with_config("{run:hostname}", &config, None, &[]);
        assert_eq!(out, "[Run disabled by config]");

        set_scripting_config(ScriptingConfig::default());
    }

    #[test]
    #[serial_test::serial]
    fn test_run_placeholder_blocked() {
        use crate::application::scripting::{get_scripting_config, set_scripting_config, ScriptingConfig};

        let mut cfg = get_scripting_config();
        cfg.run.disabled = false;
        cfg.run.allowlist = "python,cmd".to_string();
        set_scripting_config(cfg);

        let config = TemplateConfig::default();
        let out = process_with_config("{run:evil_command}", &config, None, &[]);
        assert_eq!(out, "[Run blocked: not in allowlist]");

        set_scripting_config(ScriptingConfig::default());
    }

    #[test]
    fn test_mixed_content() {
        let config = TemplateConfig::default();
        let out = process_with_config(
            "Date: {date} Clipboard: {clipboard}",
            &config,
            Some("clip"),
            &[],
        );
        assert!(out.starts_with("Date: "));
        assert!(out.contains("Clipboard: clip"));
    }

    #[test]
    fn test_unknown_placeholder_preserved() {
        let config = TemplateConfig::default();
        let out = process_with_config("Hello {var:name} world", &config, None, &[]);
        assert_eq!(out, "Hello {var:name} world");
    }

    #[test]
    fn test_process_for_preview_key_wait_substitution() {
        let out = process_for_preview("Hello {key:Enter} world {wait:500} done", None, &[], None);
        assert_eq!(out, "Hello [KEY:Enter] world [WAIT:500ms] done");
    }

    #[test]
    fn test_js_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("Logic: 10 + 20 = {js: 10 + 20}", &config, None, &[]);
        assert_eq!(out, "Logic: 10 + 20 = 30");
    }

    #[test]
    #[serial_test::serial]
    fn test_js_with_mock_engine() {
        use crate::application::scripting::{set_registry, MockScriptEngine, ScriptingRegistry};
        use std::sync::Arc;

        let mock = MockScriptEngine::new();
        mock.expect("js", "10 + 20", Ok("30".to_string()));
        let registry = ScriptingRegistry {
            engine: Arc::new(mock),
            http_fetcher: Arc::new(crate::application::scripting::ReqwestHttpFetcher),
        };
        set_registry(registry);

        let config = TemplateConfig::default();
        let out = process_with_config("Result: {js: 10 + 20}", &config, None, &[]);
        assert_eq!(out, "Result: 30");

        set_registry(ScriptingRegistry::default());
    }

    #[test]
    #[serial_test::serial]
    fn test_js_clipboard_placeholder_resolved() {
        let config = TemplateConfig::default();
        // {clipboard} inside {js:...} is pre-resolved to actual value
        let out = process_with_config(
            r#"{js: "{clipboard}".length}"#,
            &config,
            Some("hello"),
            &[],
        );
        assert_eq!(out, "5");
    }

    #[test]
    #[serial_test::serial]
    fn test_js_recursion_limit() {
        use crate::application::scripting::{get_scripting_config, set_scripting_config, ScriptingConfig};

        let mut cfg = get_scripting_config();
        cfg.js.recursion_limit = 50;
        set_scripting_config(cfg);

        let config = TemplateConfig::default();
        let recursive = r#"{js: (function f(n){ return n<=0 ? 0 : 1 + f(n-1); })(100) }"#;
        let out = process_with_config(recursive, &config, None, &[]);
        // Boa may report "stack overflow", "recursion", "Error", etc.
        assert!(
            out.contains("[JS Error") || out.contains("recursion") || out.contains("Error")
                || out.contains("stack") || out.contains("overflow") || out.contains("limit")
                || out.contains("RangeError"),
            "expected error output, got: {}",
            out
        );

        set_scripting_config(ScriptingConfig::default());
    }

    #[test]
    fn test_js_clipboard_when_clipboard_contains_template() {
        let config = TemplateConfig::default();
        // When clipboard contains the template (including "{clipboard}"), we must not
        // recursively replace - only the quoted "{clipboard}" in the template is replaced.
        let clipboard = r#"{js: clipClean("{clipboard}")}"#;
        let out = process_with_config(
            r#"{js: "{clipboard}".toUpperCase()}"#,
            &config,
            Some(clipboard),
            &[],
        );
        // Should get the clipboard as literal string, uppercased - no syntax error
        assert_eq!(out, r#"{JS: CLIPCLEAN("{CLIPBOARD}")}"#);
    }

    #[test]
    fn test_collect_interactive_vars_checkbox_datepicker_filepicker() {
        let vars = collect_interactive_vars(
            "Options: {checkbox:Include?|-SkipNoCache} Date: {date_picker:When} File: {file_picker:Path}",
        );
        assert_eq!(vars.len(), 3);
        let checkbox = vars.iter().find(|v| v.tag == "{checkbox:Include?|-SkipNoCache}");
        assert!(checkbox.is_some());
        let cb = checkbox.unwrap();
        assert_eq!(cb.label, "Include?");
        assert_eq!(cb.options, vec!["-SkipNoCache"]);
        assert!(matches!(cb.var_type, InteractiveVarType::Checkbox));

        let date = vars.iter().find(|v| v.tag == "{date_picker:When}");
        assert!(date.is_some());
        assert_eq!(date.unwrap().label, "When");
        assert!(matches!(date.unwrap().var_type, InteractiveVarType::DatePicker));

        let file = vars.iter().find(|v| v.tag == "{file_picker:Path}");
        assert!(file.is_some());
        let f = file.unwrap();
        assert_eq!(f.label, "Path");
        assert_eq!(f.options, vec!["All Files (*.*)"]);
        assert!(matches!(f.var_type, InteractiveVarType::FilePicker));

        let vars2 = collect_interactive_vars("Select: {file_picker:Doc|Text files (*.txt)}");
        let file2 = vars2.iter().find(|v| v.tag == "{file_picker:Doc|Text files (*.txt)}");
        assert!(file2.is_some());
        assert_eq!(file2.unwrap().label, "Doc");
        assert_eq!(file2.unwrap().options, vec!["Text files (*.txt)"]);
    }

    #[test]
    fn test_process_with_user_vars_checkbox_datepicker_filepicker() {
        let mut user_vars = std::collections::HashMap::new();
        user_vars.insert("{checkbox:Include?|-SkipNoCache}".to_string(), "-SkipNoCache".to_string());
        user_vars.insert("{date_picker:When}".to_string(), "20260228".to_string());
        user_vars.insert("{file_picker:Path}".to_string(), "C:\\data\\file.txt".to_string());
        let out = process_with_user_vars(
            "Flags: {checkbox:Include?|-SkipNoCache} Date: {date_picker:When} File: {file_picker:Path}",
            None,
            &[],
            Some(&user_vars),
        );
        assert_eq!(out, "Flags: -SkipNoCache Date: 20260228 File: C:\\data\\file.txt");
    }

    #[test]
    fn test_am_pm_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{am/pm}", &config, None, &[]);
        assert!(out == "AM" || out == "PM");
    }

    #[test]
    fn test_tz_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{tz}", &config, None, &[]);
        assert!(!out.is_empty());
        assert!(out.len() <= 5);
    }

    #[test]
    fn test_timezone_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config("{timezone}", &config, None, &[]);
        assert!(!out.is_empty());
    }

    #[test]
    fn test_collect_interactive_vars() {
        let vars = collect_interactive_vars("Hello {var:Env} and {var:Extra Flags} world");
        assert_eq!(vars.len(), 2);
        assert!(vars.iter().any(|v| v.label == "Env" && v.tag == "{var:Env}"));
        assert!(vars
            .iter()
            .any(|v| v.label == "Extra Flags" && v.tag == "{var:Extra Flags}"));
    }

    #[test]
    fn test_process_with_user_vars() {
        let mut user_vars = std::collections::HashMap::new();
        user_vars.insert("{var:Env}".to_string(), "dev".to_string());
        user_vars.insert("{var:Extra Flags}".to_string(), "-SkipNoCache -SkipClip".to_string());
        let out = process_with_user_vars(
            ".\\start_docker.ps1 -Environment {var:Env} -Build {var:Extra Flags}",
            None,
            &[],
            Some(&user_vars),
        );
        assert_eq!(
            out,
            ".\\start_docker.ps1 -Environment dev -Build -SkipNoCache -SkipClip"
        );
    }

    #[test]
    #[ignore] // Requires network
    fn test_http_placeholder() {
        let config = TemplateConfig::default();
        let out = process_with_config(
            "IP: {http:https://api.ipify.org?format=json|ip}",
            &config,
            None,
            &[],
        );
        assert!(out.starts_with("IP: "));
        assert!(!out.contains("[HTTP"));
    }
}
