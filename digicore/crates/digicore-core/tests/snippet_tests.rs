//! Unit, integration, edge, and negative tests for Snippet entity.
//! Covers Snippet Editor UI (add/edit/delete) and Ghost Follower/Suggestor usage.

use digicore_core::domain::entities::Snippet;

#[test]
fn test_snippet_new() {
    let s = Snippet::new("hi", "Hello World");
    assert_eq!(s.trigger, "hi");
    assert_eq!(s.content, "Hello World");
    assert!(s.options.is_empty());
    assert!(s.category.is_empty());
    assert_eq!(s.profile, "Default");
    assert!(s.app_lock.is_empty());
    assert_eq!(s.pinned, "false");
    assert!(s.last_modified.is_empty());
}

#[test]
fn test_snippet_new_with_strings() {
    let s = Snippet::new(String::from("sig"), String::from("Best regards"));
    assert_eq!(s.trigger, "sig");
    assert_eq!(s.content, "Best regards");
}

#[test]
fn test_is_pinned_true() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "true".to_string();
    assert!(s.is_pinned());
}

#[test]
fn test_is_pinned_true_uppercase() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "TRUE".to_string();
    assert!(s.is_pinned());
}

#[test]
fn test_is_pinned_true_mixed_case() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "True".to_string();
    assert!(s.is_pinned());
}

#[test]
fn test_is_pinned_false() {
    let s = Snippet::new("x", "content");
    assert!(!s.is_pinned());
}

#[test]
fn test_is_pinned_false_explicit() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "false".to_string();
    assert!(!s.is_pinned());
}

#[test]
fn test_is_pinned_empty() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "".to_string();
    assert!(!s.is_pinned());
}

#[test]
fn test_is_pinned_invalid_value() {
    let mut s = Snippet::new("x", "content");
    s.pinned = "yes".to_string();
    assert!(!s.is_pinned());
}

#[test]
fn test_snippet_full_fields_roundtrip() {
    let s = Snippet {
        trigger: "addr".into(),
        content: "123 Main St".into(),
        options: "*:".into(),
        category: "General".into(),
        profile: "Work".into(),
        app_lock: "notepad.exe,word.exe".into(),
        pinned: "true".into(),
        last_modified: "20260101120000000".into(),
    };
    assert!(s.is_pinned());
    assert_eq!(s.app_lock, "notepad.exe,word.exe");
}

#[test]
fn test_snippet_clone() {
    let s = Snippet::new("t", "c");
    let c = s.clone();
    assert_eq!(s.trigger, c.trigger);
    assert_eq!(s.content, c.content);
}
