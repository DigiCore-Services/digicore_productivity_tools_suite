//! Merges global AppState graph tunables with per-vault JSON overrides (hexagonal helper).

use std::path::Path;

use digicore_kms_ports::KmsTextEmbeddingChunkConfig;
use digicore_text_expander::application::app_state::AppState;
use sha2::{Digest, Sha256};

use crate::kms_graph_service;

pub(crate) fn kms_graph_build_params_to_json(p: &kms_graph_service::KmsGraphBuildParams) -> serde_json::Value {
    serde_json::json!({
        "enable_semantic_clustering": p.enable_semantic_clustering,
        "enable_leiden_communities": p.enable_leiden_communities,
        "enable_ai_beams": p.enable_ai_beams,
        "k_means_max_k": p.k_means_max_k,
        "k_means_iterations": p.k_means_iterations,
        "ai_beam_max_nodes": p.ai_beam_max_nodes,
        "ai_beam_similarity_threshold": p.ai_beam_similarity_threshold,
        "ai_beam_max_edges": p.ai_beam_max_edges,
        "semantic_max_notes": p.semantic_max_notes,
        "warn_note_threshold": p.warn_note_threshold,
        "beam_max_pair_checks": p.beam_max_pair_checks,
        "pagerank_iterations": p.pagerank_iterations,
        "pagerank_local_iterations": p.pagerank_local_iterations,
        "pagerank_damping": p.pagerank_damping,
        "pagerank_scope": p.pagerank_scope,
        "background_wiki_pagerank_enabled": p.background_wiki_pagerank_enabled,
        "enable_semantic_knn_edges": p.enable_semantic_knn_edges,
        "semantic_knn_per_note": p.semantic_knn_per_note,
        "semantic_knn_min_similarity": p.semantic_knn_min_similarity,
        "semantic_knn_max_edges": p.semantic_knn_max_edges,
        "semantic_knn_max_pair_checks": p.semantic_knn_max_pair_checks,
        "temporal_window_enabled": p.temporal_window_enabled,
        "temporal_default_days": p.temporal_default_days,
        "temporal_include_notes_without_mtime": p.temporal_include_notes_without_mtime,
        "temporal_edge_recency_enabled": p.temporal_edge_recency_enabled,
        "temporal_edge_recency_strength": p.temporal_edge_recency_strength,
        "temporal_edge_recency_half_life_days": p.temporal_edge_recency_half_life_days,
    })
}

pub(crate) fn vault_path_fingerprint_hex16(vault: &Path) -> String {
    let h = Sha256::digest(vault.to_string_lossy().as_bytes());
    let s = format!("{:x}", h);
    s.chars().take(16).collect()
}

fn graph_build_params_from_app_state(g: &AppState) -> kms_graph_service::KmsGraphBuildParams {
    kms_graph_service::KmsGraphBuildParams {
        enable_semantic_clustering: g.kms_graph_enable_semantic_clustering,
        enable_leiden_communities: g.kms_graph_enable_leiden_communities,
        enable_ai_beams: g.kms_graph_enable_ai_beams,
        k_means_max_k: g.kms_graph_k_means_max_k.max(2),
        k_means_iterations: g.kms_graph_k_means_iterations.max(1) as usize,
        ai_beam_max_nodes: g.kms_graph_ai_beam_max_nodes.max(2) as usize,
        ai_beam_similarity_threshold: g.kms_graph_ai_beam_similarity_threshold.clamp(0.0, 1.0),
        ai_beam_max_edges: g.kms_graph_ai_beam_max_edges as usize,
        semantic_max_notes: g.kms_graph_semantic_max_notes,
        warn_note_threshold: g.kms_graph_warn_note_threshold,
        beam_max_pair_checks: g.kms_graph_beam_max_pair_checks as usize,
        pagerank_iterations: g.kms_graph_pagerank_iterations.max(4),
        pagerank_local_iterations: g.kms_graph_pagerank_local_iterations.max(4),
        pagerank_damping: g.kms_graph_pagerank_damping.clamp(0.5, 0.99),
        pagerank_scope: g.kms_graph_pagerank_scope.clone(),
        background_wiki_pagerank_enabled: g.kms_graph_background_wiki_pagerank_enabled,
        enable_semantic_knn_edges: g.kms_graph_enable_semantic_knn_edges,
        semantic_knn_per_note: g.kms_graph_semantic_knn_per_note.clamp(1, 30),
        semantic_knn_min_similarity: g.kms_graph_semantic_knn_min_similarity.clamp(0.5, 0.999),
        semantic_knn_max_edges: g.kms_graph_semantic_knn_max_edges.min(500_000),
        semantic_knn_max_pair_checks: g.kms_graph_semantic_knn_max_pair_checks,
        temporal_window_enabled: g.kms_graph_temporal_window_enabled,
        temporal_default_days: g.kms_graph_temporal_default_days,
        temporal_include_notes_without_mtime: g.kms_graph_temporal_include_notes_without_mtime,
        temporal_edge_recency_enabled: g.kms_graph_temporal_edge_recency_enabled,
        temporal_edge_recency_strength: g.kms_graph_temporal_edge_recency_strength,
        temporal_edge_recency_half_life_days: g.kms_graph_temporal_edge_recency_half_life_days,
    }
}

