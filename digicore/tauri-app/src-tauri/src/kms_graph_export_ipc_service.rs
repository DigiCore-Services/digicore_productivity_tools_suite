//! Bounded service for graph export-oriented IPC orchestration.

use std::collections::HashMap;
use std::time::Instant;

use digicore_text_expander::application::expansion_diagnostics;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_service;
use crate::kms_repository;

use super::ApiImpl;
use super::ipc_error;
use super::{
    KmsAiBeamDto, KmsClusterLabelDto, KmsEdgeDto, KmsGraphDto, KmsGraphPaginationDto, KmsNodeDto,
};

fn xml_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
}

fn built_to_kms_graph_dto(b: kms_graph_service::BuiltFullGraph, request_id: String) -> KmsGraphDto {
    KmsGraphDto {
        nodes: b
            .nodes
            .into_iter()
            .map(|n| KmsNodeDto {
                id: n.id,
                path: n.abs_path,
                title: n.title,
                node_type: n.node_type,
                last_modified: n.last_modified,
                folder_path: n.folder_path,
                cluster_id: n.cluster_id,
                link_centrality: n.link_centrality,
            })
            .collect(),
        edges: b
            .edges
            .into_iter()
            .map(|e| KmsEdgeDto {
                source: e.source,
                target: e.target,
                kind: e.kind,
                edge_recency: e.edge_recency,
            })
            .collect(),
        cluster_labels: b
            .cluster_labels
            .into_iter()
            .map(|(cluster_id, label)| KmsClusterLabelDto { cluster_id, label })
            .collect(),
        ai_beams: b
            .beams
            .into_iter()
            .map(|b| KmsAiBeamDto {
                source_path: b.source_path,
                target_path: b.target_path,
                summary: b.summary,
            })
            .collect(),
        warnings: b.warnings,
        pagination: b.pagination.map(|p| KmsGraphPaginationDto {
            total_nodes: p.total_nodes,
            offset: p.offset,
            limit: p.limit,
            returned_nodes: p.returned_nodes,
            has_more: p.has_more,
        }),
        build_time_ms: 0,
        request_id,
    }
}

pub(crate) async fn export_graph_diagnostics(host: ApiImpl, path: String, request_id: String) -> Result<(), String> {
    let vault = host.get_vault_path();
    let vault_fp = crate::kms_graph_effective_params::vault_path_fingerprint_hex16(&vault);
    let (params, graph_flags) = {
        let guard = host
            .state
            .lock()
            .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
        let p = crate::kms_graph_effective_params::effective_graph_build_params(&*guard, &vault);
        let flags = serde_json::json!({
            "kms_graph_auto_paging_enabled": guard.kms_graph_auto_paging_enabled,
            "kms_graph_auto_paging_note_threshold": guard.kms_graph_auto_paging_note_threshold,
            "kms_graph_enable_semantic_clustering": guard.kms_graph_enable_semantic_clustering,
            "kms_graph_enable_leiden_communities": guard.kms_graph_enable_leiden_communities,
            "kms_graph_enable_ai_beams": guard.kms_graph_enable_ai_beams,
            "kms_graph_pagerank_scope": guard.kms_graph_pagerank_scope,
            "kms_graph_background_wiki_pagerank_enabled": guard.kms_graph_background_wiki_pagerank_enabled,
            "background_wiki_pagerank_enabled_effective": p.background_wiki_pagerank_enabled,
        });
        (p, flags)
    };
    let stats = kms_repository::get_diag_summary()
        .map_err(|e| ipc_error("KMS_REPO_DIAG", e.to_string(), None))?;
    let recent_diag: Vec<serde_json::Value> = expansion_diagnostics::get_recent()
        .into_iter()
        .rev()
        .take(50)
        .filter_map(|e| serde_json::to_value(&e).ok())
        .collect();
    let recent_graph_ring: Vec<serde_json::Value> = crate::kms_graph_build_ring::snapshot_ring_oldest_first()
        .into_iter()
        .filter_map(|e| serde_json::to_value(&e).ok())
        .collect();
    let exported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let payload = serde_json::json!({
        "schema_version": "kms_graph_diagnostics_v1",
        "export_request_id": request_id,
        "exported_at_unix_ms": exported_at,
        "vault_fingerprint_hex16": vault_fp,
        "kms_diag_summary": serde_json::to_value(&stats).unwrap_or(serde_json::json!({})),
        "effective_graph_build_params": crate::kms_graph_effective_params::kms_graph_build_params_to_json(&params),
        "graph_related_app_state": graph_flags,
        "recent_expansion_diagnostics_tail": recent_diag,
        "recent_graph_build_ring_tail": recent_graph_ring,
    });
    let pretty = serde_json::to_string_pretty(&payload)
        .map_err(|e| ipc_error("KMS_GRAPH_DIAG_JSON", e.to_string(), None))?;
    std::fs::write(path.trim(), pretty).map_err(|e| {
        ipc_error(
            "KMS_GRAPH_DIAG_IO",
            format!("write failed: {e}"),
            Some(request_id.clone()),
        )
    })?;
    log::info!("[KMS][Graph] diagnostics export request_id={}", request_id);
    KmsDiagnosticService::debug(
        &format!("[KMS][Graph] diagnostics export request_id={}", request_id),
        None,
    );
    Ok(())
}

