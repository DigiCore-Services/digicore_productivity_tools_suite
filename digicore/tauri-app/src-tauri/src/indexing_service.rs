use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::Serialize;
use crate::kms_repository;

/// Trait for a component that can provide items for the semantic index.
/// Follows hexagonal architecture by abstracting the source of data.
#[async_trait]
pub trait SemanticIndexProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "notes", "snippets", "clipboard").
    fn provider_id(&self) -> &str;

    /// Reindexes all items from this provider.
    /// Returns the number of items successfully indexed.
    async fn index_all(&self, app: &AppHandle) -> Result<usize, String>;

    /// Reindexes a specific item from this provider.
    #[allow(dead_code)]
    async fn index_item(&self, app: &AppHandle, entity_id: &str) -> Result<(), String>;
}

/// Orchestrator for KMS indexing tasks.
/// Manages a collection of providers and provides a unified interface for reindexing.
pub struct KmsIndexingService {
    providers: DashMap<String, Arc<dyn SemanticIndexProvider>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexAllProvidersReport {
    pub indexed_total: usize,
    pub provider_failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KmsReindexCompleteEvent {
    pub request_id: String,
    pub indexed_total: usize,
    pub provider_failures: Vec<String>,
    pub succeeded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct KmsReindexProviderProgressEvent {
    pub request_id: String,
    pub provider_id: String,
    pub phase: String, // start | progress | end
    pub started_at_ms: u64,
    pub emitted_at_ms: u64,
    pub elapsed_ms: u64,
    pub eta_remaining_ms: Option<u64>,
    pub provider_index: usize,
    pub provider_total: usize,
    pub provider_indexed_count: usize,
    pub indexed_total_so_far: usize,
    pub succeeded: Option<bool>,
    pub error: Option<String>,
}

impl KmsIndexingService {
    fn now_unix_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn eta_remaining_ms(elapsed_ms: u64, completed_providers: usize, provider_total: usize) -> Option<u64> {
        if completed_providers == 0 || completed_providers >= provider_total {
            return None;
        }
        let avg_per_provider = elapsed_ms / completed_providers as u64;
        Some(avg_per_provider.saturating_mul((provider_total - completed_providers) as u64))
    }

    pub fn new() -> Self {
        Self {
            providers: DashMap::new(),
        }
    }

    /// Registers a new provider.
    pub fn register_provider(&self, provider: Arc<dyn SemanticIndexProvider>) {
        self.providers.insert(provider.provider_id().to_string(), provider);
    }

    /// Triggers indexing for all registered providers.
    #[allow(dead_code)]
    pub async fn index_all_providers(&self, app: &AppHandle) -> Result<usize, String> {
        let report = self.index_all_providers_detailed(app, None).await;
        if report.provider_failures.is_empty() {
            Ok(report.indexed_total)
        } else {
            Err(format!(
                "Indexed {} items with {} provider failure(s): {}",
                report.indexed_total,
                report.provider_failures.len(),
                report.provider_failures.join("; ")
            ))
        }
    }

    /// Triggers indexing for all registered providers and returns a detailed report.
    pub async fn index_all_providers_detailed(
        &self,
        app: &AppHandle,
        request_id: Option<&str>,
    ) -> IndexAllProvidersReport {
        let started_at_ms = Self::now_unix_ms();
        let mut total = 0;
        let mut failed: Vec<String> = Vec::new();
        
        // Collect providers first to avoid holding DashMap lock across awaits
        let providers: Vec<_> = self.providers.iter().map(|entry| entry.value().clone()).collect();
        let provider_total = providers.len();
        
        for (idx, provider) in providers.into_iter().enumerate() {
            let provider_id = provider.provider_id().to_string();
            let provider_index = idx + 1;
            log::info!("[KMS][Indexing] Starting index_all for provider: {}", provider_id);
            if let Some(rid) = request_id {
                let emitted_at_ms = Self::now_unix_ms();
                let elapsed_ms = emitted_at_ms.saturating_sub(started_at_ms);
                let _ = app.emit(
                    "kms-reindex-provider-progress",
                    KmsReindexProviderProgressEvent {
                        request_id: rid.to_string(),
                        provider_id: provider_id.clone(),
                        phase: "start".to_string(),
                        started_at_ms,
                        emitted_at_ms,
                        elapsed_ms,
                        eta_remaining_ms: Self::eta_remaining_ms(elapsed_ms, provider_index.saturating_sub(1), provider_total),
                        provider_index,
                        provider_total,
                        provider_indexed_count: 0,
                        indexed_total_so_far: total,
                        succeeded: None,
                        error: None,
                    },
                );
            }
            match provider.index_all(app).await {
                Ok(count) => {
                    total += count;
                    log::info!("[KMS][Indexing] Provider {} completed: {} items indexed", provider_id, count);
                    if let Some(rid) = request_id {
                        let emitted_at_ms = Self::now_unix_ms();
                        let elapsed_ms = emitted_at_ms.saturating_sub(started_at_ms);
                        let progress = KmsReindexProviderProgressEvent {
                            request_id: rid.to_string(),
                            provider_id: provider_id.clone(),
                            phase: "progress".to_string(),
                            started_at_ms,
                            emitted_at_ms,
                            elapsed_ms,
                            eta_remaining_ms: Self::eta_remaining_ms(elapsed_ms, provider_index, provider_total),
                            provider_index,
                            provider_total,
                            provider_indexed_count: count,
                            indexed_total_so_far: total,
                            succeeded: Some(true),
                            error: None,
                        };
                        let _ = app.emit("kms-reindex-provider-progress", &progress);
                        let _ = app.emit(
                            "kms-reindex-provider-progress",
                            KmsReindexProviderProgressEvent {
                                phase: "end".to_string(),
                                ..progress
                            },
                        );
                    }
                }
                Err(e) => {
                    log::error!("[KMS][Indexing] Provider {} failed: {}", provider_id, e);
                    failed.push(format!("{} ({})", provider_id, e));
                    if let Some(rid) = request_id {
                        let emitted_at_ms = Self::now_unix_ms();
                        let elapsed_ms = emitted_at_ms.saturating_sub(started_at_ms);
                        let progress = KmsReindexProviderProgressEvent {
                            request_id: rid.to_string(),
                            provider_id: provider_id.clone(),
                            phase: "progress".to_string(),
                            started_at_ms,
                            emitted_at_ms,
                            elapsed_ms,
                            eta_remaining_ms: Self::eta_remaining_ms(elapsed_ms, provider_index, provider_total),
                            provider_index,
                            provider_total,
                            provider_indexed_count: 0,
                            indexed_total_so_far: total,
                            succeeded: Some(false),
                            error: Some(e.clone()),
                        };
                        let _ = app.emit("kms-reindex-provider-progress", &progress);
                        let _ = app.emit(
                            "kms-reindex-provider-progress",
                            KmsReindexProviderProgressEvent {
                                phase: "end".to_string(),
                                ..progress
                            },
                        );
                    }
                }
            }
        }
        IndexAllProvidersReport {
            indexed_total: total,
            provider_failures: failed,
        }
    }

    /// Spawns background indexing across all registered providers and logs completion
    /// with a request correlation ID.
    pub fn spawn_index_all_providers(
        self: Arc<Self>,
        app: AppHandle,
        request_id: String,
    ) {
        let _ = app.emit("kms-sync-status", "Indexing...");
        tokio::spawn(async move {
            let report = self.index_all_providers_detailed(&app, Some(&request_id)).await;
            let complete_payload = KmsReindexCompleteEvent {
                request_id: request_id.clone(),
                indexed_total: report.indexed_total,
                provider_failures: report.provider_failures.clone(),
                succeeded: report.provider_failures.is_empty(),
            };
            if report.provider_failures.is_empty() {
                log::info!(
                    "[KMS] Global reindexing completed: {} items indexed. request_id={}",
                    report.indexed_total,
                    request_id
                );
            } else {
                log::warn!(
                    "[KMS] Global reindexing completed with provider failures ({}): request_id={} failures={}",
                    report.provider_failures.len(),
                    request_id,
                    report.provider_failures.join("; ")
                );
            }
            let _ = app.emit("kms-reindex-complete", &complete_payload);
            let _ = app.emit("kms-sync-status", "Idle");
            let _ = app.emit("kms-sync-complete", ());
        });
    }
    
    #[allow(dead_code)]
    pub fn spawn_index_provider(
        self: Arc<Self>,
        app: AppHandle,
        request_id: String,
        provider_id: String,
    ) {
        let _ = app.emit("kms-sync-status", "Indexing...");
        tokio::spawn(async move {
            let result = self.index_provider_by_id(&app, &provider_id).await;
            match result {
                Ok(count) => {
                    log::info!(
                        "[KMS] Provider reindex completed: provider={} indexed={} request_id={}",
                        provider_id,
                        count,
                        request_id
                    );
                }
                Err(err) => {
                    log::warn!(
                        "[KMS] Provider reindex failed: provider={} request_id={} err={}",
                        provider_id,
                        request_id,
                        err,
                    );
                }
            }
            let _ = app.emit("kms-sync-status", "Idle");
            let _ = app.emit("kms-sync-complete", ());
        });
    }

    /// Triggers indexing for a specific provider by ID.
    pub async fn index_provider_by_id(&self, app: &AppHandle, provider_id: &str) -> Result<usize, String> {
        if let Some(provider) = self.providers.get(provider_id) {
            provider.index_all(app).await
        } else {
            Err(format!("Provider not found: {}", provider_id))
        }
    }

    /// Triggers indexing for a specific item from a specific provider.
    pub async fn index_single_item(&self, app: &AppHandle, provider_id: &str, entity_id: &str) -> Result<(), String> {
        if let Some(provider) = self.providers.get(provider_id) {
            provider.index_item(app, entity_id).await
        } else {
            Err(format!("Provider not found: {}", provider_id))
        }
    }

    /// Returns the list of registered provider IDs.
    #[allow(dead_code)]
    pub fn get_provider_ids(&self) -> Vec<String> {
        self.providers.iter().map(|e| e.key().clone()).collect()
    }
}

// --- Note Provider ---

pub struct NoteIndexProvider;

#[async_trait]
impl SemanticIndexProvider for NoteIndexProvider {
    fn provider_id(&self) -> &str {
        "notes"
    }

    async fn index_all(&self, app: &AppHandle) -> Result<usize, String> {
        use std::sync::{Arc, Mutex};
        let state_handle = app.state::<Arc<Mutex<digicore_text_expander::application::app_state::AppState>>>();
        let vault_path_str = {
            let state = state_handle.lock().map_err(|e| e.to_string())?;
            state.kms_vault_path.clone()
        };
        let vault_path = std::path::Path::new(&vault_path_str);
        
        if !vault_path.exists() {
            return Err("Vault path does not exist".to_string());
        }

        crate::kms_sync_orchestration::sync_vault_files_to_db_internal(app, vault_path).await?;
        
        // Count how many we have indexed
        let notes = kms_repository::list_notes().map_err(|e| e.to_string())?;
        Ok(notes.len())
    }

    async fn index_item(&self, app: &AppHandle, entity_id: &str) -> Result<(), String> {
        let vault_path = kms_repository::get_vault_path().map_err(|e| e.to_string())?;
        let abs_path = std::path::Path::new(&vault_path).join(entity_id);
        
        if !abs_path.exists() {
            return Err(format!("Note path does not exist: {}", entity_id));
        }

        crate::kms_sync_orchestration::sync_single_note_to_db_internal(app, &abs_path).await
    }
}

// --- Snippet Provider ---

pub struct SnippetIndexProvider;

#[async_trait]
impl SemanticIndexProvider for SnippetIndexProvider {
    fn provider_id(&self) -> &str {
        "snippets"
    }

    async fn index_all(&self, app: &AppHandle) -> Result<usize, String> {
        use crate::embedding_service;
        use std::sync::{Arc, Mutex};
        
        let state_handle = app.state::<Arc<Mutex<digicore_text_expander::application::app_state::AppState>>>();
        let library = {
            let state = state_handle.lock().map_err(|e| e.to_string())?;
            state.library.clone()
        };
        
        let mut count = 0;
        for (category, snippets) in library.iter() {
            for (idx, snippet) in snippets.iter().enumerate() {
                let content = format!("Category: {} | Trigger: {} | Content: {}", category, snippet.trigger, snippet.content);
                match embedding_service::generate_text_embedding(&content, None) {
                    Ok(vector) => {
                        let metadata = serde_json::json!({
                            "content": snippet.content,
                            "category": category,
                            "snippetIdx": idx
                        }).to_string();
                        kms_repository::upsert_embedding("text", "snippet", &snippet.trigger, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                        kms_repository::upsert_unified_fts("snippet", &snippet.trigger, &format!("{} | {}", category, snippet.trigger), &snippet.content).map_err(|e| e.to_string())?;
                        kms_repository::update_index_status("snippets", &snippet.trigger, "indexed", None).map_err(|e| e.to_string())?;
                        count += 1;
                    }
                    Err(e) => {
                        let _ = kms_repository::update_index_status("snippets", &snippet.trigger, "failed", Some(&e.to_string()));
                    }
                }
            }
        }
        Ok(count)
    }

    async fn index_item(&self, app: &AppHandle, entity_id: &str) -> Result<(), String> {
        use crate::embedding_service;
        use std::sync::{Arc, Mutex};
        
        let state_handle = app.state::<Arc<Mutex<digicore_text_expander::application::app_state::AppState>>>();
        let library = {
            let state = state_handle.lock().map_err(|e| e.to_string())?;
            state.library.clone()
        };
        
        // Find snippet by trigger (entity_id)
        for (category, snippets) in library.iter() {
            if let Some((idx, snippet)) = snippets.iter().enumerate().find(|(_, s)| s.trigger == entity_id) {
                let content = format!("Category: {} | Trigger: {} | Content: {}", category, snippet.trigger, snippet.content);
                match embedding_service::generate_text_embedding(&content, None) {
                    Ok(vector) => {
                        let metadata = serde_json::json!({
                            "content": snippet.content,
                            "category": category,
                            "snippetIdx": idx
                        }).to_string();
                        kms_repository::upsert_embedding("text", "snippet", &snippet.trigger, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                        kms_repository::upsert_unified_fts("snippet", &snippet.trigger, &format!("{} | {}", category, snippet.trigger), &snippet.content).map_err(|e| e.to_string())?;
                        kms_repository::update_index_status("snippets", &snippet.trigger, "indexed", None).map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                    Err(e) => {
                        let _ = kms_repository::update_index_status("snippets", &snippet.trigger, "failed", Some(&e.to_string()));
                        return Err(e.to_string());
                    }
                }
            }
        }
        Err(format!("Snippet not found: {}", entity_id))
    }
}

// --- Clipboard Provider ---

pub struct ClipboardIndexProvider;

#[async_trait]
impl SemanticIndexProvider for ClipboardIndexProvider {
    fn provider_id(&self) -> &str {
        "clipboard"
    }

    async fn index_all(&self, _app: &AppHandle) -> Result<usize, String> {
        use crate::clipboard_repository;
        use crate::embedding_service;
        
        let mut count = 0;
        // Get last 500 entries for indexing (configurable later)
        let entries = clipboard_repository::list_entries(None, 500).map_err(|e| e.to_string())?;

        for entry in entries {
            let entity_id = entry.id.to_string();
            let mut succeeded = false;
            let mut last_err = None;

            // 1. Handle Text modality
            if !entry.content.is_empty() {
                match embedding_service::generate_text_embedding(&entry.content, None) {
                    Ok(vector) => {
                        let metadata = serde_json::json!({
                            "id": entry.id,
                            "content": entry.content,
                            "process_name": entry.process_name,
                            "window_title": entry.window_title,
                            "entry_type": "text"
                        }).to_string();
                        kms_repository::upsert_embedding("text", "clipboard", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                        kms_repository::upsert_unified_fts("clipboard", &entity_id, &format!("{} | {}", entry.process_name, entry.window_title), &entry.content).map_err(|e| e.to_string())?;
                        succeeded = true;
                    }
                    Err(e) => last_err = Some(e.to_string()),
                }
            }

            // 2. Handle Image modality if it's an image
            if let Some(img_path) = entry.image_path {
                let abs_path = clipboard_repository::assets_root_dir().join(&img_path);
                if abs_path.exists() {
                    match embedding_service::generate_image_embedding(&abs_path) {
                        Ok(vector) => {
                            let metadata = serde_json::json!({
                                "id": entry.id,
                                "content": format!("CLIP Image: {} | {}", entry.process_name, entry.window_title),
                                "process_name": entry.process_name,
                                "window_title": entry.window_title,
                                "entry_type": "image",
                                "image_path": img_path
                            }).to_string();
                            kms_repository::upsert_embedding("image", "clipboard", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                            succeeded = true;
                        }
                        Err(e) => last_err = Some(e.to_string()),
                    }
                }
            }

            if succeeded {
                let _ = kms_repository::update_index_status("clipboard", &entity_id, "indexed", None);
                count += 1;
            } else if let Some(err) = last_err {
                let _ = kms_repository::update_index_status("clipboard", &entity_id, "failed", Some(&err));
            }
        }
        Ok(count)
    }

    async fn index_item(&self, _app: &AppHandle, entity_id: &str) -> Result<(), String> {
        use crate::clipboard_repository;
        use crate::embedding_service;

        let id: u32 = entity_id.parse().map_err(|_| "Invalid clipboard ID".to_string())?;
        let entry_opt = clipboard_repository::get_entry_by_id(id).map_err(|e| e.to_string())?;
        let entry = entry_opt.ok_or_else(|| format!("Clipboard entry not found: {}", id))?;
        
        let mut succeeded = false;
        let mut last_err = None;

        if !entry.content.is_empty() {
            match embedding_service::generate_text_embedding(&entry.content, None) {
                Ok(vector) => {
                    let metadata = serde_json::json!({
                        "id": entry.id,
                        "content": entry.content,
                        "process_name": entry.process_name,
                        "window_title": entry.window_title,
                        "entry_type": entry.entry_type
                    }).to_string();
                    kms_repository::upsert_embedding("text", "clipboard", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                    kms_repository::upsert_unified_fts("clipboard", &entity_id, &format!("{} | {}", entry.process_name, entry.window_title), &entry.content).map_err(|e| e.to_string())?;
                    succeeded = true;
                }
                Err(e) => last_err = Some(e.to_string()),
            }
        }

        if let Some(img_path) = entry.image_path {
            let abs_path = clipboard_repository::assets_root_dir().join(&img_path);
            if abs_path.exists() {
                match embedding_service::generate_image_embedding(&abs_path) {
                    Ok(vector) => {
                        let metadata = serde_json::json!({
                            "id": entry.id,
                            "content": format!("CLIP Image: {} | {}", entry.process_name, entry.window_title),
                            "process_name": entry.process_name,
                            "window_title": entry.window_title,
                            "entry_type": "image",
                            "image_path": img_path
                        }).to_string();
                        kms_repository::upsert_embedding("image", "clipboard", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                        succeeded = true;
                    }
                    Err(e) => last_err = Some(e.to_string()),
                }
            }
        }

        if succeeded {
            let _ = kms_repository::update_index_status("clipboard", &entity_id, "indexed", None);
            Ok(())
        } else {
            let err = last_err.unwrap_or_else(|| "No processable content found".to_string());
            let _ = kms_repository::update_index_status("clipboard", &entity_id, "failed", Some(&err));
            Err(err)
        }
    }
}

// --- Skill Provider ---

pub struct SkillIndexProvider;

#[async_trait]
impl SemanticIndexProvider for SkillIndexProvider {
    fn provider_id(&self) -> &str {
        "skills"
    }

    async fn index_all(&self, _app: &AppHandle) -> Result<usize, String> {
        use crate::embedding_service;
        use digicore_text_expander::ports::skill::SkillRepository;
        
        let repo = kms_repository::KmsSkillRepository;
        let _ = repo.refresh().await.map_err(|e| e.to_string())?;
        let skills = repo.list_skills().await.map_err(|e| e.to_string())?;
        
        let mut count = 0;
        for skill in skills {
            let entity_id = skill.metadata.name.clone();
            // Combining Name, Description and Instructions for semantic context
            let content = format!("Skill: {} | Description: {} | Instructions: {}", 
                skill.metadata.name, 
                skill.metadata.description, 
                skill.instructions
            );
            
            match embedding_service::generate_text_embedding(&content, None) {
                Ok(vector) => {
                    let metadata = serde_json::json!({
                        "name": skill.metadata.name,
                        "description": skill.metadata.description,
                        "version": skill.metadata.version,
                        "path": skill.path.to_string_lossy()
                    }).to_string();
                    
                    kms_repository::upsert_embedding("text", "skill", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                    // Already handled by triggers for FTS, but we ensure consistency
                    let _ = kms_repository::update_index_status("skills", &entity_id, "indexed", None);
                    count += 1;
                }
                Err(e) => {
                    let _ = kms_repository::update_index_status("skills", &entity_id, "failed", Some(&e.to_string()));
                }
            }
        }
        Ok(count)
    }

    async fn index_item(&self, _app: &AppHandle, entity_id: &str) -> Result<(), String> {
        use crate::embedding_service;
        use digicore_text_expander::ports::skill::SkillRepository;
        
        let repo = kms_repository::KmsSkillRepository;
        let skill_opt = repo.get_skill(entity_id).await.map_err(|e| e.to_string())?;
        let skill = skill_opt.ok_or_else(|| format!("Skill not found: {}", entity_id))?;
        
        let content = format!("Skill: {} | Description: {} | Instructions: {}", 
            skill.metadata.name, 
            skill.metadata.description, 
            skill.instructions
        );
        
        match embedding_service::generate_text_embedding(&content, None) {
            Ok(vector) => {
                let metadata = serde_json::json!({
                    "name": skill.metadata.name,
                    "description": skill.metadata.description,
                    "version": skill.metadata.version,
                    "path": skill.path.to_string_lossy()
                }).to_string();
                
                kms_repository::upsert_embedding("text", "skill", &entity_id, &vector, Some(metadata)).map_err(|e| e.to_string())?;
                let _ = kms_repository::update_index_status("skills", &entity_id, "indexed", None);
                Ok(())
            }
            Err(e) => {
                let _ = kms_repository::update_index_status("skills", &entity_id, "failed", Some(&e.to_string()));
                Err(e.to_string())
            }
        }
    }
}
