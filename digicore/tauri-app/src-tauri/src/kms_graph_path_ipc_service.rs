//! Bounded service for graph shortest-path IPC orchestration.
//! Extracted from `kms_graph_ipc` to keep inbound adapter modules focused.

use std::path::PathBuf;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_service;

use super::ApiImpl;
use super::ipc_error;
use super::{KmsEdgeDto, KmsGraphPathDto};

pub(crate) fn get_graph_shortest_path_dto(
    host: ApiImpl,
    from_path: String,
    to_path: String,
    request_id: String,
) -> Result<KmsGraphPathDto, String> {
    let from_buf = PathBuf::from(&from_path);
    let to_buf = PathBuf::from(&to_path);
    let rel_from = host
        .get_relative_path(&from_buf)
        .map_err(|e| ipc_error("KMS_PATH_OUTSIDE_VAULT", e, None))?;
    let rel_to = host
        .get_relative_path(&to_buf)
        .map_err(|e| ipc_error("KMS_PATH_OUTSIDE_VAULT", e, None))?;
    let links = crate::kms_link_adjacency_cache::get_all_links_cached()
        .map_err(|e| ipc_error("KMS_REPO_LINKS", e.to_string(), None))?;
    let max_visited_nodes = if links.len() > 120_000 {
        Some(20_000usize)
    } else if links.len() > 40_000 {
        Some(40_000usize)
    } else {
        None
    };
    let path_result = kms_graph_service::shortest_path_undirected_wiki_with_budget(
        &links,
        &rel_from,
        &rel_to,
        max_visited_nodes,
    );
    if path_result.budget_exhausted {
        log::warn!(
            "[KMS][Graph] event_code=KMS_SHORTEST_PATH_BUDGET_WARN request_id={} rel_from={} rel_to={} visited_nodes={} edge_count={} max_visited_nodes={}",
            request_id,
            rel_from,
            rel_to,
            path_result.visited_nodes,
            links.len(),
            max_visited_nodes.unwrap_or(0)
        );
    }
    let Some(chain_rel) = path_result.chain_rel else {
        log::info!(
            "[KMS][Graph] shortest_path request_id={} found=false hops=0",
            request_id
        );
        let message = if path_result.budget_exhausted {
            Some(
                "Path search budget was reached for this large vault. Narrow graph scope and retry."
                    .to_string(),
            )
        } else {
            Some("No path exists between these notes along wiki links.".to_string())
        };
        KmsDiagnosticService::debug(
            &format!(
                "[KMS][Graph] shortest_path request_id={} no path rel_from={} rel_to={} visited_nodes={} budget_exhausted={}",
                request_id, rel_from, rel_to, path_result.visited_nodes, path_result.budget_exhausted
            ),
            None,
        );
        return Ok(KmsGraphPathDto {
            found: false,
            node_paths: Vec::new(),
            edges: Vec::new(),
            message,
            request_id,
        });
    };
    let node_paths: Vec<String> = chain_rel
        .iter()
        .map(|r| host.resolve_absolute_path(r).to_string_lossy().to_string())
        .collect();
    let mut edges = Vec::new();
    for w in node_paths.windows(2) {
        edges.push(KmsEdgeDto {
            source: w[0].clone(),
            target: w[1].clone(),
            kind: "wiki".to_string(),
            edge_recency: None,
        });
    }
    log::info!(
        "[KMS][Graph] shortest_path request_id={} found=true hops={}",
        request_id,
        node_paths.len()
    );
    KmsDiagnosticService::debug(
        &format!(
            "[KMS][Graph] shortest_path request_id={} hops={}",
            request_id,
            node_paths.len()
        ),
        None,
    );
    Ok(KmsGraphPathDto {
        found: true,
        node_paths,
        edges,
        message: None,
        request_id,
    })
}

