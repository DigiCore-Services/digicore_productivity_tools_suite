//! Adapters for [`digicore_kms_ports`] traits (SQLite-backed `kms_repository` + wiki link cache).

pub use digicore_kms_ports::{
    GraphLoadResult, GraphNoteMinimal, LoadNoteEmbeddingsPort, LoadNotesMinimalPort, LoadWikiLinksPort,
};

use crate::kms_repository;

/// Production adapter: SQLite-backed note rows (minimal columns).
#[derive(Clone, Copy, Debug, Default)]
pub struct KmsRepositoryGraphAdapter;

impl LoadNotesMinimalPort for KmsRepositoryGraphAdapter {
    fn all_notes_minimal(&self) -> GraphLoadResult<Vec<GraphNoteMinimal>> {
        kms_repository::get_all_notes_minimal().map_err(|e| e.to_string())
    }
}

/// Production adapter: in-memory adjacency cache over `kms_links` (invalidated on index mutations).
#[derive(Clone, Copy, Debug, Default)]
pub struct WikiLinkAdjacencyCacheAdapter;

impl LoadWikiLinksPort for WikiLinkAdjacencyCacheAdapter {
    fn all_wiki_link_pairs(&self) -> GraphLoadResult<Vec<(String, String)>> {
        crate::kms_link_adjacency_cache::get_all_links_cached().map_err(|e| e.to_string())
    }
}

/// Production adapter: all note text embeddings (`entity_id` = vault-relative path).
#[derive(Clone, Copy, Debug, Default)]
pub struct KmsRepositoryEmbeddingsAdapter;

impl LoadNoteEmbeddingsPort for KmsRepositoryEmbeddingsAdapter {
    fn load_all_note_embeddings(&self) -> GraphLoadResult<Vec<(String, Vec<f32>)>> {
        kms_repository::get_all_note_embeddings().map_err(|e| e.to_string())
    }

    fn load_note_embeddings_for_paths(&self, paths: &[String]) -> GraphLoadResult<Vec<(String, Vec<f32>)>> {
        kms_repository::get_note_embeddings_for_paths(paths).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyNotes;
    impl LoadNotesMinimalPort for EmptyNotes {
        fn all_notes_minimal(&self) -> GraphLoadResult<Vec<GraphNoteMinimal>> {
            Ok(vec![])
        }
    }

    struct EmptyLinks;
    impl LoadWikiLinksPort for EmptyLinks {
        fn all_wiki_link_pairs(&self) -> GraphLoadResult<Vec<(String, String)>> {
            Ok(vec![])
        }
    }

    struct EmptyEmbeddings;
    impl LoadNoteEmbeddingsPort for EmptyEmbeddings {
        fn load_all_note_embeddings(&self) -> GraphLoadResult<Vec<(String, Vec<f32>)>> {
            Ok(vec![])
        }

        fn load_note_embeddings_for_paths(&self, _paths: &[String]) -> GraphLoadResult<Vec<(String, Vec<f32>)>> {
            Ok(vec![])
        }
    }

    #[test]
    fn port_mock_returns_empty_notes() {
        let p = EmptyNotes;
        assert!(p.all_notes_minimal().unwrap().is_empty());
    }

    #[test]
    fn build_full_graph_with_ports_empty_graph() {
        let vault = std::path::Path::new("V:/test-vault");
        let params = crate::kms_graph_service::KmsGraphBuildParams {
            enable_semantic_clustering: false,
            enable_leiden_communities: false,
            enable_ai_beams: false,
            k_means_max_k: 2u32,
            k_means_iterations: 1,
            ai_beam_max_nodes: 0,
            ai_beam_similarity_threshold: 0.0,
            ai_beam_max_edges: 0,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 1,
            pagerank_local_iterations: 1,
            pagerank_damping: 0.85,
            pagerank_scope: "off".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };
        let built = crate::kms_graph_service::build_full_graph_with_ports(
            vault,
            &params,
            None,
            &EmptyNotes,
            &EmptyLinks,
            &EmptyEmbeddings,
            &crate::kms_graph_service::TemporalRpcOverride::default(),
        )
        .expect("build");
        assert!(built.nodes.is_empty());
        assert!(built.edges.is_empty());
    }
}
