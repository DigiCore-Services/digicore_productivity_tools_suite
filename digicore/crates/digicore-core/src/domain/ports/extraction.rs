use crate::domain::{ExtractionSource, ExtractionResult};
use crate::domain::ExtractionMimeType;

/// Port for text extraction services (OCR, File Parser, etc.).
///
/// This is the boundary interface for any strategy that takes raw data
/// and returns searchable text.
#[async_trait::async_trait]
pub trait TextExtractionPort: Send + Sync {
    /// Returns true if this adapter can handle the given MIME type.
    fn can_handle(&self, mime_type: ExtractionMimeType) -> bool;

    /// Extract text from the given source.
    async fn extract(&self, source: ExtractionSource, mime_type: ExtractionMimeType) -> Result<ExtractionResult, String>;
}
