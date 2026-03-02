//! Placeholder parser (SE-13): Extracts balanced-tag and simple placeholder parsing.
//!
//! Single responsibility: parse `{...}` placeholders from template content.
//! Resolution (date, time, clipboard, etc.) remains in template_processor.

/// Find balanced tag: scan from `{tagPrefix`; count `{` and `}`; content ends when count returns to 0.
/// Returns (inner_content, full_byte_len) or None if not found/unbalanced.
pub fn find_balanced_tag<'a>(s: &'a str, tag_prefix: &str) -> Option<(&'a str, usize)> {
    if !s.starts_with('{') {
        return None;
    }
    let prefix = format!("{{{tag_prefix}");
    if !s.starts_with(&prefix) {
        return None;
    }
    let mut depth = 0;
    let mut i = 0;
    let bytes = s.as_bytes();
    let mut content_start = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '{' {
            depth += 1;
            if depth == 1 {
                content_start = i + prefix.len();
            }
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let inner = &s[content_start..i];
                let full_len = i + 1;
                return Some((inner.trim(), full_len));
            }
        }
        i += 1;
        while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
            i += 1;
        }
    }
    None
}

/// Parsed placeholder: either a script-type tag or a simple `{inner}` tag.
#[derive(Debug, Clone)]
pub enum ParsedPlaceholder<'a> {
    /// Script placeholder: {prefix:inner} e.g. {js:1+1}, {http:url}
    Script {
        prefix: &'static str,
        inner: &'a str,
        len: usize,
    },
    /// Simple placeholder: {inner} with no nested braces e.g. {date}, {clipboard}
    Simple { inner: &'a str, len: usize },
}

impl ParsedPlaceholder<'_> {
    /// Byte length of the full placeholder including braces.
    pub fn len(&self) -> usize {
        match self {
            ParsedPlaceholder::Script { len, .. } => *len,
            ParsedPlaceholder::Simple { len, .. } => *len,
        }
    }
}

/// Parse a placeholder at the start of `s` (must start with `{`).
/// Returns ParsedPlaceholder or None if not a known placeholder.
pub fn parse_placeholder_at<'a>(
    s: &'a str,
    script_prefixes: &[&'static str],
) -> Option<ParsedPlaceholder<'a>> {
    if !s.starts_with('{') {
        return None;
    }

    for prefix in script_prefixes {
        let tag_prefix = format!("{prefix}:");
        if let Some((inner, full_len)) = find_balanced_tag(s, &tag_prefix) {
            return Some(ParsedPlaceholder::Script {
                prefix,
                inner,
                len: full_len,
            });
        }
    }

    let end = s.find('}')?;
    let inner = s[1..end].trim();
    Some(ParsedPlaceholder::Simple {
        inner,
        len: end + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_balanced_tag_simple() {
        let s = "{js: 10 + 20}";
        let (inner, len) = find_balanced_tag(s, "js:").unwrap();
        assert_eq!(inner, "10 + 20");
        assert_eq!(len, 13);
    }

    #[test]
    fn test_find_balanced_tag_nested() {
        let s = r#"{js: "a" + (1 ? "b" : "c") }"#;
        let (inner, _) = find_balanced_tag(s, "js:").unwrap();
        assert!(inner.contains("a"));
    }

    #[test]
    fn test_find_balanced_tag_http() {
        let s = "{http:https://api.example.com}";
        let (inner, len) = find_balanced_tag(s, "http:").unwrap();
        assert_eq!(inner, "https://api.example.com");
        assert_eq!(len, s.len());
    }

    #[test]
    fn test_parse_placeholder_at_script() {
        let prefixes: &[&str] = &["js", "http"];
        let s = "{js: 1 + 2}";
        let p = parse_placeholder_at(s, prefixes).unwrap();
        match &p {
            ParsedPlaceholder::Script { prefix, inner, len } => {
                assert_eq!(*prefix, "js");
                assert_eq!(*inner, "1 + 2");
                assert_eq!(*len, 11);
            }
            _ => panic!("expected Script"),
        }
    }

    #[test]
    fn test_parse_placeholder_at_simple() {
        let prefixes: &[&str] = &["js", "http"];
        let s = "{date}";
        let p = parse_placeholder_at(s, prefixes).unwrap();
        match &p {
            ParsedPlaceholder::Simple { inner, len } => {
                assert_eq!(*inner, "date");
                assert_eq!(*len, 6);
            }
            _ => panic!("expected Simple"),
        }
    }
}
