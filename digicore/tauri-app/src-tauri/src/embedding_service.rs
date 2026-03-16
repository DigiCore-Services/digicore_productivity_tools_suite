use fastembed::{TextEmbedding, InitOptions, ImageEmbedding};
use std::sync::Mutex;
use anyhow::{Result, anyhow};
use std::path::Path;
use serde_json::Value;

static TEXT_MODEL: Mutex<Option<TextEmbedding>> = Mutex::new(None);
#[allow(dead_code)]
static IMAGE_MODEL: Mutex<Option<ImageEmbedding>> = Mutex::new(None);

/// Generates a text embedding, enriched with optional metadata.
pub fn generate_text_embedding(text: &str, metadata: Option<&Value>) -> Result<Vec<f32>> {
    let mut lock = TEXT_MODEL.lock().unwrap();
    if lock.is_none() {
        let model = TextEmbedding::try_new(
            InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(true)
        ).map_err(|e| anyhow!("Failed to init text model: {}", e))?;
        *lock = Some(model);
    }

    // Enrich with metadata for higher-value retrieval
    let mut enriched = text.to_string();
    if let Some(meta) = metadata {
        if let Some(title) = meta.get("window_title").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: Window: {}]", title));
        }
        if let Some(app) = meta.get("process_name").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: App: {}]", app));
        }
        if let Some(file) = meta.get("file_name").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: File: {}]", file));
        }
    }

    let model = lock.as_mut().unwrap();
    let embeddings = model.embed(vec![enriched], None)?;
    embeddings.first().cloned().ok_or_else(|| anyhow!("No text embedding generated"))
}

/// Generates an image embedding using the CLIP vision model.
#[allow(dead_code)]
pub fn generate_image_embedding(image_path: &Path) -> Result<Vec<f32>> {
    let mut lock = IMAGE_MODEL.lock().unwrap();
    if lock.is_none() {
        let model = fastembed::ImageEmbedding::try_new(
            fastembed::ImageInitOptions::new(fastembed::ImageEmbeddingModel::ClipVitB32)
                .with_show_download_progress(true)
        ).map_err(|e| anyhow!("Failed to init image model: {}", e))?;
        *lock = Some(model);
    }
    
    let path_str = image_path.to_str().ok_or_else(|| anyhow!("Invalid image path: {:?}", image_path))?;
    
    let model = lock.as_mut().unwrap();
    let embeddings = model.embed(vec![path_str], None)?;
    embeddings.first().cloned().ok_or_else(|| anyhow!("No image embedding generated"))
}


