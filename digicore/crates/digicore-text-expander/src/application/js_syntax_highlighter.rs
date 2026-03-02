//! FE-2: Simple JS keyword highlighting for Script Library editor.
//! Uses LayoutJob to color keywords, strings, and comments.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

/// JS keywords to highlight (reserved + common).
const JS_KEYWORDS: &[&str] = &[
    "function", "return", "var", "let", "const", "if", "else", "for", "while",
    "do", "switch", "case", "break", "continue", "try", "catch", "finally",
    "throw", "new", "delete", "typeof", "instanceof", "in", "of", "this",
    "true", "false", "null", "undefined", "async", "await", "class", "extends",
    "import", "export", "default", "from", "static", "get", "set",
];

/// Build a LayoutJob with JS syntax highlighting.
pub fn highlight_js(text: &str, font_size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(font_size);

    let default_format = TextFormat::simple(font.clone(), Color32::LIGHT_GRAY);
    let keyword_format = TextFormat::simple(font.clone(), Color32::from_rgb(220, 100, 100));
    let string_format = TextFormat::simple(font.clone(), Color32::from_rgb(100, 180, 100));
    let comment_format = TextFormat::simple(font.clone(), Color32::from_rgb(120, 120, 120));

    let mut i = 0;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment_format.clone());
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2;
            }
            job.append(&text[start..i], 0.0, comment_format.clone());
            continue;
        }
        if bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'`' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            job.append(&text[start..i], 0.0, string_format.clone());
            continue;
        }
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' || bytes[i] == b'$' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            let word = &text[start..i];
            let is_keyword = JS_KEYWORDS.contains(&word);
            job.append(
                word,
                0.0,
                if is_keyword {
                    keyword_format.clone()
                } else {
                    default_format.clone()
                },
            );
            continue;
        }
        let ch = &text[i..i + 1];
        i += 1;
        job.append(ch, 0.0, default_format.clone());
    }

    job
}
