use crate::kms_embed_diagnostic_log::warn_emit;
use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, ImageEmbedding, ImageInitOptions, InitOptions, TextEmbedding};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

pub use crate::kms_embed_diagnostic_log::KMS_EMBED_LOG_TARGET;

/// Stable KMS default: same string as the `EmbeddingModel` `Debug` / variant name (not fastembed's ONNX `model_code`).
pub const DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID: &str = "BGESmallENV15";

/// Returns configured model id, or the default when empty/whitespace.
pub fn normalized_embedding_model_id(configured: &str) -> String {
    let t = configured.trim();
    if t.is_empty() {
        DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID.to_string()
    } else {
        t.to_string()
    }
}

static EMBEDDING_MODEL_BY_VARIANT_DEBUG: OnceLock<HashMap<String, EmbeddingModel>> = OnceLock::new();

/// fastembed 5.x `EmbeddingModel::from_str` matches ONNX `model_code` (e.g. Xenova/bge-small-en-v1.5), not Rust variant names.
fn embedding_model_by_variant_debug_name(key: &str) -> Option<EmbeddingModel> {
    let map = EMBEDDING_MODEL_BY_VARIANT_DEBUG.get_or_init(|| {
        TextEmbedding::list_supported_models()
            .into_iter()
            .map(|info| (format!("{:?}", info.model), info.model))
            .collect()
    });
    map.get(key)
        .cloned()
        .or_else(|| {
            map.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.clone())
        })
}

/// Resolves a KMS model id to fastembed: accepts ONNX `model_code` (fastembed `FromStr`) or variant `Debug` name (e.g. BGESmallENV15).
pub fn kms_text_embedding_model_from_id(configured: &str) -> Result<EmbeddingModel, String> {
    let key = normalized_embedding_model_id(configured);
    if let Ok(m) = EmbeddingModel::from_str(&key) {
        return Ok(m);
    }
    if let Some(m) = embedding_model_by_variant_debug_name(&key) {
        return Ok(m);
    }
    Err(format!(
        "Unknown KMS text embedding model id {key:?}. Use a variant name such as BGESmallENV15 or a fastembed model_code such as Xenova/bge-small-en-v1.5 (see TextEmbedding::list_supported_models)."
    ))
}

static TEXT_MODELS: OnceLock<Mutex<HashMap<String, TextEmbedding>>> = OnceLock::new();

fn text_models() -> &'static Mutex<HashMap<String, TextEmbedding>> {
    TEXT_MODELS.get_or_init(|| Mutex::new(HashMap::new()))
}
#[allow(dead_code)]
static IMAGE_MODEL: Mutex<Option<ImageEmbedding>> = Mutex::new(None);

fn ensure_text_model(key: &str, model_enum: EmbeddingModel) -> Result<()> {
    let mut map = text_models().lock().unwrap();
    if map.contains_key(key) {
        log::debug!(
            target: KMS_EMBED_LOG_TARGET,
            "[KMS][fastembed] using cached TextEmbedding for model_key={}",
            key
        );
        return Ok(());
    }
    log::info!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][fastembed] initializing TextEmbedding model_key={} enum={:?}",
        key,
        model_enum
    );
    let m = TextEmbedding::try_new(
        InitOptions::new(model_enum.clone()).with_show_download_progress(true),
    )
    .map_err(|e| {
        log::warn!(
            target: KMS_EMBED_LOG_TARGET,
            "[KMS][fastembed] TextEmbedding::try_new FAILED model_key={} enum={:?} err={}",
            key,
            model_enum,
            e
        );
        anyhow!("Failed to init text model {key}: {e}")
    })?;
    map.insert(key.to_string(), m);
    Ok(())
}

/// Generates a text embedding with optional metadata enrichment (clipboard / expansion context).
/// Uses the default KMS text model (`BGESmallENV15` when id is unset).
pub fn generate_text_embedding(text: &str, metadata: Option<&Value>) -> Result<Vec<f32>> {
    generate_text_embedding_with_model(text, metadata, None)
}

