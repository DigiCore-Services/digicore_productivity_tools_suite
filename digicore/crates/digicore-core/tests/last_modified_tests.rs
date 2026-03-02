//! Unit, integration, edge, and negative tests for LastModified value object.

use digicore_core::domain::value_objects::LastModified;

#[test]
fn test_parse_valid_17_digit() {
    let lm = LastModified::parse("20260228194933595");
    assert!(lm.is_some());
    assert_eq!(lm.unwrap().as_str(), "20260228194933595");
}

#[test]
fn test_parse_with_whitespace() {
    let lm = LastModified::parse("  20260228194933595  ");
    assert!(lm.is_some());
    assert_eq!(lm.unwrap().as_str(), "20260228194933595");
}

#[test]
fn test_parse_invalid_too_short() {
    assert!(LastModified::parse("2026022819493359").is_none());
}

#[test]
fn test_parse_invalid_too_long() {
    assert!(LastModified::parse("202602281949335951").is_none());
}

#[test]
fn test_parse_invalid_non_digit() {
    assert!(LastModified::parse("2026022819493359a").is_none());
    assert!(LastModified::parse("2026022819493359-").is_none());
}

#[test]
fn test_parse_empty() {
    assert!(LastModified::parse("").is_none());
}

#[test]
fn test_is_newer_than() {
    let older = LastModified::parse("20260101000000000").unwrap();
    let newer = LastModified::parse("20260102000000000").unwrap();
    assert!(newer.is_newer_than(&older));
    assert!(!older.is_newer_than(&newer));
}

#[test]
fn test_is_newer_than_same() {
    let a = LastModified::parse("20260101120000000").unwrap();
    let b = LastModified::parse("20260101120000000").unwrap();
    assert!(!a.is_newer_than(&b));
    assert!(!b.is_newer_than(&a));
}

#[test]
fn test_now_produces_17_chars() {
    let lm = LastModified::now();
    assert_eq!(lm.as_str().len(), 17);
    assert!(lm.as_str().chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_display() {
    let lm = LastModified::parse("20260228194933595").unwrap();
    assert_eq!(format!("{}", lm), "20260228194933595");
}
