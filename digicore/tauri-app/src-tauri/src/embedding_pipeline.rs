//! O3: central note text embedding path (normalize -> generator port -> store -> note `embedding_model_id` + `embedding_policy_sig`).

use crate::embedding_service::{self};
use crate::kms_embed_diagnostic_log::{warn_emit, KMS_EMBED_LOG_TARGET};
use crate::kms_error::KmsError;
use crate::kms_repository;

pub use digicore_kms_ports::{
    EmbeddingGeneratorPort, EmbeddingStorePort, KmsTextEmbeddingChunkConfig,
};

/// KMS note/query embedding via fastembed; `model_id` is the configured KMS id (empty = default).
#[allow(dead_code)]
pub struct FastembedTextGenerator {
    pub model_id: String,
}

impl EmbeddingGeneratorPort for FastembedTextGenerator {
    fn embed_text_note(&self, text: &str) -> Result<Vec<f32>, String> {
        embedding_service::generate_text_embedding_with_model(text, None, Some(&self.model_id))
            .map_err(|e| e.to_string())
    }
}

pub struct KmsSqliteNoteEmbeddingStore;

impl EmbeddingStorePort for KmsSqliteNoteEmbeddingStore {
    fn upsert_text_note(&self, entity_id: &str, vector: &[f32]) -> Result<(), String> {
        kms_repository::upsert_embedding("text", "note", entity_id, vector, None).map_err(|e| e.to_string())
    }
}

/// Trims and strips BOM; collapses excessive horizontal whitespace (single-line friendly).
pub fn normalize_note_text_for_embedding(text: &str) -> String {
    let t = text.trim().trim_start_matches('\u{feff}');
    let mut out = String::with_capacity(t.len().min(256 * 1024));
    let mut prev_space = false;
    for ch in t.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn l2_normalize_vec(v: &mut [f32]) {
    let s: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if s > 1e-12 {
        for x in v {
            *x /= s;
        }
    }
}

fn mean_pool_embeddings(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let dim = vectors.first()?.len();
    if vectors.iter().any(|v| v.len() != dim) {
        return None;
    }
    let n = vectors.len() as f32;
    let mut sum = vec![0f32; dim];
    for v in vectors {
        for i in 0..dim {
            sum[i] += v[i];
        }
    }
    for x in &mut sum {
        *x /= n;
    }
    Some(sum)
}

/// Unicode-safe character windows with optional overlap.
pub fn chunk_text_for_embedding(s: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![s.to_string()];
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let step = max_chars.saturating_sub(overlap).max(1);
    loop {
        let end = (start + max_chars).min(chars.len());
        out.push(chars[start..end].iter().collect());
        if end >= chars.len() {
            break;
        }
        start = start.saturating_add(step);
    }
    out
}

#[cfg(test)]
fn embed_normalized_with_chunks<G: EmbeddingGeneratorPort>(
    gen: &G,
    normalized: &str,
    chunk_cfg: &KmsTextEmbeddingChunkConfig,
) -> Result<Vec<f32>, String> {
    let cfg = chunk_cfg.clamped();
    let n_chars = normalized.chars().count();
    if !cfg.enabled || n_chars <= cfg.max_chars as usize {
        return gen.embed_text_note(normalized);
    }
    let pieces = chunk_text_for_embedding(
        normalized,
        cfg.max_chars as usize,
        cfg.overlap_chars as usize,
    );
    if pieces.len() <= 1 {
        return gen.embed_text_note(normalized);
    }
    let mut embeddings = Vec::with_capacity(pieces.len());
    for p in pieces {
        embeddings.push(gen.embed_text_note(&p)?);
    }
    let mut pooled = mean_pool_embeddings(&embeddings).ok_or_else(|| "embedding dimension mismatch".to_string())?;
    l2_normalize_vec(&mut pooled);
    Ok(pooled)
}

/// Fastembed path for KMS storage/search: chunked notes run **one batched** `embed` call (not N sequential calls).
fn embed_normalized_fastembed_batched(
    normalized: &str,
    model_id: &str,
    chunk_cfg: &KmsTextEmbeddingChunkConfig,
) -> Result<Vec<f32>, String> {
    let cfg = chunk_cfg.clamped();
    let n_chars = normalized.chars().count();
    let pieces: Vec<String> = if !cfg.enabled || n_chars <= cfg.max_chars as usize {
        vec![normalized.to_string()]
    } else {
        let p = chunk_text_for_embedding(
            normalized,
            cfg.max_chars as usize,
            cfg.overlap_chars as usize,
        );
        if p.len() <= 1 {
            vec![normalized.to_string()]
        } else {
            p
        }
    };

    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][pipeline] batched path normalized_chars={} pieces={} chunk={{ enabled:{} max:{} overlap:{} }} model_id_raw={:?}",
        n_chars,
        pieces.len(),
        cfg.enabled,
        cfg.max_chars,
        cfg.overlap_chars,
        model_id
    );

    let embeddings = if pieces.len() == 1 {
        vec![
            embedding_service::generate_text_embedding_with_model(&pieces[0], None, Some(model_id))
                .map_err(|e| {
                    let msg = e.to_string();
                    log::warn!(
                        target: KMS_EMBED_LOG_TARGET,
                        "[KMS][pipeline] FAIL stage=single_embed pieces=1 model_id_raw={:?} err={}",
                        model_id,
                        msg
                    );
                    msg
                })?,
        ]
    } else {
        embedding_service::generate_text_embeddings_batch_with_model(&pieces, Some(model_id))
            .map_err(|e| {
                let msg = e.to_string();
                warn_emit(
                    "pipeline",
                    format!(
                        "FAIL stage=batch_embed pieces={} model_id_raw={:?} err={}",
                        pieces.len(),
                        model_id,
                        msg
                    ),
                );
                msg
            })?
    };

    if embeddings.len() == 1 {
        return Ok(embeddings[0].clone());
    }
    let dims: Vec<usize> = embeddings.iter().map(|v| v.len()).collect();
    let mut pooled = mean_pool_embeddings(&embeddings).ok_or_else(|| {
        warn_emit(
            "pipeline",
            format!(
                "FAIL stage=mean_pool piece_dims={:?} (need identical dims)",
                dims
            ),
        );
        "embedding dimension mismatch".to_string()
    })?;
    l2_normalize_vec(&mut pooled);
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][pipeline] batched ok pooled_dim={}",
        pooled.len()
    );
    Ok(pooled)
}

