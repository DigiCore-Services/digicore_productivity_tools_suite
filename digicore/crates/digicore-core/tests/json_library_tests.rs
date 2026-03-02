//! Unit, integration, edge, and negative tests for JsonLibraryAdapter.

use digicore_core::adapters::persistence::JsonLibraryAdapter;
use digicore_core::domain::entities::Snippet;
use digicore_core::domain::ports::SnippetRepository;
use std::collections::HashMap;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_load_valid_json() {
    let json = r#"{"Categories":{"Test":[{"trigger":"hi","content":"Hello","options":"*:","category":"","profile":"Default","appLock":"","pinned":"false","lastModified":"20260101120000000"}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let repo = JsonLibraryAdapter;
    let lib = repo.load(tmp.path()).unwrap();
    assert_eq!(lib.len(), 1);
    assert!(lib.contains_key("Test"));
    assert_eq!(lib["Test"].len(), 1);
    assert_eq!(lib["Test"][0].trigger, "hi");
    assert_eq!(lib["Test"][0].content, "Hello");
}

#[test]
fn test_load_save_roundtrip() {
    let mut library = HashMap::new();
    library.insert(
        "Category1".to_string(),
        vec![Snippet::new("t1", "content1"), Snippet::new("t2", "content2")],
    );

    let tmp = NamedTempFile::new().unwrap();
    let repo = JsonLibraryAdapter;
    repo.save(tmp.path(), &library).unwrap();
    let loaded = repo.load(tmp.path()).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded["Category1"].len(), 2);
}

#[test]
fn test_load_nonexistent_file() {
    let repo = JsonLibraryAdapter;
    let result = repo.load(std::path::Path::new("/nonexistent/path.json"));
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_json() {
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), "not valid json {").unwrap();

    let repo = JsonLibraryAdapter;
    let result = repo.load(tmp.path());
    assert!(result.is_err());
}

#[test]
fn test_load_empty_categories() {
    let json = r#"{"Categories":{}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let repo = JsonLibraryAdapter;
    let lib = repo.load(tmp.path()).unwrap();
    assert!(lib.is_empty());
}

#[test]
fn test_merge_keeps_newer() {
    let mut existing = HashMap::new();
    existing.insert(
        "Cat".to_string(),
        vec![
            Snippet {
                trigger: "a".into(),
                content: "old".into(),
                options: "".into(),
                category: "".into(),
                profile: "".into(),
                app_lock: "".into(),
                pinned: "false".into(),
                last_modified: "20260101000000000".into(),
            },
        ],
    );

    let mut incoming = HashMap::new();
    incoming.insert(
        "Cat".to_string(),
        vec![
            Snippet {
                trigger: "a".into(),
                content: "new".into(),
                options: "".into(),
                category: "".into(),
                profile: "".into(),
                app_lock: "".into(),
                pinned: "false".into(),
                last_modified: "20260102000000000".into(),
            },
        ],
    );

    let repo = JsonLibraryAdapter;
    repo.merge(&mut existing, incoming);
    assert_eq!(existing["Cat"][0].content, "new");
}

#[test]
fn test_save_creates_backup_when_file_exists() {
    let json = r#"{"Categories":{"Test":[{"trigger":"hi","content":"Hello","options":"*:","category":"","profile":"Default","appLock":"","pinned":"false","lastModified":"20260101120000000"}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let mut library = HashMap::new();
    library.insert(
        "Updated".to_string(),
        vec![Snippet::new("t1", "new content")],
    );

    let repo = JsonLibraryAdapter;
    let path = tmp.path();
    repo.save(path, &library).unwrap();

    let backup_path = path.with_extension(
        path.extension()
            .map_or("last".to_string(), |e| format!("{}.last", e.to_string_lossy())),
    );
    assert!(backup_path.exists(), "Backup .last file should exist at {:?}", backup_path);
    let backup_content = fs::read_to_string(&backup_path).unwrap();
    assert!(backup_content.contains("Hello"));
}

#[test]
fn test_save_atomic_no_tmp_leftover() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let mut library = HashMap::new();
    library.insert("Cat".to_string(), vec![Snippet::new("x", "content")]);

    let repo = JsonLibraryAdapter;
    repo.save(&path, &library).unwrap();

    let tmp_path = path.with_extension(
        path.extension()
            .map_or("tmp".to_string(), |e| format!("{}.tmp", e.to_string_lossy())),
    );
    assert!(!tmp_path.exists(), "Temp file should be removed after save");
    let loaded = repo.load(&path).unwrap();
    assert_eq!(loaded["Cat"][0].content, "content");
}

#[test]
fn test_save_no_backup_when_file_nonexistent() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path();
    let _ = fs::remove_file(path);

    let mut library = HashMap::new();
    library.insert("New".to_string(), vec![Snippet::new("n", "new")]);

    let repo = JsonLibraryAdapter;
    repo.save(path, &library).unwrap();

    let backup_path = path.with_extension(
        path.extension()
            .map_or("last".to_string(), |e| format!("{}.last", e.to_string_lossy())),
    );
    assert!(!backup_path.exists(), "No backup when file did not exist");
}

