//! Utility functions for display and formatting.

/// Parse file filter string "Name (*.ext1;*.ext2)" -> (name, extensions).
/// Returns None for invalid format. Extensions are without dots (e.g. "json").
pub fn parse_file_filter(s: &str) -> Option<(String, Vec<String>)> {
    let s = s.trim();
    let paren = s.find('(')?;
    let name = s[..paren].trim().to_string();
    let rest = s[paren..].trim_start_matches('(').trim_end_matches(')');
    let exts: Vec<String> = rest
        .split(';')
        .filter_map(|p| {
            let p = p.trim().trim_start_matches('*');
            if p.is_empty() || p == "." {
                Some("*".to_string())
            } else {
                Some(p.trim_start_matches('.').to_string())
            }
        })
        .collect();
    if exts.is_empty() {
        None
    } else {
        Some((name, exts))
    }
}

/// Run callback with filters built from "Name (*.ext)" string.
/// Use with FileDialogPort::pick_file.
pub fn with_file_filters<R>(
    filter_str: &str,
    f: impl FnOnce(&[(&str, &[&str])]) -> R,
) -> R {
    if filter_str == "All Files (*.*)" {
        return f(&[]);
    }
    let mut filter_storage: Vec<(String, Vec<String>)> = Vec::new();
    if let Some((name, exts)) = parse_file_filter(filter_str) {
        if !exts.is_empty() && exts[0] != "*" {
            filter_storage.push((name, exts));
        }
    }
    let ext_refs: Vec<Vec<&str>> = filter_storage
        .iter()
        .map(|(_, e)| e.iter().map(|s| s.as_str()).collect())
        .collect();
    let filters: Vec<(&str, &[&str])> = filter_storage
        .iter()
        .zip(ext_refs.iter())
        .map(|((n, _), r)| (n.as_str(), r.as_slice()))
        .collect();
    f(&filters)
}

/// Truncate a string for display, appending "..." if longer than max_len.
///
/// # Examples
///
/// ```
/// use digicore_text_expander::utils::truncate_for_display;
/// assert_eq!(truncate_for_display("hello", 5), "hello");
/// assert_eq!(truncate_for_display("hello world", 5), "he...");
/// ```
pub fn truncate_for_display(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_filter_json() {
        let (name, exts) = parse_file_filter("JSON (*.json)").unwrap();
        assert_eq!(name, "JSON");
        assert_eq!(exts, ["json"]);
    }

    #[test]
    fn test_parse_file_filter_multi() {
        let (name, exts) = parse_file_filter("Images (*.png;*.jpg;*.gif)").unwrap();
        assert_eq!(name, "Images");
        assert_eq!(exts, ["png", "jpg", "gif"]);
    }

    #[test]
    fn test_parse_file_filter_all_files() {
        let (name, exts) = parse_file_filter("All Files (*.*)").unwrap();
        assert_eq!(name, "All Files");
        assert_eq!(exts, ["*"]);
    }

    #[test]
    fn test_with_file_filters_all_files() {
        let result = with_file_filters("All Files (*.*)", |f: &[(&str, &[&str])]| {
            assert!(f.is_empty());
            "ok"
        });
        assert_eq!(result, "ok");
    }

    #[test]
    fn test_with_file_filters_json() {
        let result = with_file_filters("JSON (*.json)", |f: &[(&str, &[&str])]| {
            assert_eq!(f.len(), 1);
            assert_eq!(f[0].0, "JSON");
            assert_eq!(f[0].1, ["json"]);
            "ok"
        });
        assert_eq!(result, "ok");
    }

    #[test]
    fn test_truncate_for_display_short() {
        assert_eq!(truncate_for_display("hi", 20), "hi");
        assert_eq!(truncate_for_display("", 5), "");
        assert_eq!(truncate_for_display("exact", 5), "exact");
    }

    #[test]
    fn test_truncate_for_display_long() {
        assert_eq!(truncate_for_display("hello world", 5), "he...");
        assert_eq!(truncate_for_display("abcdefghijk", 10), "abcdefg...");
        assert_eq!(truncate_for_display("notepad.exe", 20), "notepad.exe");
    }

    #[test]
    fn test_truncate_for_display_edge() {
        assert_eq!(truncate_for_display("abc", 3), "abc");
        assert_eq!(truncate_for_display("abcd", 3), "...");
    }
}
