use std::path::{Path, PathBuf};
use crate::kms_error::{KmsError, KmsResult};
use crate::kms_link_adjacency_cache;
use crate::kms_repository;
use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_git_service::KmsGitService;
use tauri::AppHandle;
use regex::Regex;

/// Application service for KMS Knowledge Hub.
/// Orchestrates complex operations involving filesystem, database, and links.
pub struct KmsService;

impl KmsService {
    /// Resolves the absolute path for a vault-relative path.
    fn resolve_absolute_path(rel_path: &str) -> KmsResult<PathBuf> {
        let vault = kms_repository::get_vault_path()?;
        Ok(vault.join(rel_path))
    }

    /// Converts an absolute path to a vault-relative path.
    fn get_relative_path(abs_path: &Path) -> KmsResult<String> {
        let vault = kms_repository::get_vault_path()?;
        let rel = abs_path.strip_prefix(&vault)
            .map_err(|_| KmsError::Path(format!("Path {} is not within vault {}", abs_path.display(), vault.display())))?;
        Ok(rel.to_string_lossy().to_string().replace('\\', "/"))
    }

    /// Validates and renames a note, updating FS, DB, and internal links.
    pub async fn rename_note(
        _app: &AppHandle,
        old_abs_path_str: &str,
        new_name: &str,
    ) -> KmsResult<String> {
        let old_abs_path = PathBuf::from(old_abs_path_str);
        if !old_abs_path.exists() {
            return Err(KmsError::NotFound(old_abs_path_str.to_string()));
        }

        let parent = old_abs_path.parent()
            .ok_or_else(|| KmsError::Path("Cannot rename root".to_string()))?;
        
        let old_rel_path = Self::get_relative_path(&old_abs_path)?;
        
        let mut new_filename = new_name.to_string();
        if !new_filename.to_lowercase().ends_with(".md") {
            new_filename.push_str(".md");
        }
        
        let new_abs_path = parent.join(&new_filename);
        let new_rel_path = Self::get_relative_path(&new_abs_path)?;

        if new_abs_path == old_abs_path {
            return Ok(new_rel_path);
        }
        
        if new_abs_path.exists() {
            return Err(KmsError::Validation(format!("A note named '{}' already exists", new_filename)));
        }

        // 1. FS Rename
        std::fs::rename(&old_abs_path, &new_abs_path).map_err(KmsError::Io)?;

        // 2. DB Metadata update
        let title = new_abs_path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        kms_repository::rename_note(&old_rel_path, &new_rel_path, &title)?;
        let _ = kms_repository::rewrite_kms_ui_stored_paths(&old_rel_path, &new_rel_path);

        // 3. Backlink Refactoring
        let old_title = old_abs_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if !old_title.is_empty() && old_title != title {
            Self::refactor_backlinks(&old_rel_path, &old_title, &title).await?;
        }
        
        // 4. Update Link Graph paths
        kms_repository::update_links_on_path_change(&old_rel_path, &new_rel_path)?;

        kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();

        KmsDiagnosticService::info(
            &format!("Renamed note: {} -> {}", old_rel_path, new_rel_path),
            None
        );

        Ok(new_rel_path)
    }