#[test]
fn test_load_with_utf8_bom() {
    let json = r#"{"Categories":{"Test":[{"trigger":"hi","content":"Hello","options":"*:","category":"","profile":"Default","appLock":"","pinned":"false","lastModified":"20260101120000000"}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), format!("\u{FEFF}{}", json)).unwrap();

    let repo = JsonLibraryAdapter;
    let lib = repo.load(tmp.path()).unwrap();
    assert_eq!(lib["Test"][0].content, "Hello");
}

#[test]
fn test_merge_adds_new_category() {
    let mut existing = HashMap::new();
    existing.insert("Cat1".to_string(), vec![Snippet::new("a", "content")]);

    let mut incoming = HashMap::new();
    incoming.insert("Cat2".to_string(), vec![Snippet::new("b", "new")]);

    let repo = JsonLibraryAdapter;
    repo.merge(&mut existing, incoming);
    assert_eq!(existing.len(), 2);
    assert_eq!(existing["Cat2"][0].trigger, "b");
}

#[test]
fn test_merge_adds_new_snippet_same_category() {
    let mut existing = HashMap::new();
    existing.insert(
        "Cat".to_string(),
        vec![Snippet {
            trigger: "a".into(),
            content: "a".into(),
            options: "".into(),
            category: "".into(),
            profile: "".into(),
            app_lock: "".into(),
            pinned: "false".into(),
            last_modified: "20260101000000000".into(),
        }],
    );

    let mut incoming = HashMap::new();
    incoming.insert(
        "Cat".to_string(),
        vec![Snippet {
            trigger: "b".into(),
            content: "b".into(),
            options: "".into(),
            category: "".into(),
            profile: "".into(),
            app_lock: "".into(),
            pinned: "false".into(),
            last_modified: "20260102000000000".into(),
        }],
    );

    let repo = JsonLibraryAdapter;
    repo.merge(&mut existing, incoming);
    assert_eq!(existing["Cat"].len(), 2);
}

#[test]
fn test_merge_empty_incoming_preserves_existing() {
    let mut existing = HashMap::new();
    existing.insert("Cat".to_string(), vec![Snippet::new("a", "content")]);

    let incoming = HashMap::new();
    let repo = JsonLibraryAdapter;
    repo.merge(&mut existing, incoming);
    assert_eq!(existing["Cat"][0].content, "content");
}

#[test]
fn test_load_full_snippet_editor_format() {
    let json = r#"{"Categories":{"General":[{"trigger":"addr","content":"123 Main St","options":"*:","category":"General","profile":"Work","appLock":"notepad.exe","pinned":"true","lastModified":"20260101120000000"}]}}"#;
    let tmp = NamedTempFile::new().unwrap();
    fs::write(tmp.path(), json).unwrap();

    let repo = JsonLibraryAdapter;
    let lib = repo.load(tmp.path()).unwrap();
    assert_eq!(lib["General"][0].trigger, "addr");
    assert_eq!(lib["General"][0].content, "123 Main St");
    assert_eq!(lib["General"][0].options, "*:");
    assert_eq!(lib["General"][0].category, "General");
    assert_eq!(lib["General"][0].profile, "Work");
    assert_eq!(lib["General"][0].app_lock, "notepad.exe");
    assert_eq!(lib["General"][0].pinned, "true");
    assert_eq!(lib["General"][0].last_modified, "20260101120000000");
}

#[test]
fn test_save_load_snippet_with_app_lock_comma_separated() {
    let mut snip = Snippet::new("sig", "Best regards");
    snip.app_lock = "notepad.exe,word.exe".to_string();
    snip.pinned = "true".to_string();
    snip.last_modified = "20260228120000000".to_string();

    let mut library = HashMap::new();
    library.insert("Cat".to_string(), vec![snip]);

    let tmp = NamedTempFile::new().unwrap();
    let repo = JsonLibraryAdapter;
    repo.save(tmp.path(), &library).unwrap();
    let loaded = repo.load(tmp.path()).unwrap();
    assert_eq!(loaded["Cat"][0].app_lock, "notepad.exe,word.exe");
    assert!(loaded["Cat"][0].is_pinned());
}

#[test]
fn test_merge_keeps_older_when_incoming_older() {
    let mut existing = HashMap::new();
    existing.insert(
        "Cat".to_string(),
        vec![
            Snippet {
                trigger: "a".into(),
                content: "newer".into(),
                options: "".into(),
                category: "".into(),
                profile: "".into(),
                app_lock: "".into(),
                pinned: "false".into(),
                last_modified: "20260102000000000".into(),
            },
        ],
    );

    let mut incoming = HashMap::new();
    incoming.insert(
        "Cat".to_string(),
        vec![
            Snippet {
                trigger: "a".into(),
                content: "older".into(),
                options: "".into(),
                category: "".into(),
                profile: "".into(),
                app_lock: "".into(),
                pinned: "false".into(),
                last_modified: "20260101000000000".into(),
            },
        ],
    );

    let repo = JsonLibraryAdapter;
    repo.merge(&mut existing, incoming);
    assert_eq!(existing["Cat"][0].content, "newer");
}
