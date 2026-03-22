//! Unit tests for AppState try_load_library and try_save_library.

use digicore_core::domain::Snippet;
use digicore_text_expander::application::app_state::AppState;
use std::collections::HashMap;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_try_load_library_success() {
    let json = r#"{"Categories":{"Test":[{"trigger":"hi","content":"Hello","options":"*:","category":"Test","profile":"Default","appLock":"","pinned":"false","lastModified":""}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let mut state = AppState::new();
    state.library_path = tmp.path().to_string_lossy().to_string();
    let result = state.try_load_library();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);
    assert_eq!(state.categories.len(), 1);
    assert!(state.library.contains_key("Test"));
    assert_eq!(state.library["Test"].len(), 1);
    assert_eq!(state.library["Test"][0].trigger, "hi");
}

#[test]
fn test_try_load_library_empty_path_returns_zero() {
    let mut state = AppState::new();
    state.library_path = String::new();
    let result = state.try_load_library();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_try_load_library_nonexistent_fails() {
    let mut state = AppState::new();
    state.library_path = "/nonexistent/path/library.json".to_string();
    let result = state.try_load_library();
    assert!(result.is_err());
}

#[test]
fn test_try_save_library_success() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_string_lossy().to_string();

    let mut state = AppState::new();
    state.library_path = path.clone();
    state.library.insert(
        "Cat1".to_string(),
        vec![Snippet::new("t1", "content1"), Snippet::new("t2", "content2")],
    );
    state.categories = vec!["Cat1".to_string()];

    let result = state.try_save_library();
    assert!(result.is_ok());
    assert!(tmp.path().exists());
    let json = fs::read_to_string(tmp.path()).unwrap();
    assert!(json.contains("\"trigger\": \"t1\""));
    assert!(json.contains("\"content\": \"content1\""));
}

#[test]
fn test_try_save_library_empty_path_fails() {
    let mut state = AppState::new();
    state.library_path = String::new();
    let result = state.try_save_library();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_try_save_library_empty_in_memory_loads_then_saves() {
    let json = r#"{"Categories":{"Existing":[{"trigger":"ex","content":"Existing content","options":"*:","category":"Existing","profile":"Default","appLock":"","pinned":"false","lastModified":""}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let mut state = AppState::new();
    state.library_path = tmp.path().to_string_lossy().to_string();
    state.library = HashMap::new();

    let result = state.try_save_library();
    assert!(result.is_ok());
    let loaded = fs::read_to_string(tmp.path()).unwrap();
    assert!(loaded.contains("Existing"));
    assert!(loaded.contains("Existing content"));
}

#[test]
fn test_add_snippet() {
    let mut state = AppState::new();
    state.library.insert("Cat1".to_string(), vec![Snippet::new("t1", "c1")]);
    state.categories = vec!["Cat1".to_string()];

    let mut snip = Snippet::new("t2", "c2");
    snip.options = "*:".into();
    snip.category = "Cat1".into();
    snip.profile = "Default".into();
    snip.pinned = "false".into();
    snip.last_modified = String::new();
    state.add_snippet("Cat1", &snip);
    assert_eq!(state.library["Cat1"].len(), 2);
    assert_eq!(state.library["Cat1"][1].trigger, "t2");
    assert!(!state.library["Cat1"][1].last_modified.is_empty());
}

#[test]
fn test_update_snippet() {
    let mut s1 = Snippet::new("t1", "c1");
    s1.category = "Cat1".into();
    let mut s2 = Snippet::new("t2", "c2");
    s2.category = "Cat1".into();
    let mut state = AppState::new();
    state.library.insert("Cat1".to_string(), vec![s1, s2]);
    state.categories = vec!["Cat1".to_string()];

    let mut snip = Snippet::new("t2-updated", "c2-updated");
    snip.options = "*:".into();
    snip.category = "Cat1".into();
    snip.profile = "Default".into();
    snip.pinned = "false".into();
    snip.last_modified = String::new();
    state.update_snippet("Cat1", 1, &snip).unwrap();
    assert_eq!(state.library["Cat1"].len(), 2);
    assert_eq!(state.library["Cat1"][1].trigger, "t2-updated");
    assert_eq!(state.library["Cat1"][1].content, "c2-updated");
}

#[test]
fn test_delete_snippet() {
    let mut state = AppState::new();
    state.library.insert(
        "Cat1".to_string(),
        vec![Snippet::new("t1", "c1"), Snippet::new("t2", "c2")],
    );
    state.categories = vec!["Cat1".to_string()];

    state.delete_snippet("Cat1", 0).unwrap();
    assert_eq!(state.library["Cat1"].len(), 1);
    assert_eq!(state.library["Cat1"][0].trigger, "t2");

    state.delete_snippet("Cat1", 0).unwrap();
    assert!(!state.library.contains_key("Cat1"));
}