    async fn refactor_backlinks(old_rel_path: &str, old_title: &str, new_title: &str) -> KmsResult<()> {
        let (_, backlinkers) = kms_repository::get_links_for_note(old_rel_path)?;
            
        let escaped_old = regex::escape(old_title);
        let re_pattern = format!(r"\[\[{}(?:\|([^\]]+))?\]\]", escaped_old);
        let re = Regex::new(&re_pattern).map_err(|e| KmsError::General(format!("Regex error: {}", e)))?;

        for note in backlinkers {
            let abs_path = Self::resolve_absolute_path(&note.path)?;
            if let Ok(content) = std::fs::read_to_string(&abs_path) {
                let new_content = re.replace_all(&content, |caps: &regex::Captures| {
                    if let Some(alias) = caps.get(1) {
                        format!("[[{}|{}]]", new_title, alias.as_str())
                    } else {
                        format!("[[{}]]", new_title)
                    }
                }).to_string();

                if new_content != content {
                    if let Err(err) = std::fs::write(&abs_path, new_content) {
                        KmsDiagnosticService::warn(
                            "Failed to persist backlink refactor update",
                            Some(format!("path={} err={}", note.path, err)),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Recursively renames a folder and updates all nested notes.
    pub async fn rename_folder(
        _app: &AppHandle,
        old_abs_path_str: &str,
        new_name: &str,
    ) -> KmsResult<String> {
        let old_abs_path = PathBuf::from(old_abs_path_str);
        if !old_abs_path.exists() || !old_abs_path.is_dir() {
            return Err(KmsError::NotFound(old_abs_path_str.to_string()));
        }

        let parent = old_abs_path.parent()
            .ok_or_else(|| KmsError::Path("Cannot rename root".to_string()))?;
        
        let mut new_folder_name = new_name.to_string();
        if new_folder_name.to_lowercase().ends_with(".md") {
            new_folder_name = new_folder_name[..new_folder_name.len()-3].to_string();
        }
        
        let new_abs_path = parent.join(&new_folder_name);
        if new_abs_path == old_abs_path {
            return Self::get_relative_path(&old_abs_path);
        }
        
        if new_abs_path.exists() {
            return Err(KmsError::Validation(format!("A folder named '{}' already exists", new_folder_name)));
        }

        let old_rel_path = Self::get_relative_path(&old_abs_path)?;
        let new_rel_path = Self::get_relative_path(&new_abs_path)?;

        std::fs::rename(&old_abs_path, &new_abs_path).map_err(KmsError::Io)?;
        
        kms_repository::rename_folder(&old_rel_path, &new_rel_path)?;
        let _ = kms_repository::rewrite_kms_ui_stored_paths(&old_rel_path, &new_rel_path);

        kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();
        
        KmsDiagnosticService::info(
            &format!("Renamed folder: {} -> {}", old_rel_path, new_rel_path),
            None
        );

        Ok(new_rel_path)
    }

    pub async fn move_item(
        _app: &AppHandle,
        path_str: &str,
        new_parent_path_str: &str,
    ) -> KmsResult<String> {
        let item_abs_path = PathBuf::from(path_str);
        let target_parent_abs = PathBuf::from(new_parent_path_str);
        
        if !item_abs_path.exists() {
            return Err(KmsError::NotFound(path_str.to_string()));
        }
        if !target_parent_abs.exists() || !target_parent_abs.is_dir() {
            return Err(KmsError::NotFound(new_parent_path_str.to_string()));
        }
        
        let item_name = item_abs_path.file_name()
            .ok_or_else(|| KmsError::Path("Invalid item path".to_string()))?;
        let new_abs_path = target_parent_abs.join(item_name);
        
        if new_abs_path == item_abs_path {
            return Self::get_relative_path(&item_abs_path);
        }
        
        if new_abs_path.exists() {
            return Err(KmsError::Validation("An item with the same name already exists in target".to_string()));
        }

        let old_rel_path = Self::get_relative_path(&item_abs_path)?;
        let new_rel_path = Self::get_relative_path(&new_abs_path)?;

        // If it's a file, rename manually via repository too.
        // If it's a folder, we need recursive rename.
        if item_abs_path.is_dir() {
            std::fs::rename(&item_abs_path, &new_abs_path).map_err(KmsError::Io)?;
            kms_repository::rename_folder(&old_rel_path, &new_rel_path)?;
            let _ = kms_repository::rewrite_kms_ui_stored_paths(&old_rel_path, &new_rel_path);
        } else {
            std::fs::rename(&item_abs_path, &new_abs_path).map_err(KmsError::Io)?;
            let title = new_abs_path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string());
            kms_repository::rename_note(&old_rel_path, &new_rel_path, &title)?;
            kms_repository::update_links_on_path_change(&old_rel_path, &new_rel_path)?;
            let _ = kms_repository::rewrite_kms_ui_stored_paths(&old_rel_path, &new_rel_path);
        }

        kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();

        KmsDiagnosticService::info(
            &format!("Moved item: {} -> {}", old_rel_path, new_rel_path),
            None
        );

        Ok(new_rel_path)
    }

    pub async fn save_note(
        _app: &AppHandle,
        abs_path_str: &str,
        content: &str,
    ) -> KmsResult<()> {
        let path_buf = PathBuf::from(abs_path_str);
        
        if let Some(parent) = path_buf.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(KmsError::Io)?;
            }
        }

        std::fs::write(&path_buf, content).map_err(KmsError::Io)?;
            
        let _title = path_buf
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let rel_path = Self::get_relative_path(&path_buf)?;
        
        // This part usually involves calling `kms_sync_orchestration::sync_note_index_internal`.
        // We should eventually move that to a service too.
        
        // 4. Git Auto-commit
        let _ = KmsGitService::commit_path(&rel_path, &format!("Auto-save: node update {}", rel_path));
        
        KmsDiagnosticService::debug(&format!("Saved note: {}", rel_path), None);
        Ok(())
    }

    pub async fn delete_note(
        _app: &AppHandle,
        abs_path_str: &str,
    ) -> KmsResult<()> {
        let path_buf = PathBuf::from(abs_path_str);
        let rel_path = Self::get_relative_path(&path_buf)?;

        if path_buf.exists() {
            std::fs::remove_file(&path_buf).map_err(KmsError::Io)?;
        }
        
        kms_repository::delete_note(&rel_path)?;
        let _ = kms_repository::prune_kms_ui_path_entries(&rel_path, false);

        kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();
        
        KmsDiagnosticService::info(&format!("Deleted note: {}", rel_path), None);
        Ok(())
    }

    /// Extracts a contextual snippet from content around a search query or keywords.
    pub fn extract_contextual_snippet(content: &str, query: &str) -> String {
        let content_trimmed = content.trim();
        if content_trimmed.is_empty() {
            return String::new();
        }

        let query_norm = query.to_lowercase();
        let content_norm = content_trimmed.to_lowercase();
        
        // 1. Try exact match first
        let index = content_norm.find(&query_norm);
        
        // 2. If no exact match (semantic), try matching individual keywords
        let index = index.or_else(|| {
            query_norm.split_whitespace()
                .filter(|w| w.len() > 2) // Only meaningful words
                .find_map(|word| content_norm.find(word))
        });
        
        let match_pos = index.unwrap_or(0);
        
        let start = if match_pos > 80 { match_pos - 80 } else { 0 };
        let end = std::cmp::min(content_trimmed.len(), match_pos + 120);
        
        let mut snippet = content_trimmed[start..end].to_string();
        
        if start > 0 {
            if let Some(space_idx) = snippet.find(' ') {
                if space_idx < 20 {
                    snippet = snippet[space_idx+1..].to_string();
                }
            }
            snippet.insert_str(0, "... ");
        }
        
        if end < content_trimmed.len() {
            if let Some(last_space) = snippet.rfind(' ') {
                if snippet.len() - last_space < 20 {
                    snippet.truncate(last_space);
                }
            }
            snippet.push_str(" ...");
        }
        
        snippet.replace("\r", "").replace("\n", " ").trim().to_string()
    }
}
