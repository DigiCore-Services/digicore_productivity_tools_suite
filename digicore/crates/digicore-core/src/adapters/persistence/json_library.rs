//! JsonLibraryAdapter - implements SnippetRepository for JSON library format.
//!
//! Format: { "Categories": { "CategoryName": [ { trigger, content, options, ... } ] } }
//! Compatible with legacy AHK text_expansion_library.json during migration.
//! Sanitizes unescaped control chars (newlines, tabs) from legacy AHK output when reading.
//! Rust writes standards-compliant JSON.
//!
//! F27: Library backup (.last) before overwrite.
//! F29: Atomic save (temp file + move).

use crate::domain::ports::SnippetRepository;
use crate::domain::Snippet;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Escape control characters inside JSON string values so serde_json can parse.
fn escape_control_chars_in_strings(json: &str) -> String {
    let mut result = String::with_capacity(json.len() + 64);
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = json.chars().peekable();

    while let Some(c) = chars.next() {
        if escaped {
            result.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' && in_string {
            result.push(c);
            escaped = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }
        if in_string && c.is_ascii() && (c as u8) < 0x20 {
            match c {
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                _ => result.push_str(&format!("\\u{:04x}", c as u32)),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// JSON library file format (matches AHK text_expansion_library.json).
#[derive(Debug, Serialize, Deserialize)]
struct LibraryFile {
    #[serde(rename = "Categories")]
    categories: HashMap<String, Vec<Snippet>>,
}

/// Adapter for reading/writing JSON snippet library.
#[derive(Debug, Default)]
pub struct JsonLibraryAdapter;

impl SnippetRepository for JsonLibraryAdapter {
    fn load(&self, path: &Path) -> Result<HashMap<String, Vec<Snippet>>> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read library: {}", path.display()))?;
        // Strip UTF-8 BOM if present (common on Windows)
        let contents = contents.strip_prefix('\u{FEFF}').unwrap_or(&contents);
        // Escape unescaped control chars from legacy AHK JSON (literal newlines/tabs)
        let contents = escape_control_chars_in_strings(contents);
        let lib: LibraryFile = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse JSON: {}", path.display()))?;
        Ok(lib.categories)
    }

    fn save(&self, path: &Path, library: &HashMap<String, Vec<Snippet>>) -> Result<()> {
        let lib = LibraryFile {
            categories: library.clone(),
        };
        let json = serde_json::to_string_pretty(&lib)
            .context("Failed to serialize library to JSON")?;

        // F27: Backup existing file to .last before overwrite
        if path.exists() {
            let backup_path = path.with_extension(path.extension().map_or("last".to_string(), |e| format!("{}.last", e.to_string_lossy())));
            fs::copy(path, &backup_path)
                .with_context(|| format!("Failed to backup library to: {}", backup_path.display()))?;
        }

        // F29: Atomic save - write to .tmp then rename
        let tmp_path = path.with_extension(path.extension().map_or("tmp".to_string(), |e| format!("{}.tmp", e.to_string_lossy())));
        fs::write(&tmp_path, &json)
            .with_context(|| format!("Failed to write library to temp: {}", tmp_path.display()))?;
        fs::rename(&tmp_path, path).or_else(|_| {
            // Fallback for cross-filesystem rename (e.g. Windows): copy + remove
            fs::copy(&tmp_path, path)?;
            let _ = fs::remove_file(&tmp_path);
            Ok::<(), std::io::Error>(())
        })
            .with_context(|| format!("Failed to move temp to library: {}", path.display()))?;
        Ok(())
    }
}