/// Same as [`generate_text_embedding`], but selects the fastembed text model by KMS model id (empty = default).
pub fn generate_text_embedding_with_model(
    text: &str,
    metadata: Option<&Value>,
    model_id: Option<&str>,
) -> Result<Vec<f32>> {
    let key = normalized_embedding_model_id(model_id.unwrap_or(""));
    let model_enum = kms_text_embedding_model_from_id(&key).map_err(|e| {
        warn_emit(
            "fastembed",
            format!(
                "unknown model id model_key={} configured={:?} err={}",
                key, model_id, e
            ),
        );
        anyhow!(e)
    })?;
    ensure_text_model(&key, model_enum)?;

    let mut enriched = text.to_string();
    if let Some(meta) = metadata {
        if let Some(title) = meta.get("window_title").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: Window: {title}]"));
        }
        if let Some(app) = meta.get("process_name").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: App: {app}]"));
        }
        if let Some(file) = meta.get("file_name").and_then(|v| v.as_str()) {
            enriched.push_str(&format!(" [Context: File: {file}]"));
        }
    }

    let enriched_chars = enriched.chars().count();
    let mut map = text_models().lock().unwrap();
    let model = map
        .get_mut(&key)
        .ok_or_else(|| anyhow!("Text embedding model {key} missing after init"))?;
    let embeddings = model.embed(vec![enriched], None).map_err(|e| {
        warn_emit(
            "fastembed",
            format!(
                "embed(single) FAILED model_key={} enriched_chars={} err={}",
                key, enriched_chars, e
            ),
        );
        anyhow!("Single text embed failed: {e}")
    })?;
    let first = embeddings
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("No text embedding generated"))?;
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][fastembed] embed(single) ok model_key={} dim={}",
        key,
        first.len()
    );
    Ok(first)
}

/// One ONNX forward pass over many strings (uses fastembed internal batching); avoids per-chunk mutex + session work.
pub fn generate_text_embeddings_batch_with_model(
    texts: &[String],
    model_id: Option<&str>,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }
    let key = normalized_embedding_model_id(model_id.unwrap_or(""));
    let model_enum = kms_text_embedding_model_from_id(&key).map_err(|e| {
        log::warn!(
            target: KMS_EMBED_LOG_TARGET,
            "[KMS][fastembed] unknown model id (batch) model_key={} configured={:?} err={}",
            key,
            model_id,
            e
        );
        anyhow!(e)
    })?;
    ensure_text_model(&key, model_enum)?;
    let total_chars: usize = texts.iter().map(|s| s.chars().count()).sum();
    let min_piece = texts.iter().map(|s| s.chars().count()).min().unwrap_or(0);
    let max_piece = texts.iter().map(|s| s.chars().count()).max().unwrap_or(0);
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][fastembed] embed(batch) start model_key={} pieces={} total_chars={} min_piece_chars={} max_piece_chars={}",
        key,
        texts.len(),
        total_chars,
        min_piece,
        max_piece
    );
    let mut map = text_models().lock().unwrap();
    let model = map
        .get_mut(&key)
        .ok_or_else(|| anyhow!("Text embedding model {key} missing after init"))?;
    let out = model.embed(texts, None).map_err(|e| {
        warn_emit(
            "fastembed",
            format!(
                "embed(batch) FAILED model_key={} pieces={} total_chars={} err={}",
                key,
                texts.len(),
                total_chars,
                e
            ),
        );
        anyhow!("Batch text embed failed: {e}")
    })?;
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][fastembed] embed(batch) ok model_key={} returned_vectors={} dims_sample={:?}",
        key,
        out.len(),
        out.first().map(|v| v.len())
    );
    Ok(out)
}

/// Generates an image embedding using the CLIP vision model.
#[allow(dead_code)]
pub fn generate_image_embedding(image_path: &Path) -> Result<Vec<f32>> {
    let mut lock = IMAGE_MODEL.lock().unwrap();
    if lock.is_none() {
        let model = ImageEmbedding::try_new(
            ImageInitOptions::new(fastembed::ImageEmbeddingModel::ClipVitB32)
                .with_show_download_progress(true),
        )
        .map_err(|e| anyhow!("Failed to init image model: {}", e))?;
        *lock = Some(model);
    }

    let path_str = image_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid image path: {:?}", image_path))?;

    let model = lock.as_mut().unwrap();
    let embeddings = model.embed(vec![path_str], None)?;
    embeddings
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("No image embedding generated"))
}
