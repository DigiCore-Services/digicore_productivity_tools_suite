//! O3: embedding generator/store ports and chunk policy (no fastembed / SQLite here).

/// Character-based chunking for long notes/queries. When disabled or text fits in one chunk, a single embed call is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KmsTextEmbeddingChunkConfig {
    pub enabled: bool,
    pub max_chars: u32,
    pub overlap_chars: u32,
}

impl Default for KmsTextEmbeddingChunkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_chars: 2048,
            overlap_chars: 128,
        }
    }
}

impl KmsTextEmbeddingChunkConfig {
    pub fn clamped(self) -> Self {
        let max_chars = self.max_chars.clamp(256, 8192);
        let max_overlap = max_chars / 2;
        let overlap_chars = self.overlap_chars.min(max_overlap);
        Self {
            enabled: self.enabled,
            max_chars,
            overlap_chars,
        }
    }
}

/// Generates a text embedding in the KMS note vector space.
pub trait EmbeddingGeneratorPort: Send + Sync {
    fn embed_text_note(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// Persists vectors for a KMS text note (`entity_id` = vault-relative path).
pub trait EmbeddingStorePort: Send + Sync {
    fn upsert_text_note(&self, entity_id: &str, vector: &[f32]) -> Result<(), String>;
}
