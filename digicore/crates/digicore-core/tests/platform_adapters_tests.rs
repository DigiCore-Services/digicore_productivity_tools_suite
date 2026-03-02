//! Integration tests for platform adapters (mock implementations).
//! Unit, integration, edge, and negative tests.

use digicore_core::adapters::platform::mock::{MockClipboardAdapter, MockInputAdapter, MockWindowAdapter};
use digicore_core::domain::ports::{ClipboardPort, InputPort, Key, WindowContextPort};

#[test]
fn test_mock_input_type_text() {
    let adapter = MockInputAdapter::new();
    adapter.type_text("hello").unwrap();
    adapter.type_text("world").unwrap();
    let typed = adapter.typed_text();
    assert_eq!(typed, vec!["hello", "world"]);
}

#[test]
fn test_mock_input_key_sequence() {
    let adapter = MockInputAdapter::new();
    adapter
        .key_sequence(&[Key::Char('x'), Key::Tab, Key::Enter])
        .unwrap();
    let keys = adapter.keys_pressed.lock().unwrap();
    assert_eq!(keys.len(), 3);
}

#[test]
fn test_mock_clipboard_get_set() {
    let adapter = MockClipboardAdapter::new();
    adapter.set_text("clip content").unwrap();
    assert_eq!(adapter.get_text().unwrap(), "clip content");
}

#[test]
fn test_mock_clipboard_empty() {
    let adapter = MockClipboardAdapter::new();
    assert!(adapter.get_text().unwrap().is_empty());
    assert!(!adapter.has_text().unwrap());
}

#[test]
fn test_mock_clipboard_with_content() {
    let adapter = MockClipboardAdapter::with_content("pre-filled");
    assert_eq!(adapter.get_text().unwrap(), "pre-filled");
}

#[test]
fn test_mock_window_get_active() {
    let adapter = MockWindowAdapter::with_context("notepad.exe", "Untitled - Notepad");
    let ctx = adapter.get_active().unwrap();
    assert_eq!(ctx.process_name, "notepad.exe");
    assert_eq!(ctx.title, "Untitled - Notepad");
}

#[test]
fn test_mock_window_set_context() {
    let adapter = MockWindowAdapter::new();
    adapter.set_context("cursor.exe", "main.rs - digicore");
    let ctx = adapter.get_active().unwrap();
    assert_eq!(ctx.process_name, "cursor.exe");
    assert_eq!(ctx.title, "main.rs - digicore");
}

#[test]
fn test_mock_window_default_empty() {
    let adapter = MockWindowAdapter::new();
    let ctx = adapter.get_active().unwrap();
    assert!(ctx.process_name.is_empty());
    assert!(ctx.title.is_empty());
}