/// KMS hybrid/semantic search query: same normalization and chunk policy as note indexing (fastembed, single vector space).
pub fn embed_kms_query_text_blocking(
    query: &str,
    chunk_cfg: &KmsTextEmbeddingChunkConfig,
    embedding_model_id: &str,
) -> Result<Vec<f32>, String> {
    let normalized = normalize_note_text_for_embedding(query);
    embed_normalized_fastembed_batched(&normalized, embedding_model_id, chunk_cfg)
}

/// Blocking embed for one note: normalize, optional chunked mean-pool, upsert vector, persist model id on `kms_notes`.
pub fn embed_note_text_blocking(
    rel_path: &str,
    text: &str,
    model_id: &str,
    chunk_cfg: &KmsTextEmbeddingChunkConfig,
) -> Result<(), String> {
    let c = chunk_cfg.clamped();
    let mid_norm = embedding_service::normalized_embedding_model_id(model_id);
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][pipeline] note start path={} model_id_raw={:?} model_id_norm={} text_chars={} chunk={{ enabled:{} max:{} overlap:{} }}",
        rel_path,
        model_id,
        mid_norm,
        text.chars().count(),
        c.enabled,
        c.max_chars,
        c.overlap_chars
    );

    let normalized = normalize_note_text_for_embedding(text);
    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][pipeline] note normalized path={} normalized_chars={}",
        rel_path,
        normalized.chars().count()
    );

    let vec = embed_normalized_fastembed_batched(&normalized, model_id, chunk_cfg).map_err(|e| {
        warn_emit(
            "pipeline",
            format!(
                "FAIL stage=embed_vector path={} model_id_norm={} err={}",
                rel_path, mid_norm, e
            ),
        );
        e
    })?;

    let store = KmsSqliteNoteEmbeddingStore;
    store.upsert_text_note(rel_path, &vec).map_err(|e| {
        warn_emit(
            "pipeline",
            format!(
                "FAIL stage=sqlite_upsert_embedding path={} vector_dim={} err={}",
                rel_path,
                vec.len(),
                e
            ),
        );
        e
    })?;

    let sig = kms_repository::note_embedding_policy_sig(
        &mid_norm,
        c.enabled,
        c.max_chars,
        c.overlap_chars,
        vec.len(),
    );
    kms_repository::set_note_embedding_identity(rel_path, &mid_norm, &sig).map_err(|e: KmsError| {
        let s = e.to_string();
        warn_emit(
            "pipeline",
            format!(
                "FAIL stage=set_note_embedding_identity path={} sig={} err={}",
                rel_path, sig, s
            ),
        );
        s
    })?;

    log::debug!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][pipeline] note ok path={} dim={}",
        rel_path,
        vec.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedGen(Vec<f32>);
    impl EmbeddingGeneratorPort for FixedGen {
        fn embed_text_note(&self, _text: &str) -> Result<Vec<f32>, String> {
            Ok(self.0.clone())
        }
    }

    struct DimGen(usize);
    impl EmbeddingGeneratorPort for DimGen {
        fn embed_text_note(&self, text: &str) -> Result<Vec<f32>, String> {
            let mut v = vec![0f32; self.0];
            v[0] = text.len() as f32;
            Ok(v)
        }
    }

    struct RecordingStore {
        upserts: std::sync::Mutex<Vec<(String, usize)>>,
    }
    impl EmbeddingStorePort for RecordingStore {
        fn upsert_text_note(&self, entity_id: &str, vector: &[f32]) -> Result<(), String> {
            self.upserts
                .lock()
                .unwrap()
                .push((entity_id.to_string(), vector.len()));
            Ok(())
        }
    }

    #[test]
    fn normalize_trims_and_collapses_whitespace() {
        assert_eq!(
            normalize_note_text_for_embedding("  a \n\t b  "),
            "a b"
        );
    }

    #[test]
    fn pipeline_invokes_generator_then_store() {
        let gen = FixedGen(vec![1.0, 2.0, 3.0]);
        let store = RecordingStore {
            upserts: std::sync::Mutex::new(Vec::new()),
        };
        let v = gen.embed_text_note("x").unwrap();
        store.upsert_text_note("note.md", &v).unwrap();
        let g = store.upserts.lock().unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "note.md");
        assert_eq!(g[0].1, 3);
    }

    #[test]
    fn chunk_text_splits_long_strings() {
        let s: String = (0..90).map(|_| 'x').collect();
        let parts = chunk_text_for_embedding(&s, 30, 0);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts.concat(), s);
    }

    #[test]
    fn mean_pool_averages_dimensions() {
        let v = mean_pool_embeddings(&[vec![2.0, 0.0], vec![0.0, 4.0]]).unwrap();
        assert!((v[0] - 1.0).abs() < 1e-5);
        assert!((v[1] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn chunked_embed_mean_pools() {
        let gen = DimGen(2);
        let cfg = KmsTextEmbeddingChunkConfig {
            enabled: true,
            max_chars: 256,
            overlap_chars: 0,
        };
        let text: String = (0..520).map(|_| 'a').collect();
        let normalized = normalize_note_text_for_embedding(&text);
        let out = embed_normalized_with_chunks(&gen, &normalized, &cfg).unwrap();
        assert_eq!(out.len(), 2);
        let sq: f32 = out.iter().map(|x| x * x).sum();
        assert!((sq - 1.0).abs() < 1e-4, "expected L2-normalized pooled vector");
    }
}
