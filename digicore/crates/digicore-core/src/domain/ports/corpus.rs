use std::path::PathBuf;

/// Port for persisting captured corpus images to disk (or other storage).
pub trait CorpusStoragePort: Send + Sync {
    /// Save the raw image bytes to a uniquely named file with the given extension.
    /// Returns the absolute path to the saved file.
    fn save_image(&self, data: &[u8], filename_prefix: &str, extension: &str) -> anyhow::Result<PathBuf>;
}

/// Port for generating OCR baselines from a given image.
#[async_trait::async_trait]
pub trait CorpusBaselinePort: Send + Sync {
    /// Given an image file, generate its text extraction and save a baseline snapshot.
    /// Returns the text content that was generated.
    async fn generate_baseline(&self, image_path: &PathBuf, snapshot_name: &str) -> anyhow::Result<String>;
}
