use std::sync::Arc;
use tauri::{AppHandle, Manager};
use async_trait::async_trait;
use dashmap::DashMap;
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

impl KmsIndexingService {
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
    pub async fn index_all_providers(&self, app: &AppHandle) -> Result<usize, String> {
        let mut total = 0;
        
        // Collect providers first to avoid holding DashMap lock across awaits
        let providers: Vec<_> = self.providers.iter().map(|entry| entry.value().clone()).collect();
        
        for provider in providers {
            log::info!("[KMS][Indexing] Starting index_all for provider: {}", provider.provider_id());
            match provider.index_all(app).await {
                Ok(count) => {
                    total += count;
                    log::info!("[KMS][Indexing] Provider {} completed: {} items indexed", provider.provider_id(), count);
                }
                Err(e) => {
                    log::error!("[KMS][Indexing] Provider {} failed: {}", provider.provider_id(), e);
                }
            }
        }
        Ok(total)
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
        use crate::api;
        // The implementation already exists in api.rs as sync_vault_files_to_db
        // We will call it here. We'll need to expose it or move it.
        // For now, let's assume we'll refactor api.rs to call this provider.
        
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

        // We'll reimplement the sync logic here or call a shared function.
        // Let's reimplement for cleaner separation.
        api::sync_vault_files_to_db_internal(app, vault_path).await?;
        
        // Count how many we have indexed
        let notes = kms_repository::list_notes()?;
        Ok(notes.len())
    }

    async fn index_item(&self, app: &AppHandle, entity_id: &str) -> Result<(), String> {
        use crate::api;
        let vault_path = kms_repository::get_vault_path()?;
        let abs_path = std::path::Path::new(&vault_path).join(entity_id);
        
        if !abs_path.exists() {
            return Err(format!("Note path does not exist: {}", entity_id));
        }

        api::sync_single_note_to_db_internal(app, &abs_path).await
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
                        kms_repository::upsert_embedding("text", "snippet", &snippet.trigger, vector, Some(metadata))?;
                        kms_repository::upsert_unified_fts("snippet", &snippet.trigger, &format!("{} | {}", category, snippet.trigger), &snippet.content)?;
                        kms_repository::update_index_status("snippets", &snippet.trigger, "indexed", None)?;
                        count += 1;
                    }
                    Err(e) => {
                        kms_repository::update_index_status("snippets", &snippet.trigger, "failed", Some(&e.to_string()))?;
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
                        kms_repository::upsert_embedding("text", "snippet", &snippet.trigger, vector, Some(metadata))?;
                        kms_repository::upsert_unified_fts("snippet", &snippet.trigger, &format!("{} | {}", category, snippet.trigger), &snippet.content)?;
                        kms_repository::update_index_status("snippets", &snippet.trigger, "indexed", None)?;
                        return Ok(());
                    }
                    Err(e) => {
                        kms_repository::update_index_status("snippets", &snippet.trigger, "failed", Some(&e.to_string()))?;
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
        let entries = clipboard_repository::list_entries(None, 500)?;

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
                        kms_repository::upsert_embedding("text", "clipboard", &entity_id, vector, Some(metadata))?;
                        kms_repository::upsert_unified_fts("clipboard", &entity_id, &format!("{} | {}", entry.process_name, entry.window_title), &entry.content)?;
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
                            kms_repository::upsert_embedding("image", "clipboard", &entity_id, vector, Some(metadata))?;
                            succeeded = true;
                        }
                        Err(e) => last_err = Some(e.to_string()),
                    }
                }
            }

            if succeeded {
                kms_repository::update_index_status("clipboard", &entity_id, "indexed", None)?;
                count += 1;
            } else if let Some(err) = last_err {
                kms_repository::update_index_status("clipboard", &entity_id, "failed", Some(&err))?;
            }
        }
        Ok(count)
    }

    async fn index_item(&self, _app: &AppHandle, entity_id: &str) -> Result<(), String> {
        use crate::clipboard_repository;
        use crate::embedding_service;

        let id: u32 = entity_id.parse().map_err(|_| "Invalid clipboard ID".to_string())?;
        let entry_opt = clipboard_repository::get_entry_by_id(id)?;
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
                    kms_repository::upsert_embedding("text", "clipboard", &entity_id, vector, Some(metadata))?;
                    kms_repository::upsert_unified_fts("clipboard", &entity_id, &format!("{} | {}", entry.process_name, entry.window_title), &entry.content)?;
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
                        kms_repository::upsert_embedding("image", "clipboard", &entity_id, vector, Some(metadata))?;
                        succeeded = true;
                    }
                    Err(e) => last_err = Some(e.to_string()),
                }
            }
        }

        if succeeded {
            kms_repository::update_index_status("clipboard", &entity_id, "indexed", None)?;
            Ok(())
        } else {
            let err = last_err.unwrap_or_else(|| "No processable content found".to_string());
            kms_repository::update_index_status("clipboard", &entity_id, "failed", Some(&err))?;
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
                    
                    kms_repository::upsert_embedding("text", "skill", &entity_id, vector, Some(metadata))?;
                    // Already handled by triggers for FTS, but we ensure consistency
                    kms_repository::update_index_status("skills", &entity_id, "indexed", None)?;
                    count += 1;
                }
                Err(e) => {
                    kms_repository::update_index_status("skills", &entity_id, "failed", Some(&e.to_string()))?;
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
                
                kms_repository::upsert_embedding("text", "skill", &entity_id, vector, Some(metadata))?;
                kms_repository::update_index_status("skills", &entity_id, "indexed", None)?;
                Ok(())
            }
            Err(e) => {
                kms_repository::update_index_status("skills", &entity_id, "failed", Some(&e.to_string()))?;
                Err(e.to_string())
            }
        }
    }
}
