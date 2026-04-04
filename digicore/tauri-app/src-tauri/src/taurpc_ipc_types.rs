//! TauRPC-generated IPC DTOs shared by api procedures and resolvers.

#[taurpc::ipc_type]
pub struct RichTextDto {


    pub plain: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
}

#[taurpc::ipc_type]
pub struct SkillDto {
    pub metadata: SkillMetadataDto,
    pub path: Option<String>,
    pub instructions: Option<String>,
    pub resources: Vec<SkillResourceDto>,
}

#[taurpc::ipc_type]
pub struct SkillResourceDto {
    pub name: String,
    pub r#type: String, // "Script" | "Template" | "Reference" | "Other"
    pub rel_path: String,
}

#[taurpc::ipc_type]
pub struct SkillMetadataDto {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<String>, // JSON string for arbitrary KV
    pub disable_model_invocation: Option<bool>,
    pub scope: String, // "Global" | "Project"
    pub sync_targets: Vec<String>,
}

#[taurpc::ipc_type]
pub struct SyncConflictDto {
    pub target: String,
    pub existing_name: String,
    pub conflict_type: String, // "NameCollision" | "ContentMismatch"
}

#[taurpc::ipc_type]
pub struct KmsNoteDto {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub preview: Option<String>,
    pub last_modified: Option<String>,
    pub is_favorite: bool,
    pub sync_status: String,
    pub node_type: String,     // "note" | "skill" | "image" | "asset"
    pub folder_path: String,   // For grouping
    pub embedding_model_id: Option<String>,
    /// Tags from YAML frontmatter (indexed at sync time).
    pub tags: Vec<String>,
}

#[taurpc::ipc_type]
pub struct KmsLogDto {
    pub id: i32,
    pub level: String,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: String,
}

#[taurpc::ipc_type]
pub struct KmsFileSystemItemDto {
    pub name: String,
    pub path: String,
    pub rel_path: String,
    pub item_type: String, // "file" | "directory"
    pub children: Option<Vec<KmsFileSystemItemDto>>,
    pub note: Option<KmsNoteDto>,
}

#[taurpc::ipc_type]
pub struct KmsLinksDto {
    pub outgoing: Vec<KmsNoteDto>,
    pub incoming: Vec<KmsNoteDto>,
}

#[taurpc::ipc_type]
pub struct KmsNodeDto {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub node_type: String,     // "note" | "skill" | "image" | "asset"
    pub last_modified: String, // ISO 8601 or display date
    pub folder_path: String,   // For clustering
    pub cluster_id: Option<i32>,
    /// Wiki-link PageRank-style centrality, 0..1 (server-normalized).
    #[serde(default)]
    pub link_centrality: f32,
}

fn default_kms_edge_kind_wiki() -> String {
    "wiki".to_string()
}

#[taurpc::ipc_type]
pub struct KmsEdgeDto {
    pub source: String,
    pub target: String,
    #[serde(default = "default_kms_edge_kind_wiki")]
    pub kind: String,
    #[serde(default)]
    pub edge_recency: Option<f32>,
}

#[taurpc::ipc_type]
pub struct KmsClusterLabelDto {
    pub cluster_id: i32,
    pub label: String,
}

#[taurpc::ipc_type]
pub struct KmsAiBeamDto {
    pub source_path: String,
    pub target_path: String,
    pub summary: String,
}

#[taurpc::ipc_type]
pub struct KmsGraphPaginationDto {
    pub total_nodes: u32,
    pub offset: u32,
    pub limit: u32,
    pub returned_nodes: u32,
    pub has_more: bool,
}

#[taurpc::ipc_type]
pub struct KmsGraphDto {
    pub nodes: Vec<KmsNodeDto>,
    pub edges: Vec<KmsEdgeDto>,
    pub cluster_labels: Vec<KmsClusterLabelDto>,
    pub ai_beams: Vec<KmsAiBeamDto>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub pagination: Option<KmsGraphPaginationDto>,
    /// Milliseconds spent building this graph payload on the server (last fetch).
    #[serde(default)]
    pub build_time_ms: u32,
    /// Correlates logs and client retries with one graph RPC response.
    #[serde(default)]
    pub request_id: String,
}

#[taurpc::ipc_type]
pub struct KmsGraphPathDto {
    pub found: bool,
    /// Absolute paths from `from_path` to `to_path`, inclusive.
    pub node_paths: Vec<String>,
    /// Undirected edges along the path (consecutive notes).
    pub edges: Vec<KmsEdgeDto>,
    pub message: Option<String>,
    #[serde(default)]
    pub request_id: String,
}

#[taurpc::ipc_type]
pub struct KmsNoteGraphPreviewDto {
    pub path: String,
    pub title: String,
    pub excerpt: String,
    pub last_modified: Option<String>,
    #[serde(default)]
    pub request_id: String,
}

#[taurpc::ipc_type]
pub struct ScriptLogEntryDto {
    pub timestamp: String,
    pub script_type: String,
    pub message: String,
    pub duration_ms: f64,
    pub code_len: u32,
    pub is_error: bool,
}

#[taurpc::ipc_type]
pub struct SearchResultDto {
    pub entity_type: String, // 'note', 'snippet', 'clip'
    pub entity_id: String,
    pub distance: f32,
    pub modality: String, // 'text', 'image'
    pub metadata: Option<String>,
    pub snippet: Option<String>,
    /// Wall time to embed the query for this search (ms); same on every row when present.
    pub kms_query_embedding_ms: Option<f32>,
    /// Effective KMS text embedding model id used for the query vector (normalized).
    pub kms_effective_embedding_model_id: Option<String>,
}

#[taurpc::ipc_type]
pub struct IndexingStatusDto {
    pub category: String, // "notes", "snippets", "clipboard"
    pub indexed_count: u32,
    pub failed_count: u32,
    pub total_count: u32,
    pub last_error: Option<String>,
}

#[taurpc::ipc_type]
pub struct KmsDiagnosticsDto {
    pub note_count: u32,
    pub snippet_count: u32,
    pub clip_count: u32,
    pub vector_count: u32,
    pub error_log_count: u32,
}

#[taurpc::ipc_type]
pub struct KmsEmbeddingPolicyDiagnosticsDto {
    pub indexed_note_count: u32,
    pub stale_embedding_note_count: u32,
    pub expected_policy_signature: String,
    /// All rows in `kms_notes` (every tracked vault note file).
    pub total_notes_in_index: u32,
    pub pending_note_count: u32,
    /// `sync_status = failed` (read/sync errors), not necessarily embedding-only failures.
    pub failed_sync_note_count: u32,
    /// Indexed notes whose `embedding_model_id` + `embedding_policy_sig` match current effective settings.
    pub embedding_aligned_note_count: u32,
    /// `total_notes_in_index - indexed - pending - failed` (unexpected status values, if any).
    pub other_sync_status_note_count: u32,
    /// Recursive `.md` / `.markdown` count on disk under the configured vault (matches sync scanner).
    pub vault_markdown_files_on_disk: u32,
    /// All regular files under the vault (any extension); Explorer-style total vs markdown-only index.
    pub vault_all_files_on_disk: u32,
}

#[taurpc::ipc_type]
pub struct KmsIndexStatusRow {
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}