fn apply_vault_graph_patch(
    p: &mut kms_graph_service::KmsGraphBuildParams,
    obj: &serde_json::Map<String, serde_json::Value>,
) {
    if let Some(v) = obj
        .get("kms_graph_enable_semantic_clustering")
        .and_then(|x| x.as_bool())
    {
        p.enable_semantic_clustering = v;
    }
    if let Some(v) = obj
        .get("kms_graph_enable_leiden_communities")
        .and_then(|x| x.as_bool())
    {
        p.enable_leiden_communities = v;
    }
    if let Some(v) = obj.get("kms_graph_enable_ai_beams").and_then(|x| x.as_bool()) {
        p.enable_ai_beams = v;
    }
    if let Some(v) = obj.get("kms_graph_k_means_max_k").and_then(|x| x.as_u64()) {
        p.k_means_max_k = (v as u32).max(2);
    }
    if let Some(v) = obj.get("kms_graph_k_means_iterations").and_then(|x| x.as_u64()) {
        p.k_means_iterations = (v as u32).max(1) as usize;
    }
    if let Some(v) = obj.get("kms_graph_ai_beam_max_nodes").and_then(|x| x.as_u64()) {
        p.ai_beam_max_nodes = (v as u32).max(2) as usize;
    }
    if let Some(v) = obj
        .get("kms_graph_ai_beam_similarity_threshold")
        .and_then(|x| x.as_f64())
    {
        p.ai_beam_similarity_threshold = (v as f32).clamp(0.0, 1.0);
    }
    if let Some(v) = obj.get("kms_graph_ai_beam_max_edges").and_then(|x| x.as_u64()) {
        p.ai_beam_max_edges = v as usize;
    }
    if let Some(v) = obj.get("kms_graph_semantic_max_notes").and_then(|x| x.as_u64()) {
        p.semantic_max_notes = v as u32;
    }
    if let Some(v) = obj.get("kms_graph_warn_note_threshold").and_then(|x| x.as_u64()) {
        p.warn_note_threshold = v as u32;
    }
    if let Some(v) = obj.get("kms_graph_beam_max_pair_checks").and_then(|x| x.as_u64()) {
        p.beam_max_pair_checks = v as usize;
    }
    if let Some(v) = obj.get("kms_graph_pagerank_iterations").and_then(|x| x.as_u64()) {
        p.pagerank_iterations = (v as u32).max(4);
    }
    if let Some(v) = obj.get("kms_graph_pagerank_local_iterations").and_then(|x| x.as_u64()) {
        p.pagerank_local_iterations = (v as u32).max(4);
    }
    if let Some(v) = obj
        .get("kms_graph_pagerank_damping")
        .and_then(|x| x.as_f64())
    {
        p.pagerank_damping = (v as f32).clamp(0.5, 0.99);
    }
    if let Some(v) = obj
        .get("kms_graph_pagerank_scope")
        .and_then(|x| x.as_str())
    {
        p.pagerank_scope = v.to_string();
    }
    if let Some(v) = obj
        .get("kms_graph_background_wiki_pagerank_enabled")
        .and_then(|x| x.as_bool())
    {
        p.background_wiki_pagerank_enabled = v;
    }
    if let Some(v) = obj
        .get("kms_graph_enable_semantic_knn_edges")
        .and_then(|x| x.as_bool())
    {
        p.enable_semantic_knn_edges = v;
    }
    if let Some(v) = obj
        .get("kms_graph_semantic_knn_per_note")
        .and_then(|x| x.as_u64())
    {
        p.semantic_knn_per_note = (v as u32).clamp(1, 30);
    }
    if let Some(v) = obj
        .get("kms_graph_semantic_knn_min_similarity")
        .and_then(|x| x.as_f64())
    {
        p.semantic_knn_min_similarity = (v as f32).clamp(0.5, 0.999);
    }
    if let Some(v) = obj
        .get("kms_graph_semantic_knn_max_edges")
        .and_then(|x| x.as_u64())
    {
        p.semantic_knn_max_edges = (v as u32).min(500_000);
    }
    if let Some(v) = obj
        .get("kms_graph_semantic_knn_max_pair_checks")
        .and_then(|x| x.as_u64())
    {
        p.semantic_knn_max_pair_checks = (v.min(50_000_000)) as u32;
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_window_enabled")
        .and_then(|x| x.as_bool())
    {
        p.temporal_window_enabled = v;
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_default_days")
        .and_then(|x| x.as_u64())
    {
        p.temporal_default_days = v as u32;
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_include_notes_without_mtime")
        .and_then(|x| x.as_bool())
    {
        p.temporal_include_notes_without_mtime = v;
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_edge_recency_enabled")
        .and_then(|x| x.as_bool())
    {
        p.temporal_edge_recency_enabled = v;
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_edge_recency_strength")
        .and_then(|x| x.as_f64())
    {
        p.temporal_edge_recency_strength = (v as f32).clamp(0.0, 1.0);
    }
    if let Some(v) = obj
        .get("kms_graph_temporal_edge_recency_half_life_days")
        .and_then(|x| x.as_f64())
    {
        p.temporal_edge_recency_half_life_days = (v as f32).max(0.1);
    }
}

pub(crate) fn effective_graph_build_params(
    state: &AppState,
    vault_root: &Path,
) -> kms_graph_service::KmsGraphBuildParams {
    let mut p = graph_build_params_from_app_state(state);
    let key = kms_graph_service::vault_graph_settings_key(vault_root);
    if let Ok(map) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&state.kms_graph_vault_overrides_json)
    {
        if let Some(entry) = map.get(&key) {
            if let Some(obj) = entry.as_object() {
                apply_vault_graph_patch(&mut p, obj);
            }
        }
    }
    p
}

/// Minimum cosine similarity for the vector leg of hybrid/semantic KMS search (0 = off). Merged with vault JSON overrides.
pub(crate) fn effective_kms_search_min_similarity(state: &AppState, vault_root: &std::path::Path) -> f32 {
    let mut v = state.kms_search_min_similarity.clamp(0.0, 1.0);
    let key = kms_graph_service::vault_graph_settings_key(vault_root);
    if let Ok(map) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&state.kms_graph_vault_overrides_json)
    {
        if let Some(entry) = map.get(&key) {
            if let Some(obj) = entry.as_object() {
                if let Some(x) = obj.get("kms_search_min_similarity").and_then(|j| j.as_f64()) {
                    v = (x as f32).clamp(0.0, 1.0);
                }
            }
        }
    }
    v
}

/// Note/query chunking policy: global AppState merged with per-vault JSON (same map as graph overrides).
pub(crate) fn effective_kms_embedding_chunk_config(
    state: &AppState,
    vault_root: &Path,
) -> KmsTextEmbeddingChunkConfig {
    let mut cfg = KmsTextEmbeddingChunkConfig {
        enabled: state.kms_embedding_chunk_enabled,
        max_chars: state.kms_embedding_chunk_max_chars,
        overlap_chars: state.kms_embedding_chunk_overlap_chars,
    };
    let key = kms_graph_service::vault_graph_settings_key(vault_root);
    if let Ok(map) =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&state.kms_graph_vault_overrides_json)
    {
        if let Some(entry) = map.get(&key) {
            if let Some(obj) = entry.as_object() {
                if let Some(v) = obj.get("kms_embedding_chunk_enabled").and_then(|j| j.as_bool()) {
                    cfg.enabled = v;
                }
                if let Some(v) = obj.get("kms_embedding_chunk_max_chars").and_then(|j| j.as_u64()) {
                    cfg.max_chars = (v as u32).clamp(256, 8192);
                }
                if let Some(v) = obj.get("kms_embedding_chunk_overlap_chars").and_then(|j| j.as_u64()) {
                    cfg.overlap_chars = (v as u32).min(4096);
                }
            }
        }
    }
    let max_o = cfg.max_chars / 2;
    cfg.overlap_chars = cfg.overlap_chars.min(max_o);
    cfg.clamped()
}
