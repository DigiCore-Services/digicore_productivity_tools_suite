//! Snippet entity - represents a single text expansion entry.

use serde::{Deserialize, Serialize};

/// Trigger types for snippets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum TriggerType {
    Suffix,
    Regex,
}

impl Default for TriggerType {
    fn default() -> Self {
        Self::Suffix
    }
}

/// A text expansion snippet.
///
/// Matches the JSON format from text_expansion_library.json:
/// trigger, content, options, category, profile, appLock, pinned, lastModified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Snippet {
    pub trigger: String,
    #[serde(default)]
    pub trigger_type: TriggerType,
    pub content: String,
    #[serde(default, rename = "htmlContent")]
    pub html_content: Option<String>,
    #[serde(default, rename = "rtfContent")]
    pub rtf_content: Option<String>,
    #[serde(default)]
    pub options: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default, rename = "appLock")]
    pub app_lock: String,
    #[serde(default)]
    pub pinned: String,
    #[serde(default, rename = "lastModified")]
    pub last_modified: String,
}

impl Snippet {
    /// Create a new snippet with required fields.
    pub fn new(trigger: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            trigger_type: TriggerType::Suffix,
            content: content.into(),
            html_content: None,
            rtf_content: None,
            options: String::new(),
            category: String::new(),
            profile: "Default".into(),
            app_lock: String::new(),
            pinned: "false".into(),
            last_modified: String::new(),
        }
    }

    /// Whether this snippet is pinned (priority in search).
    pub fn is_pinned(&self) -> bool {
        self.pinned.eq_ignore_ascii_case("true")
    }
}
