use digicore_core::domain::ports::TextExtractionPort;
use digicore_core::domain::{ExtractionSource, ExtractionMimeType, ExtractionResult};
use std::fs;

/// Adapter for extracting text from plain files (.txt, .py, .md, etc.).
pub struct PlainFileExtractionAdapter;

#[async_trait::async_trait]
impl TextExtractionPort for PlainFileExtractionAdapter {
    fn can_handle(&self, mime_type: ExtractionMimeType) -> bool {
        match mime_type {
            ExtractionMimeType::Text | ExtractionMimeType::Markdown | ExtractionMimeType::Python => true,
            _ => false,
        }
    }

    async fn extract(&self, source: ExtractionSource, mime_type: ExtractionMimeType) -> Result<ExtractionResult, String> {
        if !self.can_handle(mime_type) {
            return Err(format!("PlainFileExtractionAdapter cannot handle {:?}", mime_type));
        }

        let text = match source {
            ExtractionSource::File(ref path) => {
                fs::read_to_string(path).map_err(|e| format!("Failed to read file {}: {}", path.display(), e))?
            }
            ExtractionSource::Buffer(ref bytes) => {
                String::from_utf8(bytes.to_vec()).map_err(|e| format!("Failed to decode buffer as UTF-8: {}", e))?
            }
        };

        Ok(ExtractionResult {
            text,
            confidence: 1.0, // Plain file reads are 100% accurate relative to the source
            metadata: serde_json::json!({
                "source_type": match source {
                    ExtractionSource::File(_) => "file",
                    ExtractionSource::Buffer(_) => "buffer",
                },
                "mime_type": format!("{:?}", mime_type),
            }),
            tables: None,
            entities: None,
            diagnostics: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_can_handle() {
        let adapter = PlainFileExtractionAdapter;
        assert!(adapter.can_handle(ExtractionMimeType::Text));
        assert!(adapter.can_handle(ExtractionMimeType::Markdown));
        assert!(adapter.can_handle(ExtractionMimeType::Python));
        assert!(!adapter.can_handle(ExtractionMimeType::Png));
    }

    #[tokio::test]
    async fn test_extract_from_buffer() {
        let adapter = PlainFileExtractionAdapter;
        let source = ExtractionSource::Buffer("hello world".as_bytes().to_vec());
        let result = adapter.extract(source, ExtractionMimeType::Text).await.unwrap();
        assert_eq!(result.text, "hello world");
        assert_eq!(result.confidence, 1.0);
    }

    #[tokio::test]
    async fn test_extract_from_file() {
        let adapter = PlainFileExtractionAdapter;
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "file content").unwrap();
        
        let source = ExtractionSource::File(file.path().to_path_buf());
        let result = adapter.extract(source, ExtractionMimeType::Text).await.unwrap();
        assert_eq!(result.text.trim(), "file content");
    }

    #[tokio::test]
    async fn test_invalid_mime_type() {
        let adapter = PlainFileExtractionAdapter;
        let source = ExtractionSource::Buffer(vec![]);
        let result = adapter.extract(source, ExtractionMimeType::Png).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_extract_empty_buffer() {
        let adapter = PlainFileExtractionAdapter;
        let source = ExtractionSource::Buffer(vec![]);
        let result = adapter.extract(source, ExtractionMimeType::Text).await.unwrap();
        assert_eq!(result.text, "");
    }

    #[tokio::test]
    async fn test_extract_unicode() {
        let adapter = PlainFileExtractionAdapter;
        let content = "🦀 DigiCore OCR 🚀";
        let source = ExtractionSource::Buffer(content.as_bytes().to_vec());
        let result = adapter.extract(source, ExtractionMimeType::Text).await.unwrap();
        assert_eq!(result.text, content);
    }

    #[tokio::test]
    async fn test_extract_missing_file() {
        let adapter = PlainFileExtractionAdapter;
        let source = ExtractionSource::File(std::path::PathBuf::from("non_existent_file.txt"));
        let result = adapter.extract(source, ExtractionMimeType::Text).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read file"));
    }
}
