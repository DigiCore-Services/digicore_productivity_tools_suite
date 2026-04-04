//! KMS semantic / hybrid search: query embedding, repository search, and result DTO mapping.

use crate::embedding_service;

use super::*;
use crate::kms_service::KmsService;

pub(crate) async fn kms_search_semantic(
    host: ApiImpl,
    query: String,
    modality: Option<String>,
    limit: u32,
    search_mode: Option<String>,
) -> Result<Vec<SearchResultDto>, String> {
    let request_id = kms_request_id("search_semantic");
    let modality = modality.unwrap_or_else(|| "text".to_string());
    let search_mode = search_mode.unwrap_or_else(|| "Hybrid".to_string());
    let vault = host.get_vault_path();
    let (min_vector_sim, chunk_cfg, eff_embed_model, include_embed_diag) = {
        let g = host.state.lock().map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_STATE_LOCK",
                "KMS_SEARCH_STATE_LOCK_FAIL",
                "Failed to lock app state",
                Some(e.to_string()),
            )
        })?;
        (
            crate::kms_graph_effective_params::effective_kms_search_min_similarity(&*g, &vault),
            crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, &vault),
            embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id),
            g.kms_search_include_embedding_diagnostics,
        )
    };

    let request_id_in_task = request_id.clone();
    tokio::task::spawn_blocking(move || {
        let t_embed = std::time::Instant::now();
        let vector = crate::embedding_pipeline::embed_kms_query_text_blocking(
            &query,
            &chunk_cfg,
            &eff_embed_model,
        )
        .map_err(|e| {
            kms_ipc_error(
                &request_id_in_task,
                "KMS_SEARCH_EMBED",
                "KMS_SEARCH_EMBED_FAIL",
                "Embedding failed for semantic search",
                Some(e.to_string()),
            )
        })?;
        let kms_query_embedding_ms = t_embed.elapsed().as_secs_f32() * 1000.0;

        let results = kms_repository::search_hybrid(
            &query,
            &modality,
            vector,
            &search_mode,
            limit,
            min_vector_sim,
        )
        .map_err(|e| {
            kms_ipc_error(
                &request_id_in_task,
                "KMS_SEARCH_REPO",
                "KMS_SEARCH_QUERY_FAIL",
                "Search query failed",
                Some(e.to_string()),
            )
        })?;

        Ok(results
            .into_iter()
            .map(|r| {
                let final_id = if r.entity_type == "note" {
                    host
                        .resolve_absolute_path(&r.entity_id)
                        .to_string_lossy()
                        .to_string()
                } else {
                    r.entity_id
                };

                let mut snippet = None;
                if r.entity_type == "note" {
                    if let Ok(content) = std::fs::read_to_string(&final_id) {
                        snippet = Some(KmsService::extract_contextual_snippet(&content, &query));
                    }
                } else if r.entity_type == "snippet" || r.entity_type == "clipboard" {
                    if let Some(meta_str) = &r.metadata {
                        if let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(meta_str) {
                            snippet = meta_json
                                .get("content")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                        }
                    }

                    if snippet.is_none() {
                        snippet = r.metadata.clone();
                    }
                }

                SearchResultDto {
                    entity_type: r.entity_type,
                    entity_id: final_id,
                    distance: r.distance,
                    modality: r.modality,
                    metadata: r.metadata,
                    snippet,
                    kms_query_embedding_ms: include_embed_diag.then_some(kms_query_embedding_ms),
                    kms_effective_embedding_model_id: include_embed_diag
                        .then(|| eff_embed_model.clone()),
                }
            })
            .collect())
    })
    .await
    .map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SEARCH_WORKER",
            "KMS_SEARCH_TASK_JOIN_FAIL",
            "Search worker task failed",
            Some(e.to_string()),
        )
    })?
}

