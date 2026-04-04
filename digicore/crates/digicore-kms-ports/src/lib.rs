//! Hexagonal ports for KMS graph data loading (notes, wiki links, embeddings).
//! Adapters live in the Tauri crate (`digicore-text-expander-tauri`) and call `kms_repository`.

mod embedding;

pub use embedding::{EmbeddingGeneratorPort, EmbeddingStorePort, KmsTextEmbeddingChunkConfig};

use serde::{Deserialize, Serialize};

/// Minimal note row for graph assembly (mirrors SQLite `kms_notes` fields used by the graph).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNoteMinimal {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub last_modified: Option<String>,
    #[serde(default)]
    pub wiki_pagerank: Option<f32>,
}

pub type GraphLoadResult<T> = Result<T, String>;

/// Load all indexed notes (minimal columns).
pub trait LoadNotesMinimalPort: Send + Sync {
    fn all_notes_minimal(&self) -> GraphLoadResult<Vec<GraphNoteMinimal>>;
}

/// Load all wiki link pairs as vault-relative `(source, target)` paths.
pub trait LoadWikiLinksPort: Send + Sync {
    fn all_wiki_link_pairs(&self) -> GraphLoadResult<Vec<(String, String)>>;
}

/// Load text embeddings for notes (`entity_id` = vault-relative path).
pub trait LoadNoteEmbeddingsPort: Send + Sync {
    fn load_all_note_embeddings(&self) -> GraphLoadResult<Vec<(String, Vec<f32>)>>;

    /// Subset load for local neighborhood graph and other scoped consumers.
    fn load_note_embeddings_for_paths(&self, paths: &[String]) -> GraphLoadResult<Vec<(String, Vec<f32>)>>;
}
