//! LastModified - 17-char timestamp for merge-by-trigger.
//!
//! Format: YYYYMMDDHHMMSSmmm (e.g. 20260228194933595)

use std::fmt;

/// 17-character timestamp for snippet versioning (merge-by-trigger).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastModified(String);

impl LastModified {
    /// Parse from string. Returns None if invalid format.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.len() == 17 && s.chars().all(|c| c.is_ascii_digit()) {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }

    /// Create from current system time.
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let s = format!("{:017}", ms);
        Self(s.chars().take(17).collect::<String>())
    }

    /// Raw string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compare: self is newer than other.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.0 > other.0
    }
}

impl fmt::Display for LastModified {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for LastModified {
    fn from(s: String) -> Self {
        Self(s)
    }
}
