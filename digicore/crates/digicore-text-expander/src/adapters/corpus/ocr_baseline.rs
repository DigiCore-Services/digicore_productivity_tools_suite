use digicore_core::domain::ports::{CorpusBaselinePort, TextExtractionPort};
use digicore_core::domain::{ExtractionSource, ExtractionMimeType};
use crate::adapters::extraction::WindowsNativeOcrAdapter;
use std::fs;
use std::path::PathBuf;

pub struct OcrBaselineAdapter {
    snapshot_dir: String,
    ocr_adapter: WindowsNativeOcrAdapter,
}

impl OcrBaselineAdapter {
    pub fn new(snapshot_dir: String, config: Option<crate::adapters::extraction::RuntimeConfig>) -> Self {
        Self {
            snapshot_dir,
            ocr_adapter: WindowsNativeOcrAdapter::new(config),
        }
    }
}

#[async_trait::async_trait]
impl CorpusBaselinePort for OcrBaselineAdapter {
    async fn generate_baseline(&self, image_path: &PathBuf, snapshot_name: &str) -> anyhow::Result<String> {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let target_dir = PathBuf::from(manifest_dir).join(&self.snapshot_dir);

        if !target_dir.exists() {
            fs::create_dir_all(&target_dir)?;
        }

        let ext = image_path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
        let mime_type = match ext.as_str() {
            "png" => ExtractionMimeType::Png,
            _ => ExtractionMimeType::Jpeg,
        };

        // Extract using OCR
        let source = ExtractionSource::File(image_path.clone());
        let result = self.ocr_adapter.extract(source, mime_type).await.map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let actual_text = result.text.trim();

        // Write as an insta .snap file
        // Insta snapshot format:
        // ---
        // source: ...
        // expression: ...
        // ---
        // content
        let snap_filename = format!("ocr_regression_tests__{}.snap", snapshot_name);
        let snap_path = target_dir.join(snap_filename);
        
        let snap_content = format!(
            "---\nsource: crates/digicore-text-expander/tests/ocr_regression_tests.rs\nexpression: actual_text\n---\n{}",
            actual_text
        );

        fs::write(&snap_path, snap_content)?;

        // Write the structured baseline JSON
        let baseline_json = serde_json::json!({
            "source_image": format!("{}.{}", snapshot_name, ext),
            "expected_text": actual_text,
            "metadata": {
                "generated_by": "Corpus Generation Utility",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        });
        
        let json_filename = format!("{}_baseline.json", snapshot_name);
        let json_path = target_dir.join(json_filename);
        fs::write(&json_path, serde_json::to_string_pretty(&baseline_json)?)?;

        Ok(actual_text.to_string())
    }
}
