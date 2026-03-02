//! Clipboard resolver (SE-12): Pre-resolve {clipboard} in JS expressions.
//!
//! Enables clipClean("{clipboard}") to receive actual clipboard content.

/// Escape a string for use inside a JS double-quoted string literal.
pub fn escape_for_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Replace "{clipboard}" in JS code with the actual clipboard value as a JS string literal.
/// Only replaces the quoted form to avoid recursive replacement when clipboard contains
/// the literal text "{clipboard}" (e.g. when the template itself was copied).
pub fn resolve_clipboard_in_js(js_code: &str, clipboard: &str) -> String {
    let escaped = escape_for_js_string(clipboard);
    let replacement = format!("\"{}\"", escaped);
    js_code.replace("\"{clipboard}\"", &replacement)
}
