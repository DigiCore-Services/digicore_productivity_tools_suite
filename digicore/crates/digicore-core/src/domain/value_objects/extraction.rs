use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The source of content to be processed by an extraction strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ExtractionSource {
    /// A raw buffer in memory (ideal for clipboard capture).
    Buffer(Vec<u8>),
    /// A local file path (for PDF or image uploads).
    File(PathBuf),
}

/// Supported MIME types for extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractionMimeType {
    Png,
    Jpeg,
    Pdf,
    Text,
    Markdown,
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBlock {
    pub y_top: f32,
    pub y_bottom: f32,
    pub column_centers: Vec<f32>,
    pub headers: Vec<Vec<String>>,
    pub body: Vec<Vec<String>>,
    pub footers: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEntity {
    pub entity_type: String, // e.g., "Email", "Date", "Currency"
    pub value: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// The result of a text extraction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// The actual text extracted.
    pub text: String,
    /// Confidence score (0.0 to 1.0) if provided by the engine.
    pub confidence: f32,
    /// Metadata such as page count, dimensions, or word counts.
    pub metadata: serde_json::Value,
    /// Extracted tables as a list of grids (rows of cells).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<Vec<TableBlock>>,
    /// Semantic entities detected in the text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Vec<SemanticEntity>>,
    /// Diagnostic hints for visualization (e.g. coerced characters, large gaps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<serde_json::Value>,
}
