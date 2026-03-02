//! Unit tests for expansion engine.

use digicore_core::adapters::platform::mock::{MockClipboardAdapter, MockInputAdapter, MockWindowAdapter};
use digicore_core::domain::entities::Snippet;
use digicore_text_expander::application::expansion_engine::ExpansionEngine;
use std::collections::HashMap;

#[test]
fn test_find_snippet_no_app_lock() {
    let input = MockInputAdapter::new();
    let clipboard = MockClipboardAdapter::new();
    let window = MockWindowAdapter::with_context("notepad.exe", "Test");

    let mut library = HashMap::new();
    library.insert(
        "Test".to_string(),
        vec![Snippet::new("hi", "Hello World")],
    );

    let mut engine = ExpansionEngine::new(input, clipboard, window);
    engine.load_library(library);

    let (snippet, cat) = engine.find_snippet("hi").unwrap();
    assert_eq!(snippet.trigger, "hi");
    assert_eq!(snippet.content, "Hello World");
    assert_eq!(cat, "Test");
}

#[test]
fn test_find_snippet_app_lock_allowed() {
    let input = MockInputAdapter::new();
    let clipboard = MockClipboardAdapter::new();
    let window = MockWindowAdapter::with_context("notepad.exe", "Test");

    let mut snip = Snippet::new("sig", "Best regards");
    snip.app_lock = "notepad.exe".to_string();

    let mut library = HashMap::new();
    library.insert("Cat".to_string(), vec![snip]);

    let mut engine = ExpansionEngine::new(input, clipboard, window);
    engine.load_library(library);

    assert!(engine.find_snippet("sig").is_some());
}

#[test]
fn test_find_snippet_app_lock_denied() {
    let input = MockInputAdapter::new();
    let clipboard = MockClipboardAdapter::new();
    let window = MockWindowAdapter::with_context("chrome.exe", "Browser");

    let mut snip = Snippet::new("sig", "Best regards");
    snip.app_lock = "notepad.exe".to_string();

    let mut library = HashMap::new();
    library.insert("Cat".to_string(), vec![snip]);

    let mut engine = ExpansionEngine::new(input, clipboard, window);
    engine.load_library(library);

    assert!(engine.find_snippet("sig").is_none());
}

#[test]
fn test_expand_trigger_types_content() {
    let input = MockInputAdapter::new();
    let clipboard = MockClipboardAdapter::new();
    let window = MockWindowAdapter::new();

    let mut library = HashMap::new();
    library.insert(
        "Cat".to_string(),
        vec![Snippet::new("x", "expanded content")],
    );

    let mut engine = ExpansionEngine::new(input, clipboard, window);
    engine.load_library(library);

    let result = engine.expand_trigger("x").unwrap();
    assert_eq!(result, Some("expanded content".to_string()));
}

#[test]
fn test_expand_trigger_not_found() {
    let input = MockInputAdapter::new();
    let clipboard = MockClipboardAdapter::new();
    let window = MockWindowAdapter::new();

    let engine = ExpansionEngine::new(input, clipboard, window);

    let result = engine.expand_trigger("nonexistent").unwrap();
    assert_eq!(result, None);
}