pub(crate) async fn export_wiki_links_json(host: ApiImpl, path: String, request_id: String) -> Result<(), String> {
    let vault = host.get_vault_path();
    let vault_fp = crate::kms_graph_effective_params::vault_path_fingerprint_hex16(&vault);
    let links = crate::kms_link_adjacency_cache::get_all_links_cached()
        .map_err(|e| ipc_error("KMS_REPO_LINKS", e.to_string(), Some(request_id.clone())))?;
    let exported_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let edges: Vec<serde_json::Value> = links
        .iter()
        .map(|(s, t)| serde_json::json!({ "source": s, "target": t }))
        .collect();
    let payload = serde_json::json!({
        "schema_version": "kms_wiki_links_export_v1",
        "export_request_id": request_id,
        "exported_at_unix_ms": exported_at,
        "vault_fingerprint_hex16": vault_fp,
        "edge_count": links.len(),
        "edges": edges,
    });
    let pretty = serde_json::to_string_pretty(&payload)
        .map_err(|e| ipc_error("KMS_LINKS_EXPORT_JSON", e.to_string(), None))?;
    std::fs::write(path.trim(), pretty).map_err(|e| {
        ipc_error(
            "KMS_LINKS_EXPORT_IO",
            format!("write failed: {e}"),
            None,
        )
    })?;
    log::info!("[KMS][Graph] wiki links JSON export request_id={}", request_id);
    Ok(())
}

pub(crate) async fn export_graph_graphml(host: ApiImpl, path: String, request_id: String) -> Result<(), String> {
    let vault = host.get_vault_path();
    let params = {
        let g = host
            .state
            .lock()
            .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
        crate::kms_graph_effective_params::effective_graph_build_params(&*g, &vault)
    };
    let out_path = path.trim().to_string();
    let rid_log = request_id.clone();
    tokio::task::spawn_blocking(move || {
        let built = kms_graph_service::build_full_graph(&vault, &params, None)
            .map_err(|e| ipc_error("KMS_GRAPH_BUILD", e, Some(request_id.clone())))?;
        let mut w = String::new();
        w.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        w.push_str("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n");
        w.push_str("  <key id=\"path\" for=\"node\" attr.name=\"path\" attr.type=\"string\"/>\n");
        w.push_str("  <key id=\"label\" for=\"node\" attr.name=\"label\" attr.type=\"string\"/>\n");
        w.push_str("  <key id=\"kind\" for=\"edge\" attr.name=\"kind\" attr.type=\"string\"/>\n");
        w.push_str("  <graph id=\"kms_wiki\" edgedefault=\"undirected\">\n");
        let mut path_to_id: HashMap<String, usize> = HashMap::new();
        for (i, n) in built.nodes.iter().enumerate() {
            path_to_id.insert(n.abs_path.clone(), i);
            let path_esc = xml_escape_text(&n.abs_path);
            let label_esc = xml_escape_text(&n.title);
            w.push_str(&format!(
                "    <node id=\"n{i}\"><data key=\"path\">{path_esc}</data><data key=\"label\">{label_esc}</data></node>\n"
            ));
        }
        let mut ei = 0usize;
        for e in &built.edges {
            if let (Some(&si), Some(&ti)) =
                (path_to_id.get(&e.source), path_to_id.get(&e.target))
            {
                let kind_esc = xml_escape_text(&e.kind);
                w.push_str(&format!(
                    "    <edge id=\"e{ei}\" source=\"n{si}\" target=\"n{ti}\"><data key=\"kind\">{kind_esc}</data></edge>\n"
                ));
                ei += 1;
            }
        }
        w.push_str("  </graph>\n</graphml>\n");
        std::fs::write(&out_path, w).map_err(|e| {
            ipc_error(
                "KMS_GRAPHML_IO",
                format!("write failed: {e}"),
                Some(request_id.clone()),
            )
        })?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| ipc_error("KMS_GRAPH_WORKER", format!("task join: {e}"), None))??;
    log::info!("[KMS][Graph] graphml export request_id={}", rid_log);
    Ok(())
}

pub(crate) async fn export_graph_dto_json(host: ApiImpl, path: String, request_id: String) -> Result<(), String> {
    let vault = host.get_vault_path();
    let vault_fp = crate::kms_graph_effective_params::vault_path_fingerprint_hex16(&vault);
    let params = {
        let g = host
            .state
            .lock()
            .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
        crate::kms_graph_effective_params::effective_graph_build_params(&*g, &vault)
    };
    let out_path = path.trim().to_string();
    let rid_log = request_id.clone();
    tokio::task::spawn_blocking(move || {
        let t_build = Instant::now();
        let built = kms_graph_service::build_full_graph(&vault, &params, None)
            .map_err(|e| ipc_error("KMS_GRAPH_BUILD", e, Some(request_id.clone())))?;
        let mut dto = built_to_kms_graph_dto(built, request_id.clone());
        dto.build_time_ms = t_build
            .elapsed()
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        let exported_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let graph_value = serde_json::to_value(&dto).map_err(|e| {
            ipc_error(
                "KMS_GRAPH_DTO_EXPORT_JSON",
                format!("serialize graph DTO: {e}"),
                Some(request_id.clone()),
            )
        })?;
        let payload = serde_json::json!({
            "schema_version": "kms_graph_dto_export_v1",
            "export_request_id": request_id,
            "exported_at_unix_ms": exported_at,
            "vault_fingerprint_hex16": vault_fp,
            "graph": graph_value,
        });
        let pretty = serde_json::to_string_pretty(&payload).map_err(|e| {
            ipc_error(
                "KMS_GRAPH_DTO_EXPORT_JSON",
                format!("pretty JSON: {e}"),
                None,
            )
        })?;
        std::fs::write(&out_path, pretty).map_err(|e| {
            ipc_error(
                "KMS_GRAPH_DTO_EXPORT_IO",
                format!("write failed: {e}"),
                None,
            )
        })?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| ipc_error("KMS_GRAPH_WORKER", format!("task join: {e}"), None))??;
    log::info!("[KMS][Graph] graph DTO JSON export request_id={}", rid_log);
    Ok(())
}

