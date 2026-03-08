use digicore_core::domain::ports::TextExtractionPort;
use digicore_core::domain::{ExtractionSource, ExtractionMimeType, ExtractionResult};
use crate::adapters::extraction::{PlainFileExtractionAdapter, WindowsNativeOcrAdapter};
use std::sync::Arc;

/// Orchestrator for multiple text extraction strategies.
pub struct ExtractionDispatcher {
    adapters: Vec<Arc<dyn TextExtractionPort>>,
}

impl ExtractionDispatcher {
    pub fn new() -> Self {
        Self {
            adapters: vec![
                Arc::new(PlainFileExtractionAdapter),
                Arc::new(WindowsNativeOcrAdapter::default()),
            ],
        }
    }

    /// Primary entry point for text extraction.
    /// Automatically selects the best adapter for the given MIME type.
    pub async fn extract(&self, source: ExtractionSource, mime_type: ExtractionMimeType) -> Result<ExtractionResult, String> {
        for adapter in &self.adapters {
            if adapter.can_handle(mime_type) {
                return adapter.extract(source, mime_type).await;
            }
        }
        Err(format!("No extraction adapter found for MIME type: {:?}", mime_type))
    }
}

/// Factory for creating the extraction service.
pub fn create_extraction_service() -> ExtractionDispatcher {
    ExtractionDispatcher::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dispatcher_routing_to_plain_file() {
        let dispatcher = ExtractionDispatcher::new();
        let source = ExtractionSource::Buffer("print('test')".as_bytes().to_vec());
        
        // Python should be handled by PlainFileExtractionAdapter
        let result = dispatcher.extract(source, ExtractionMimeType::Python).await.unwrap();
        assert_eq!(result.text, "print('test')");
        assert_eq!(result.metadata["source_type"], "buffer");
    }

    #[tokio::test]
    async fn test_dispatcher_unsupported_type() {
        let dispatcher = ExtractionDispatcher::new();
        let source = ExtractionSource::Buffer(vec![]);
        
        // PDF is currently not handled by any adapter
        let result = dispatcher.extract(source, ExtractionMimeType::Pdf).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No extraction adapter found"));
    }
}
