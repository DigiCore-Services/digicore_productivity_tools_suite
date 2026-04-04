//! Bounded service for local-graph IPC orchestration.
//! Extracted from `kms_graph_ipc` to keep inbound adapter modules focused.

use std::path::PathBuf;
use std::time::Instant;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_service;
use crate::kms_repository;

use super::ApiImpl;
use super::ipc_error;
use super::{KmsAiBeamDto, KmsClusterLabelDto, KmsEdgeDto, KmsGraphDto, KmsNodeDto};

fn built_local_to_kms_graph_dto(
    b: kms_graph_service::BuiltLocalGraph,
    request_id: String,
) -> KmsGraphDto {
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
        ai_beams: Vec::<KmsAiBeamDto>::new(),
        warnings: b.warnings,
        pagination: None,
        build_time_ms: 0,
        request_id,
    }
}

pub(crate) fn get_local_graph_dto(
    host: ApiImpl,
    path: String,
    depth: u32,
    request_id: String,
) -> Result<KmsGraphDto, String> {
    let t_build = Instant::now();
    let path_buf = PathBuf::from(path.trim());
    let rel_center = if path_buf.is_absolute() {
        host.get_relative_path(&path_buf)
            .map_err(|e| ipc_error("KMS_PATH_OUTSIDE_VAULT", e, None))?
    } else {
        path.trim().replace('\\', "/")
    };

    let all_notes =
        kms_repository::get_all_notes_minimal().map_err(|e| ipc_error("KMS_REPO_NOTES", e.to_string(), None))?;

    let norm_path_key = |p: &str| p.replace('\\', "/").trim().to_lowercase();
    let indexed = all_notes
        .iter()
        .any(|n| norm_path_key(&n.path) == norm_path_key(&rel_center));
    if !indexed {
        return Err(ipc_error(
            "KMS_NOTE_NOT_INDEXED",
            "That path is not an indexed note in this vault.",
            Some(rel_center.clone()),
        ));
    }
    let canonical_rel = all_notes
        .iter()
        .find(|n| norm_path_key(&n.path) == norm_path_key(&rel_center))
        .map(|n| n.path.clone())
        .unwrap_or_else(|| rel_center.clone());

    let vault = host.get_vault_path();
    let build_params = {
        let g = host
            .state
            .lock()
            .map_err(|e| ipc_error("KMS_STATE_LOCK", e.to_string(), None))?;
        crate::kms_graph_effective_params::effective_graph_build_params(&*g, &vault)
    };
    let note_count = all_notes.len() as u32;
    let mut effective_depth = depth.max(1);
    if note_count >= build_params.warn_note_threshold.max(1500) {
        let depth_cap = if note_count >= build_params.warn_note_threshold.saturating_mul(4).max(6000)
        {
            1
        } else {
            2
        };
        if effective_depth > depth_cap {
            log::warn!(
                "[KMS][Graph] event_code=KMS_LOCAL_GRAPH_DEPTH_CLAMP_WARN request_id={} requested_depth={} effective_depth={} note_count={}",
                request_id,
                depth,
                depth_cap,
                note_count
            );
            effective_depth = depth_cap;
        }
    }

    let emb = crate::kms_graph_ports::KmsRepositoryEmbeddingsAdapter;
    let built = kms_graph_service::build_local_graph(
        &vault,
        &canonical_rel,
        effective_depth,
        &build_params,
        &emb,
    )
    .map_err(|e| ipc_error("KMS_GRAPH_BUILD", e, None))?;

    let mut dto = built_local_to_kms_graph_dto(built, request_id.clone());
    dto.build_time_ms = t_build.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
    if effective_depth != depth {
        dto.warnings.push(format!(
            "Local graph depth was reduced from {} to {} for large-vault safety ({} indexed notes).",
            depth, effective_depth, note_count
        ));
    }

    KmsDiagnosticService::debug(
        &format!(
            "[KMS][Graph] local dto request_id={} ({} notes, {} edges, {} warnings) in {}ms",
            dto.request_id,
            dto.nodes.len(),
            dto.edges.len(),
            dto.warnings.len(),
            dto.build_time_ms
        ),
        None,
    );

    crate::kms_graph_build_ring::push_graph_build_entry(crate::kms_graph_build_ring::KmsGraphBuildRingEntry {
        kind: "local".to_string(),
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

