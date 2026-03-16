//! Integration tests for Tauri commands (load_library, save_library, get_app_state).
//! Tests the underlying AppState logic without the full Tauri runtime.

use digicore_core::domain::Snippet;
use digicore_text_expander::application::app_state::AppState;
use std::fs;
use tempfile::NamedTempFile;

fn create_test_library() -> (NamedTempFile, String) {
    let json = r#"{"Categories":{"Test":[{"trigger":"hi","content":"Hello World","options":"*:","category":"Test","profile":"Default","appLock":"","pinned":"false","lastModified":""},{"trigger":"bye","content":"Goodbye","options":"*:","category":"Test","profile":"Default","appLock":"","pinned":"false","lastModified":""}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    (tmp, path)
}

#[test]
fn test_load_library_via_app_state() {
    let (_tmp, path) = create_test_library();
    let mut state = AppState::new();
    state.library_path = path;
    let result = state.try_load_library();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);
    assert_eq!(state.library.len(), 1);
    assert!(state.library.contains_key("Test"));
    assert_eq!(state.library["Test"].len(), 2);
}

#[test]
fn test_save_library_via_app_state() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    let mut state = AppState::new();
    state.library_path = path.clone();
    state
        .library
        .insert("Cat1".to_string(), vec![Snippet::new("t1", "content1")]);
    state.categories = vec!["Cat1".to_string()];
    let result = state.try_save_library();
    assert!(result.is_ok());
    let content = fs::read_to_string(tmp.path()).unwrap();
    assert!(content.contains("\"trigger\": \"t1\""));
    assert!(content.contains("\"content\": \"content1\""));
}

#[test]
fn test_save_empty_path_fails() {
    let mut state = AppState::new();
    state.library_path = String::new();
    let result = state.try_save_library();
    assert!(result.is_err());
}

#[test]
fn test_load_nonexistent_fails() {
    let mut state = AppState::new();
    state.library_path = "/nonexistent/path.json".to_string();
    let result = state.try_load_library();
    assert!(result.is_err());
}
