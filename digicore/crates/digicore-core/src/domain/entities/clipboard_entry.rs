//! Clipboard Entry entity.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

/// A clipboard history entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ClipEntry {
    #[serde(default)]
    pub id: i64,
    pub content: String,
    #[serde(rename = "html_content")]
    pub html_content: Option<String>,
    #[serde(rename = "rtf_content")]
    pub rtf_content: Option<String>,
    pub process_name: String,
    pub window_title: String,
    pub length: usize,
    pub word_count: usize,
    #[serde(rename = "created_at")]
    #[cfg_attr(feature = "specta", specta(type = String))]
    pub timestamp: DateTime<Local>,
    #[serde(default = "default_entry_type")]
    pub entry_type: String,
    pub mime_type: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub image_width: Option<i32>,
    pub image_height: Option<i32>,
    pub image_bytes: Option<i64>,
    pub parent_id: Option<i64>,
    pub metadata: Option<String>,
    pub file_list: Option<Vec<String>>,
}

fn default_entry_type() -> String {
    "text".to_string()
}

impl ClipEntry {
    pub fn new(
        content: String,
        html_content: Option<String>,
        rtf_content: Option<String>,
        process_name: String,
        window_title: String,
    ) -> Self {
        let length = content.len();
        let word_count = content.split_whitespace().count();
        Self {
            id: 0,
            content,
            html_content,
            rtf_content,
            process_name,
            window_title,
            length,
            word_count,
            timestamp: Local::now(),
            entry_type: "text".to_string(),
            mime_type: None,
            image_path: None,
            thumb_path: None,
            image_width: None,
            image_height: None,
            image_bytes: None,
            parent_id: None,
            file_list: None,
            metadata: None,
        }
    }
}
