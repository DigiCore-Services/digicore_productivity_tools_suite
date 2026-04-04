//! Bounded service for full-graph IPC orchestration.
//! Extracted from `kms_graph_ipc` to keep inbound adapter modules focused.

use std::time::Instant;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_service;

use super::ApiImpl;
use super::ipc_error;
use super::{
    KmsAiBeamDto, KmsClusterLabelDto, KmsEdgeDto, KmsGraphDto, KmsGraphPaginationDto, KmsNodeDto,
};

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

pub(crate) async fn get_graph_dto(
    host: ApiImpl,
    offset: u32,
    limit: u32,
    time_from_utc: Option<String>,
    time_to_utc: Option<String>,
    request_id: String,
) -> Result<KmsGraphDto, String> {
    let vault = host.get_vault_path();
    let (params, temporal) = {
        let g = host
            .state
            .lock()
            .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
        let p = crate::kms_graph_effective_params::effective_graph_build_params(&*g, &vault);
        let t = kms_graph_service::TemporalRpcOverride {
            time_from_utc,
            time_to_utc,
        };
        (p, t)
    };
    let pag = if limit > 0 { Some((offset, limit)) } else { None };

    let dto = tokio::task::spawn_blocking(move || {
        let t_build = Instant::now();
        let built = kms_graph_service::build_full_graph_with_ports(
            &vault,
            &params,
            pag,
            &crate::kms_graph_ports::KmsRepositoryGraphAdapter,
            &crate::kms_graph_ports::WikiLinkAdjacencyCacheAdapter,
            &crate::kms_graph_ports::KmsRepositoryEmbeddingsAdapter,
            &temporal,
        )
        .map_err(|e| ipc_error("KMS_GRAPH_BUILD", e, None))?;
        let mut dto = built_to_kms_graph_dto(built, request_id);
        dto.build_time_ms = t_build.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
        Ok::<KmsGraphDto, String>(dto)
    })
    .await
    .map_err(|e| ipc_error("KMS_GRAPH_WORKER", format!("task join: {e}"), None))??;

    let cluster_assignments = dto.nodes.iter().filter(|n| n.cluster_id.is_some()).count();
    KmsDiagnosticService::debug(
        &format!(
            "[KMS][Graph] built dto request_id={} ({} notes, {} edges, ~{} clustered nodes, {} ai_beams, {} warnings) in {}ms",
            dto.request_id,
            dto.nodes.len(),
            dto.edges.len(),
            cluster_assignments,
            dto.ai_beams.len(),
            dto.warnings.len(),
            dto.build_time_ms
        ),
        None,
    );

    crate::kms_graph_build_ring::push_graph_build_entry(crate::kms_graph_build_ring::KmsGraphBuildRingEntry {
        kind: "full".to_string(),
        recorded_at_unix_ms: crate::kms_graph_build_ring::unix_ms_now(),
        request_id: dto.request_id.clone(),
        build_time_ms: dto.build_time_ms,
        node_count: dto.nodes.len(),
        edge_count: dto.edges.len(),
        beam_count: dto.ai_beams.len(),
        warning_count: dto.warnings.len(),
        pagination: dto
            .pagination
            .as_ref()
            .and_then(|p| serde_json::to_value(p).ok()),
        warnings_tail: crate::kms_graph_build_ring::truncate_warnings_for_ring(&dto.warnings),
    });

    Ok(dto)
}

