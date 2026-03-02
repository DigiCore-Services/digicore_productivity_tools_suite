//! SE-20: Integration test harness for template placeholders.
//!
//! End-to-end tests: template with {js:}, {http:}, {clipboard}, {uuid}, {random:N}, {run:}; assert output.

use digicore_text_expander::application::scripting::{
    get_scripting_config, set_scripting_config, BoaScriptEngine, MockHttpFetcher, ScriptingConfig,
    ScriptingRegistry, set_registry,
};
use digicore_text_expander::application::template_processor::{
    process_with_config, TemplateConfig,
};
use std::sync::Arc;

fn default_config() -> TemplateConfig {
    TemplateConfig::default()
}

#[test]
fn integration_js_and_clipboard() {
    let config = default_config();
    let template = "Clipboard=[{clipboard}] JS=10+20={js: 10 + 20}";
    let out = process_with_config(template, &config, Some("hello"), &[]);
    assert_eq!(out, "Clipboard=[hello] JS=10+20=30");
}

#[test]
fn integration_js_clipboard_clip_history() {
    let config = TemplateConfig {
        clip_max_depth: 5,
        ..default_config()
    };
    let history = vec!["first".to_string(), "second".to_string()];
    let template = "Clip:{clipboard} | Clip1:{clip:1} | Clip2:{clip:2} | JS:{js: 1 + 2}";
    let out = process_with_config(template, &config, Some("current"), &history);
    assert_eq!(out, "Clip:current | Clip1:first | Clip2:second | JS:3");
}

#[test]
fn integration_js_http_clipboard_with_mock() {
    let mock_http = MockHttpFetcher::with_ipify_default();
    let registry = ScriptingRegistry {
        engine: Arc::new(BoaScriptEngine::new()),
        http_fetcher: Arc::new(mock_http),
    };
    set_registry(registry);

    let config = default_config();
    let template = "Clipboard={clipboard} | IP={http:https://api.ipify.org?format=json|ip} | JS={js: 2 * 3}";
    let out = process_with_config(template, &config, Some("hello"), &[]);
    assert_eq!(out, "Clipboard=hello | IP=192.168.1.1 | JS=6");

    set_registry(ScriptingRegistry::default());
}

#[test]
fn integration_full_template() {
    let mock_http = MockHttpFetcher::with_ipify_default();
    let registry = ScriptingRegistry {
        engine: Arc::new(BoaScriptEngine::new()),
        http_fetcher: Arc::new(mock_http),
    };
    set_registry(registry);

    let config = default_config();
    let template = "Clipboard={clipboard} | IP={http:https://api.ipify.org?format=json|ip} | Sum={js: 10 + 20}";
    let out = process_with_config(template, &config, Some("test"), &[]);
    assert_eq!(out, "Clipboard=test | IP=192.168.1.1 | Sum=30");

    set_registry(ScriptingRegistry::default());
}

#[test]
fn integration_js_and_clipboard_resolved_in_js() {
    let config = default_config();
    let template = r#"{js: "{clipboard}".length} chars from clipboard"#;
    let out = process_with_config(template, &config, Some("hello"), &[]);
    assert_eq!(out, "5 chars from clipboard");
}

#[test]
fn integration_uuid_random() {
    let config = default_config();
    let template = "ID={uuid} Code={random:6}";
    let out = process_with_config(template, &config, None, &[]);
    let parts: Vec<&str> = out.split(' ').collect();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].starts_with("ID="));
    let uuid_part = &parts[0][3..];
    assert_eq!(uuid_part.len(), 36);
    assert!(uuid_part.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    assert!(parts[1].starts_with("Code="));
    let random_part = &parts[1][5..];
    assert_eq!(random_part.len(), 6);
    assert!(random_part.chars().all(|c| c.is_ascii_uppercase()));
}

#[test]
#[serial_test::serial]
#[cfg(target_os = "windows")]
fn integration_run_allowed() {
    let mut cfg = get_scripting_config();
    cfg.run.disabled = false;
    cfg.run.allowlist = "cmd".to_string();
    set_scripting_config(cfg);

    let config = default_config();
    let template = "{run:cmd /c echo OK}";
    let out = process_with_config(template, &config, None, &[]);
    assert!(out.contains("OK"), "expected output to contain OK, got {:?}", out);
    assert!(!out.contains("Error") && !out.contains("disabled") && !out.contains("blocked"));

    set_scripting_config(ScriptingConfig::default());
}
