//! Unit, integration, edge, and negative tests for Ghost Suggestor (F43-F47).

use digicore_core::domain::entities::Snippet;
use digicore_text_expander::application::ghost_suggestor::{self, GhostSuggestorConfig};
use serial_test::serial;
use std::collections::HashMap;

fn make_library() -> HashMap<String, Vec<Snippet>> {
    let mut lib = HashMap::new();
    lib.insert(
        "Cat1".to_string(),
        vec![
            Snippet::new("hi", "Hello"),
            Snippet::new("hello", "Hello World"),
            Snippet::new("sig", "Best regards"),
        ],
    );
    let mut pinned = Snippet::new("addr", "123 Main St");
    pinned.pinned = "true".to_string();
    lib.insert("Cat2".to_string(), vec![pinned]);
    lib
}

#[test]
#[serial]
fn test_start_stop() {
    ghost_suggestor::stop();
    assert!(!ghost_suggestor::is_enabled());

    let config = GhostSuggestorConfig::default();
    ghost_suggestor::start(config, HashMap::new());
    assert!(ghost_suggestor::is_enabled());

    ghost_suggestor::stop();
    assert!(!ghost_suggestor::is_enabled());
}

#[test]
#[serial]
fn test_prefix_match_suggestions() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("hi", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(!suggestions.is_empty(), "Should have suggestions for 'hi'");
    let triggers: Vec<_> = suggestions.iter().map(|s| s.snippet.trigger.as_str()).collect();
    assert!(triggers.contains(&"hi"), "trigger 'hi' matches buffer 'hi'");
    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_prefix_match_partial_trigger() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("he", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(!suggestions.is_empty());
    let triggers: Vec<_> = suggestions.iter().map(|s| s.snippet.trigger.as_str()).collect();
    assert!(triggers.contains(&"hello"), "buffer 'he' should match trigger 'hello'");

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_prefix_match_case_insensitive() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("HI", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.snippet.trigger == "hi"));

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_empty_buffer_no_suggestions() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("", "notepad.exe");
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(suggestions.is_empty());

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_no_match_suggestions() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("xyz", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(suggestions.is_empty());

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_update_library_refreshes_suggestions() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    let mut lib = HashMap::new();
    lib.insert("Cat".to_string(), vec![Snippet::new("old", "Old content")]);
    ghost_suggestor::start(config, lib);

    ghost_suggestor::on_buffer_changed("old", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].snippet.content, "Old content");

    let mut new_lib = HashMap::new();
    new_lib.insert("Cat".to_string(), vec![Snippet::new("old", "Updated content")]);
    ghost_suggestor::update_library(new_lib);
    ghost_suggestor::on_buffer_changed("old", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let suggestions = ghost_suggestor::get_suggestions();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].snippet.content, "Updated content");

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_cycle_selection_forward() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: true,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("h", "notepad.exe");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let _ = ghost_suggestor::tick_debounce();
    let idx = ghost_suggestor::cycle_selection_forward();
    assert!(idx <= ghost_suggestor::get_suggestions().len());

    ghost_suggestor::stop();
}

#[test]
#[serial]
fn test_disabled_returns_no_suggestions() {
    ghost_suggestor::stop();
    let config = GhostSuggestorConfig {
        enabled: false,
        debounce_ms: 0,
        offset_x: 0,
        offset_y: 20,
    };
    ghost_suggestor::start(config, make_library());

    ghost_suggestor::on_buffer_changed("hi", "notepad.exe");
    let suggestions = ghost_suggestor::get_suggestions();
    assert!(suggestions.is_empty());

    ghost_suggestor::stop();
}
