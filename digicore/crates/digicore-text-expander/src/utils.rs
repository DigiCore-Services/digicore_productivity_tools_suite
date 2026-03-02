//! Utility functions for display and formatting.

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
    use super::truncate_for_display;

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
