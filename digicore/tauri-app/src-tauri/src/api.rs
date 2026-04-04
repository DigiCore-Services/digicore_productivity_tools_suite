//! TauRPC API - type-safe IPC procedures for DigiCore Text Expander.
// Triggering bindings regeneration.

pub(crate) use crate::app_diagnostics::diag_log;
pub(crate) use crate::app_shell::{
    bring_main_to_foreground_above_ghost_follower, get_app, open_file_in_default_app,
    var_type_to_string,
};
pub(crate) use crate::fs_util::copy_dir_recursive;
pub(crate) use crate::kms_ipc_boundary::{ipc_error, kms_ipc_error, kms_request_id};

use crate::{
    clipboard_repository,
    kms_repository,
    embedding_service,
    indexing_service,
    skill_sync,
    app_state_to_dto, AppearanceTransparencyRuleDto, ConfigUpdateDto, ExpansionStatsDto, GhostFollowerStateDto,
    GhostSuggestorStateDto, ScriptingDslConfigDto, ScriptingEngineConfigDto, ScriptingHttpConfigDto,
    ScriptingDetachedSignatureExportDto, ScriptingLuaConfigDto, ScriptingProfileDiffEntryDto,
    ScriptingProfileDryRunDto, ScriptingProfileImportResultDto, ScriptingProfilePreviewDto,
    ScriptingSignerRegistryDto, CopyToClipboardConfigDto, CopyToClipboardStatsDto,
    ScriptingPyConfigDto, SettingsBundlePreviewDto, SettingsImportResultDto, SnippetLogicTestResultDto,
    UiPrefsDto,
};
use crate::kms_git_service::KmsVersion;

use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::clipboard_history;
use digicore_text_expander::application::expansion_diagnostics;
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
use digicore_text_expander::application::expansion_stats;
use digicore_text_expander::application::discovery;
use digicore_text_expander::application::ghost_follower::{
    self, ExpandTrigger, FollowerEdge, FollowerMode, MonitorAnchor,
};
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::{get_scripting_config, set_scripting_config};
use digicore_text_expander::application::template_processor;
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{
    sync_discovery_config, sync_ghost_config, update_library, GhostConfig,
};
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::ports::{storage_keys, StoragePort};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    ClipEntryDto, DiagnosticEntryDto, InteractiveVarDto, PinnedSnippetDto,
    PendingVariableInputDto as PendingVarDto,     SuggestionDto,
};
pub use crate::taurpc_ipc_types::*;

// Export to frontend src/ (outside src-tauri) to avoid watcher rebuild loop

#[taurpc::procedures(export_to = "../src/bindings_new.ts")]


pub trait Api {

    async fn greet(name: String) -> String;


    async fn get_app_state() -> Result<crate::AppStateDto, String>;

    async fn load_library() -> Result<u32, String>;
    async fn save_library() -> Result<(), String>;
    async fn set_library_path(path: String) -> Result<(), String>;
    async fn save_settings() -> Result<(), String>;
    async fn get_ui_prefs() -> Result<UiPrefsDto, String>;
    async fn save_ui_prefs(last_tab: u32, column_order: Vec<String>) -> Result<(), String>;
    async fn add_snippet(category: String, snippet: Snippet) -> Result<(), String>;
    async fn update_snippet(
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String>;
    async fn delete_snippet(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn update_config(config: ConfigUpdateDto) -> Result<(), String>;
    async fn get_clipboard_entries() -> Result<Vec<ClipEntryDto>, String>;
    async fn search_clipboard_entries(
        search: String,
        operator: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ClipEntryDto>, String>;
    async fn delete_clip_entry(index: u32) -> Result<(), String>;
    async fn delete_clip_entry_by_id(id: u32) -> Result<(), String>;
    async fn clear_clipboard_history() -> Result<(), String>;
    async fn get_clipboard_rich_text() -> Result<RichTextDto, String>;
    async fn get_image_gallery(
        search: Option<String>,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ClipEntryDto>, u32), String>;
    async fn get_child_entries(parent_id: u32) -> Result<Vec<ClipEntryDto>, String>;
    async fn get_copy_to_clipboard_config() -> Result<CopyToClipboardConfigDto, String>;
    async fn save_copy_to_clipboard_config(config: CopyToClipboardConfigDto) -> Result<(), String>;
    async fn get_copy_to_clipboard_stats() -> Result<CopyToClipboardStatsDto, String>;
    async fn copy_to_clipboard(text: String) -> Result<(), String>;
    async fn copy_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn save_clipboard_image_by_id(id: u32, path: String) -> Result<(), String>;
    async fn open_clipboard_image_by_id(id: u32) -> Result<(), String>;
    async fn get_clip_entry_by_id(id: u32) -> Result<Option<ClipEntryDto>, String>;
    async fn get_script_library_js() -> Result<String, String>;
    async fn save_script_library_js(content: String) -> Result<(), String>;
    async fn get_script_library_py() -> Result<String, String>;
    async fn save_script_library_py(content: String) -> Result<(), String>;
    async fn get_script_library_lua() -> Result<String, String>;
    async fn save_script_library_lua(content: String) -> Result<(), String>;
    async fn get_scripting_engine_config() -> Result<ScriptingEngineConfigDto, String>;
    async fn save_scripting_engine_config(config: ScriptingEngineConfigDto) -> Result<(), String>;
    async fn get_appearance_transparency_rules() -> Result<Vec<AppearanceTransparencyRuleDto>, String>;
    async fn get_running_process_names() -> Result<Vec<String>, String>;
    async fn export_settings_bundle_to_file(
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String>;
    async fn preview_settings_bundle_from_file(path: String) -> Result<SettingsBundlePreviewDto, String>;
    async fn import_settings_bundle_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String>;
    async fn export_scripting_profile_to_file(path: String, selected_groups: Vec<String>) -> Result<u32, String>;
    async fn export_scripting_profile_with_detached_signature_to_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingDetachedSignatureExportDto, String>;
    async fn preview_scripting_profile_from_file(path: String) -> Result<ScriptingProfilePreviewDto, String>;
    async fn dry_run_import_scripting_profile_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileDryRunDto, String>;
    async fn import_scripting_profile_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileImportResultDto, String>;
    async fn get_scripting_signer_registry() -> Result<ScriptingSignerRegistryDto, String>;
    async fn save_scripting_signer_registry(registry: ScriptingSignerRegistryDto) -> Result<(), String>;
    async fn save_appearance_transparency_rule(app_process: String, opacity: u32, enabled: bool) -> Result<(), String>;
    async fn delete_appearance_transparency_rule(app_process: String) -> Result<(), String>;
    async fn apply_appearance_transparency_now(app_process: String, opacity: u32) -> Result<u32, String>;
    async fn restore_appearance_defaults() -> Result<u32, String>;

    async fn get_script_logs() -> Result<Vec<ScriptLogEntryDto>, String>;
    async fn clear_script_logs() -> Result<(), String>;
    async fn get_ghost_suggestor_state() -> Result<GhostSuggestorStateDto, String>;
    async fn ghost_suggestor_accept() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_snooze() -> Result<(), String>;
    async fn ghost_suggestor_dismiss() -> Result<(), String>;
    async fn ghost_suggestor_ignore(phrase: String) -> Result<(), String>;
    async fn ghost_suggestor_create_snippet() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_cycle_forward() -> Result<u32, String>;
    async fn get_ghost_follower_state(search_filter: Option<String>)
        -> Result<GhostFollowerStateDto, String>;
    async fn ghost_follower_insert(trigger: String, content: String) -> Result<(), String>;
    async fn ghost_follower_set_search(filter: String) -> Result<(), String>;
    async fn bring_main_window_to_foreground() -> Result<(), String>;
    async fn ghost_follower_restore_always_on_top() -> Result<(), String>;
    async fn ghost_follower_capture_target_window() -> Result<(), String>;
    async fn ghost_follower_touch() -> Result<(), String>;
    async fn ghost_follower_set_collapsed(collapsed: bool) -> Result<(), String>;
    async fn ghost_follower_set_size(width: f64, height: f64) -> Result<(), String>;
    async fn ghost_follower_set_opacity(opacity_pct: u32) -> Result<(), String>;
    async fn ghost_follower_save_position(x: i32, y: i32) -> Result<(), String>;
    async fn ghost_follower_hide() -> Result<(), String>;
    async fn ghost_follower_request_view_full(content: String) -> Result<(), String>;
    async fn ghost_follower_request_edit(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn ghost_follower_request_promote(content: String, trigger: String) -> Result<(), String>;
    async fn ghost_follower_toggle_pin(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn get_pending_variable_input() -> Result<Option<PendingVarDto>, String>;
    async fn submit_variable_input(values: HashMap<String, String>) -> Result<(), String>;
    async fn cancel_variable_input() -> Result<(), String>;
    async fn get_expansion_stats() -> Result<ExpansionStatsDto, String>;
    async fn reset_expansion_stats() -> Result<(), String>;
    async fn get_diagnostic_logs() -> Result<Vec<DiagnosticEntryDto>, String>;
    async fn clear_diagnostic_logs() -> Result<(), String>;
    async fn test_snippet_logic(
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String>;
    async fn get_weather_location_suggestions(
        city_query: String,
        country: Option<String>,
        region: Option<String>,
    ) -> Result<Vec<String>, String>;
    async fn kms_launch() -> Result<(), String>;
    async fn kms_initialize() -> Result<String, String>;
    async fn kms_list_notes() -> Result<Vec<KmsNoteDto>, String>;
    async fn kms_load_note(path: String) -> Result<String, String>;
    async fn kms_save_note(path: String, content: String) -> Result<(), String>;
    async fn kms_set_note_favorite(path: String, favorite: bool) -> Result<(), String>;
    async fn kms_get_recent_note_paths() -> Result<Vec<String>, String>;
    async fn kms_set_recent_note_paths(paths: Vec<String>) -> Result<(), String>;
    async fn kms_get_favorite_path_order() -> Result<Vec<String>, String>;
    async fn kms_set_favorite_path_order(paths: Vec<String>) -> Result<(), String>;
    async fn kms_delete_note(path: String) -> Result<(), String>;
    async fn kms_rename_note(old_path: String, new_name: String) -> Result<String, String>;
    async fn kms_rename_folder(old_path: String, new_name: String) -> Result<String, String>;
    async fn kms_delete_folder(path: String) -> Result<(), String>;
    async fn kms_move_item(path: String, new_parent_path: String) -> Result<String, String>;
    async fn kms_create_folder(path: String) -> Result<(), String>;
    async fn kms_search_semantic(query: String, modality: Option<String>, limit: u32, search_mode: Option<String>) -> Result<Vec<SearchResultDto>, String>;
    async fn kms_reindex_all() -> Result<(), String>;
    async fn kms_reindex_note(path: String) -> Result<(), String>;
    async fn kms_request_note_embedding_migration() -> Result<u64, String>;
    async fn kms_cancel_note_embedding_migration() -> Result<(), String>;
    async fn kms_get_embedding_policy_diagnostics() -> Result<KmsEmbeddingPolicyDiagnosticsDto, String>;
    async fn kms_get_embedding_diagnostic_log_path() -> Result<Option<String>, String>;
    async fn kms_repair_database() -> Result<(), String>;
    async fn kms_get_vault_path() -> Result<String, String>;
    async fn kms_set_vault_path(new_path: String, migrate: bool) -> Result<(), String>;
    async fn kms_reindex_type(provider_id: String) -> Result<u32, String>;
    async fn kms_get_indexing_status() -> Result<Vec<IndexingStatusDto>, String>;
    async fn kms_get_indexing_details(provider_id: String) -> Result<Vec<KmsIndexStatusRow>, String>;
    async fn kms_retry_item(provider_id: String, entity_id: String) -> Result<(), String>;
    async fn kms_retry_failed(provider_id: String) -> Result<(), String>;
    async fn kms_get_note_links(path: String) -> Result<KmsLinksDto, String>;
    async fn kms_get_logs(limit: u32) -> Result<Vec<KmsLogDto>, String>;
    async fn kms_clear_logs() -> Result<(), String>;
    async fn kms_get_vault_structure() -> Result<KmsFileSystemItemDto, String>;
    async fn kms_list_vault_media() -> Result<Vec<String>, String>;
    async fn kms_list_unused_vault_media() -> Result<Vec<String>, String>;

    // --- KMS Versioning ---
    async fn kms_get_history(rel_path: String) -> Result<Vec<KmsVersion>, String>;
    async fn kms_get_note_revision_content(hash: String, path: String) -> Result<String, String>;
    async fn kms_restore_version(hash: String, rel_path: String) -> Result<(), String>;

    // --- Skill Hub ---
    async fn kms_list_skills() -> Result<Vec<SkillDto>, String>;
    async fn kms_get_skill(name: String) -> Result<Option<SkillDto>, String>;
    async fn kms_save_skill(skill: SkillDto, overwrite: bool) -> Result<(), String>;
    async fn kms_delete_skill(name: String) -> Result<(), String>;
    async fn kms_sync_skills() -> Result<(), String>;
    async fn kms_add_skill_resource(skill_name: String, source_path: String, target_subdir: Option<String>) -> Result<SkillResourceDto, String>;
    async fn kms_remove_skill_resource(skill_name: String, rel_path: String) -> Result<(), String>;
    async fn kms_check_skill_conflicts(skill_name: String, sync_targets: Vec<String>) -> Result<Vec<SyncConflictDto>, String>;
    
    // --- Diagnostics ---
    async fn kms_get_diagnostics() -> Result<KmsDiagnosticsDto, String>;
    // Structured JSON for support; caller supplies path from save dialog.
    async fn kms_export_graph_diagnostics(path: String) -> Result<(), String>;
    // All wiki link rows (vault-relative paths) as JSON for external graph tools.
    async fn kms_export_wiki_links_json(path: String) -> Result<(), String>;
    async fn kms_export_graph_graphml(path: String) -> Result<(), String>;
    async fn kms_export_graph_dto_json(path: String) -> Result<(), String>;
    async fn kms_prune_history() -> Result<String, String>;
    async fn kms_evaluate_placeholders(
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String>;
    async fn kms_get_graph(
        offset: u32,
        limit: u32,
        time_from_utc: Option<String>,
        time_to_utc: Option<String>,
    ) -> Result<KmsGraphDto, String>;
    async fn kms_get_local_graph(path: String, depth: u32) -> Result<KmsGraphDto, String>;
    async fn kms_get_graph_shortest_path(from_path: String, to_path: String) -> Result<KmsGraphPathDto, String>;
    async fn kms_get_note_graph_preview(path: String, max_chars: u32) -> Result<KmsNoteGraphPreviewDto, String>;
    async fn kms_get_vault_graph_overrides_json() -> Result<String, String>;
    async fn kms_set_vault_graph_overrides_json(json: String) -> Result<(), String>;
    async fn kms_clear_vault_graph_overrides_json() -> Result<(), String>;
}

#[derive(Clone)]
pub struct ApiImpl {
    pub state: Arc<Mutex<digicore_text_expander::application::app_state::AppState>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
    pub clipboard: Arc<dyn digicore_core::domain::ports::ClipboardPort>,
}

impl ApiImpl {

    fn get_vault_path(&self) -> PathBuf {
        let app = get_app(&self.app_handle);
        let app_state = app.state::<Arc<Mutex<AppState>>>();
        let state = app_state.lock().unwrap();
        
        if state.kms_vault_path.is_empty() {
             // Fallback to default documents dir
             let docs = app.path().document_dir().unwrap_or_else(|_| PathBuf::from(""));
             docs.join("DigiCore Notes")
        } else {
             PathBuf::from(&state.kms_vault_path)
        }
    }

    fn resolve_absolute_path(&self, relative_path: &str) -> PathBuf {
        self.get_vault_path().join(relative_path)
    }

    fn get_relative_path(&self, absolute_path: &Path) -> Result<String, String> {
        let vault = self.get_vault_path();
        absolute_path
            .strip_prefix(&vault)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .map_err(|_| format!("Path {} is not within the vault {}", absolute_path.display(), vault.display()))
    }
}

#[path = "kms_graph_ipc.rs"]
pub(crate) mod kms_graph_ipc;
#[path = "kms_graph_path_ipc_service.rs"]
pub(crate) mod kms_graph_path_ipc_service;
#[path = "kms_graph_local_ipc_service.rs"]
pub(crate) mod kms_graph_local_ipc_service;
#[path = "kms_graph_full_ipc_service.rs"]
pub(crate) mod kms_graph_full_ipc_service;
#[path = "kms_graph_note_preview_ipc_service.rs"]
pub(crate) mod kms_graph_note_preview_ipc_service;
#[path = "kms_graph_export_ipc_service.rs"]
pub(crate) mod kms_graph_export_ipc_service;
#[path = "kms_graph_overrides_ipc_service.rs"]
pub(crate) mod kms_graph_overrides_ipc_service;
#[path = "kms_indexing_diag_ipc_service.rs"]
pub(crate) mod kms_indexing_diag_ipc_service;
#[path = "kms_reindex_ipc_service.rs"]
pub(crate) mod kms_reindex_ipc_service;
#[path = "kms_embedding_ops_ipc_service.rs"]
pub(crate) mod kms_embedding_ops_ipc_service;
#[path = "kms_lifecycle_ipc_service.rs"]
pub(crate) mod kms_lifecycle_ipc_service;
#[path = "kms_search_ipc_service.rs"]
pub(crate) mod kms_search_ipc_service;
#[path = "kms_vault_path_ipc_service.rs"]
pub(crate) mod kms_vault_path_ipc_service;
#[path = "config_ipc_service.rs"]
pub(crate) mod config_ipc_service;
#[path = "settings_bundle_ipc_service.rs"]
pub(crate) mod settings_bundle_ipc_service;
#[path = "settings_bundle_export_ipc_service.rs"]
pub(crate) mod settings_bundle_export_ipc_service;
#[path = "settings_bundle_preview_ipc_service.rs"]
pub(crate) mod settings_bundle_preview_ipc_service;
#[path = "settings_bundle_import_ipc_service.rs"]
pub(crate) mod settings_bundle_import_ipc_service;
#[path = "ghost_appearance_ipc_service.rs"]
pub(crate) mod ghost_appearance_ipc_service;
#[path = "variable_diagnostics_ipc_service.rs"]
pub(crate) mod variable_diagnostics_ipc_service;
#[path = "runtime_helpers_ipc_service.rs"]
pub(crate) mod runtime_helpers_ipc_service;
#[path = "clipboard_media_ipc_service.rs"]
pub(crate) mod clipboard_media_ipc_service;
#[path = "clipboard_history_ipc_service.rs"]
pub(crate) mod clipboard_history_ipc_service;
#[path = "ui_prefs_ipc_service.rs"]
pub(crate) mod ui_prefs_ipc_service;
#[path = "snippet_library_ipc_service.rs"]
pub(crate) mod snippet_library_ipc_service;
#[path = "scripting_ipc_service.rs"]
pub(crate) mod scripting_ipc_service;
#[path = "scripting_profile_ipc_service.rs"]
pub(crate) mod scripting_profile_ipc_service;
#[path = "kms_notes_vault_ipc_service.rs"]
pub(crate) mod kms_notes_vault_ipc_service;
#[path = "kms_git_history_ipc_service.rs"]
pub(crate) mod kms_git_history_ipc_service;
#[path = "kms_logs_ipc_service.rs"]
pub(crate) mod kms_logs_ipc_service;
#[path = "kms_skills_ipc_service.rs"]
pub(crate) mod kms_skills_ipc_service;

#[taurpc::resolvers]
impl Api for ApiImpl {

    async fn get_script_logs(self) -> Result<Vec<ScriptLogEntryDto>, String> {
        scripting_ipc_service::get_script_logs(self).await
    }

    async fn clear_script_logs(self) -> Result<(), String> {
        scripting_ipc_service::clear_script_logs(self).await
    }

    async fn greet(self, name: String) -> String {

        format!("Hello, {}! DigiCore Text Expander backend ready.", name)
    }

    async fn get_app_state(self) -> Result<crate::AppStateDto, String> {

        let guard = self.state.lock().map_err(|e| e.to_string())?;
        Ok(app_state_to_dto(&guard))
    }

    async fn load_library(self) -> Result<u32, String> {
        snippet_library_ipc_service::load_library(self).await
    }

    async fn save_library(self) -> Result<(), String> {
        snippet_library_ipc_service::save_library(self).await
    }

    async fn set_library_path(self, path: String) -> Result<(), String> {
        snippet_library_ipc_service::set_library_path(self, path).await
    }

    async fn save_settings(self) -> Result<(), String> {
        config_ipc_service::save_settings(self).await
    }

    async fn get_ui_prefs(self) -> Result<UiPrefsDto, String> {
        ui_prefs_ipc_service::get_ui_prefs(self).await
    }

    async fn save_ui_prefs(self, last_tab: u32, column_order: Vec<String>) -> Result<(), String> {
        ui_prefs_ipc_service::save_ui_prefs(self, last_tab, column_order).await
    }

    async fn add_snippet(self, category: String, snippet: Snippet) -> Result<(), String> {
        snippet_library_ipc_service::add_snippet(self, category, snippet).await
    }

    async fn update_snippet(
        self,
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String> {
        snippet_library_ipc_service::update_snippet(self, category, snippet_idx, snippet).await
    }

    async fn delete_snippet(self, category: String, snippet_idx: u32) -> Result<(), String> {
        snippet_library_ipc_service::delete_snippet(self, category, snippet_idx).await
    }


    async fn update_config(self, config: ConfigUpdateDto) -> Result<(), String> {
        config_ipc_service::update_config(self, config).await
    }


    async fn get_clipboard_entries(self) -> Result<Vec<ClipEntryDto>, String> {
        clipboard_history_ipc_service::get_clipboard_entries(self).await
    }

    async fn search_clipboard_entries(
        self,
        search: String,
        operator: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ClipEntryDto>, String> {
        clipboard_history_ipc_service::search_clipboard_entries(self, search, operator, limit).await
    }

    async fn delete_clip_entry(self, index: u32) -> Result<(), String> {
        clipboard_history_ipc_service::delete_clip_entry(self, index).await
    }

    async fn delete_clip_entry_by_id(self, id: u32) -> Result<(), String> {
        clipboard_history_ipc_service::delete_clip_entry_by_id(self, id).await
    }

    async fn clear_clipboard_history(self) -> Result<(), String> {
        clipboard_history_ipc_service::clear_clipboard_history(self).await
    }

    async fn get_clipboard_rich_text(self) -> Result<RichTextDto, String> {
        clipboard_media_ipc_service::get_clipboard_rich_text(self).await
    }

    async fn get_image_gallery(
        self,
        search: Option<String>,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<ClipEntryDto>, u32), String> {
        clipboard_media_ipc_service::get_image_gallery(self, search, page, page_size).await
    }

    async fn get_child_entries(self, parent_id: u32) -> Result<Vec<ClipEntryDto>, String> {
        clipboard_media_ipc_service::get_child_entries(self, parent_id).await
    }

    async fn get_copy_to_clipboard_config(self) -> Result<CopyToClipboardConfigDto, String> {
        clipboard_history_ipc_service::get_copy_to_clipboard_config(self).await
    }

    async fn save_copy_to_clipboard_config(self, config: CopyToClipboardConfigDto) -> Result<(), String> {
        clipboard_history_ipc_service::save_copy_to_clipboard_config(self, config).await
    }

    async fn get_copy_to_clipboard_stats(self) -> Result<CopyToClipboardStatsDto, String> {
        clipboard_history_ipc_service::get_copy_to_clipboard_stats(self).await
    }

    async fn copy_to_clipboard(self, text: String) -> Result<(), String> {
        clipboard_media_ipc_service::copy_to_clipboard(self, text).await
    }

    async fn copy_clipboard_image_by_id(self, id: u32) -> Result<(), String> {
        clipboard_media_ipc_service::copy_clipboard_image_by_id(self, id).await
    }

    async fn save_clipboard_image_by_id(self, id: u32, path: String) -> Result<(), String> {
        clipboard_media_ipc_service::save_clipboard_image_by_id(self, id, path).await
    }

    async fn open_clipboard_image_by_id(self, id: u32) -> Result<(), String> {
        clipboard_media_ipc_service::open_clipboard_image_by_id(self, id).await
    }

    async fn get_clip_entry_by_id(self, id: u32) -> Result<Option<ClipEntryDto>, String> {
        clipboard_media_ipc_service::get_clip_entry_by_id(self, id).await
    }

    async fn get_script_library_js(self) -> Result<String, String> {
        scripting_ipc_service::get_script_library_js(self).await
    }

    async fn save_script_library_js(self, content: String) -> Result<(), String> {
        scripting_ipc_service::save_script_library_js(self, content).await
    }

    async fn get_script_library_py(self) -> Result<String, String> {
        scripting_ipc_service::get_script_library_py(self).await
    }

    async fn save_script_library_py(self, content: String) -> Result<(), String> {
        scripting_ipc_service::save_script_library_py(self, content).await
    }

    async fn get_script_library_lua(self) -> Result<String, String> {
        scripting_ipc_service::get_script_library_lua(self).await
    }

    async fn save_script_library_lua(self, content: String) -> Result<(), String> {
        scripting_ipc_service::save_script_library_lua(self, content).await
    }

    async fn get_scripting_engine_config(self) -> Result<ScriptingEngineConfigDto, String> {
        scripting_ipc_service::get_scripting_engine_config(self).await
    }

    async fn save_scripting_engine_config(self, config: ScriptingEngineConfigDto) -> Result<(), String> {
        scripting_ipc_service::save_scripting_engine_config(self, config).await
    }

    async fn export_scripting_profile_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<u32, String> {
        scripting_profile_ipc_service::export_scripting_profile_to_file(self, path, selected_groups).await
    }

    async fn export_scripting_profile_with_detached_signature_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingDetachedSignatureExportDto, String> {
        scripting_profile_ipc_service::export_scripting_profile_with_detached_signature_to_file(
            self,
            path,
            selected_groups,
        )
        .await
    }

    async fn preview_scripting_profile_from_file(
        self,
        path: String,
    ) -> Result<ScriptingProfilePreviewDto, String> {
        scripting_profile_ipc_service::preview_scripting_profile_from_file(self, path).await
    }

    async fn dry_run_import_scripting_profile_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileDryRunDto, String> {
        scripting_profile_ipc_service::dry_run_import_scripting_profile_from_file(
            self,
            path,
            selected_groups,
        )
        .await
    }

    async fn import_scripting_profile_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<ScriptingProfileImportResultDto, String> {
        scripting_profile_ipc_service::import_scripting_profile_from_file(self, path, selected_groups).await
    }

    async fn get_scripting_signer_registry(self) -> Result<ScriptingSignerRegistryDto, String> {
        scripting_ipc_service::get_scripting_signer_registry(self).await
    }

    async fn save_scripting_signer_registry(
        self,
        registry: ScriptingSignerRegistryDto,
    ) -> Result<(), String> {
        scripting_ipc_service::save_scripting_signer_registry(self, registry).await
    }

    async fn get_appearance_transparency_rules(self) -> Result<Vec<AppearanceTransparencyRuleDto>, String> {
        ghost_appearance_ipc_service::get_appearance_transparency_rules(self).await
    }

    async fn get_running_process_names(self) -> Result<Vec<String>, String> {
        runtime_helpers_ipc_service::get_running_process_names(self).await
    }

    async fn export_settings_bundle_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String> {
        settings_bundle_ipc_service::export_settings_bundle_to_file(
            self,
            path,
            selected_groups,
            theme,
            autostart_enabled,
        )
        .await
    }

    async fn preview_settings_bundle_from_file(
        self,
        path: String,
    ) -> Result<SettingsBundlePreviewDto, String> {
        settings_bundle_ipc_service::preview_settings_bundle_from_file(self, path).await
    }

    async fn import_settings_bundle_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String> {
        settings_bundle_ipc_service::import_settings_bundle_from_file(self, path, selected_groups)
            .await
    }

    async fn save_appearance_transparency_rule(
        self,
        app_process: String,
        opacity: u32,
        enabled: bool,
    ) -> Result<(), String> {
        ghost_appearance_ipc_service::save_appearance_transparency_rule(self, app_process, opacity, enabled)
            .await
    }

    async fn delete_appearance_transparency_rule(self, app_process: String) -> Result<(), String> {
        ghost_appearance_ipc_service::delete_appearance_transparency_rule(self, app_process).await
    }

    async fn apply_appearance_transparency_now(
        self,
        app_process: String,
        opacity: u32,
    ) -> Result<u32, String> {
        ghost_appearance_ipc_service::apply_appearance_transparency_now(self, app_process, opacity)
            .await
    }

    async fn restore_appearance_defaults(self) -> Result<u32, String> {
        ghost_appearance_ipc_service::restore_appearance_defaults(self).await
    }

    async fn get_ghost_suggestor_state(self) -> Result<GhostSuggestorStateDto, String> {
        ghost_appearance_ipc_service::get_ghost_suggestor_state(self).await
    }

    async fn ghost_suggestor_accept(self) -> Result<Option<(String, String)>, String> {
        ghost_appearance_ipc_service::ghost_suggestor_accept(self).await
    }

    async fn ghost_suggestor_snooze(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_suggestor_snooze(self).await
    }

    async fn ghost_suggestor_dismiss(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_suggestor_dismiss(self).await
    }

    async fn ghost_suggestor_ignore(self, phrase: String) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_suggestor_ignore(self, phrase).await
    }

    async fn ghost_suggestor_create_snippet(self) -> Result<Option<(String, String)>, String> {
        ghost_appearance_ipc_service::ghost_suggestor_create_snippet(self).await
    }

    async fn ghost_suggestor_cycle_forward(self) -> Result<u32, String> {
        ghost_appearance_ipc_service::ghost_suggestor_cycle_forward(self).await
    }

    async fn get_ghost_follower_state(
        self,
        search_filter: Option<String>,
    ) -> Result<GhostFollowerStateDto, String> {
        ghost_appearance_ipc_service::get_ghost_follower_state(self, search_filter).await
    }

    async fn ghost_follower_insert(self, _trigger: String, content: String) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_insert(self, _trigger, content).await
    }

    async fn bring_main_window_to_foreground(self) -> Result<(), String> {
        ghost_appearance_ipc_service::bring_main_window_to_foreground(self).await
    }

    async fn ghost_follower_restore_always_on_top(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_restore_always_on_top(self).await
    }

    async fn ghost_follower_capture_target_window(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_capture_target_window(self).await
    }

    async fn ghost_follower_touch(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_touch(self).await
    }

    async fn ghost_follower_set_collapsed(self, collapsed: bool) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_set_collapsed(self, collapsed).await
    }

    async fn ghost_follower_set_size(self, width: f64, height: f64) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_set_size(self, width, height).await
    }

    async fn ghost_follower_set_opacity(self, opacity_pct: u32) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_set_opacity(self, opacity_pct).await
    }

    async fn ghost_follower_save_position(self, x: i32, y: i32) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_save_position(self, x, y).await
    }

    async fn ghost_follower_hide(self) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_hide(self).await
    }

    async fn ghost_follower_set_search(self, filter: String) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_set_search(self, filter).await
    }

    async fn ghost_follower_request_view_full(self, content: String) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_request_view_full(self, content).await
    }

    async fn ghost_follower_request_edit(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_request_edit(self, category, snippet_idx).await
    }

    async fn ghost_follower_request_promote(
        self,
        content: String,
        trigger: String,
    ) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_request_promote(self, content, trigger).await
    }

    async fn ghost_follower_toggle_pin(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        ghost_appearance_ipc_service::ghost_follower_toggle_pin(self, category, snippet_idx).await
    }

    async fn get_pending_variable_input(self) -> Result<Option<PendingVarDto>, String> {
        variable_diagnostics_ipc_service::get_pending_variable_input(self).await
    }

    async fn submit_variable_input(self, values: HashMap<String, String>) -> Result<(), String> {
        variable_diagnostics_ipc_service::submit_variable_input(self, values).await
    }

    async fn cancel_variable_input(self) -> Result<(), String> {
        variable_diagnostics_ipc_service::cancel_variable_input(self).await
    }

    async fn get_expansion_stats(self) -> Result<ExpansionStatsDto, String> {
        variable_diagnostics_ipc_service::get_expansion_stats(self).await
    }

    async fn reset_expansion_stats(self) -> Result<(), String> {
        variable_diagnostics_ipc_service::reset_expansion_stats(self).await
    }

    async fn get_diagnostic_logs(self) -> Result<Vec<DiagnosticEntryDto>, String> {
        variable_diagnostics_ipc_service::get_diagnostic_logs(self).await
    }

    async fn clear_diagnostic_logs(self) -> Result<(), String> {
        variable_diagnostics_ipc_service::clear_diagnostic_logs(self).await
    }

    async fn test_snippet_logic(
        self,
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String> {
        variable_diagnostics_ipc_service::test_snippet_logic(self, content, user_values).await
    }

    async fn kms_evaluate_placeholders(
        self,
        content: String,
        user_values: Option<HashMap<String, String>>,
    ) -> Result<SnippetLogicTestResultDto, String> {
        variable_diagnostics_ipc_service::kms_evaluate_placeholders(self, content, user_values).await
    }

    async fn kms_get_graph(
        self,
        offset: u32,
        limit: u32,
        time_from_utc: Option<String>,
        time_to_utc: Option<String>,
    ) -> Result<KmsGraphDto, String> {
        kms_graph_ipc::kms_get_graph(self, offset, limit, time_from_utc, time_to_utc).await
    }

    async fn kms_get_local_graph(self, path: String, depth: u32) -> Result<KmsGraphDto, String> {
        kms_graph_ipc::kms_get_local_graph(self, path, depth).await
    }

    async fn kms_get_graph_shortest_path(
        self,
        from_path: String,
        to_path: String,
    ) -> Result<KmsGraphPathDto, String> {
        kms_graph_ipc::kms_get_graph_shortest_path(self, from_path, to_path).await
    }

    async fn kms_get_note_graph_preview(
        self,
        path: String,
        max_chars: u32,
    ) -> Result<KmsNoteGraphPreviewDto, String> {
        kms_graph_ipc::kms_get_note_graph_preview(self, path, max_chars).await
    }

    async fn kms_export_graph_diagnostics(self, path: String) -> Result<(), String> {
        kms_graph_ipc::kms_export_graph_diagnostics(self, path).await
    }

    async fn kms_export_wiki_links_json(self, path: String) -> Result<(), String> {
        kms_graph_ipc::kms_export_wiki_links_json(self, path).await
    }

    async fn kms_export_graph_graphml(self, path: String) -> Result<(), String> {
        kms_graph_ipc::kms_export_graph_graphml(self, path).await
    }

    async fn kms_export_graph_dto_json(self, path: String) -> Result<(), String> {
        kms_graph_ipc::kms_export_graph_dto_json(self, path).await
    }

    async fn kms_get_vault_graph_overrides_json(self) -> Result<String, String> {
        kms_graph_ipc::kms_get_vault_graph_overrides_json(self).await
    }

    async fn kms_set_vault_graph_overrides_json(self, json: String) -> Result<(), String> {
        kms_graph_ipc::kms_set_vault_graph_overrides_json(self, json).await
    }

    async fn kms_clear_vault_graph_overrides_json(self) -> Result<(), String> {
        kms_graph_ipc::kms_clear_vault_graph_overrides_json(self).await
    }

    async fn get_weather_location_suggestions(
        self,
        city_query: String,
        country: Option<String>,
        region: Option<String>,
    ) -> Result<Vec<String>, String> {
        runtime_helpers_ipc_service::get_weather_location_suggestions(
            self, city_query, country, region,
        )
        .await
    }

    async fn kms_launch(self) -> Result<(), String> {
        kms_lifecycle_ipc_service::kms_launch(self).await
    }

    async fn kms_initialize(self) -> Result<String, String> {
        kms_lifecycle_ipc_service::kms_initialize(self).await
    }

    async fn kms_get_note_links(self, path: String) -> Result<KmsLinksDto, String> {
        kms_notes_vault_ipc_service::kms_get_note_links(self, path).await
    }

    async fn kms_get_history(self, rel_path: String) -> Result<Vec<KmsVersion>, String> {
        kms_git_history_ipc_service::kms_get_history(self, rel_path).await
    }

    async fn kms_get_note_revision_content(self, hash: String, path: String) -> Result<String, String> {
        kms_git_history_ipc_service::kms_get_note_revision_content(self, hash, path).await
    }

    async fn kms_restore_version(self, hash: String, rel_path: String) -> Result<(), String> {
        kms_git_history_ipc_service::kms_restore_version(self, hash, rel_path).await
    }

    async fn kms_list_notes(self) -> Result<Vec<KmsNoteDto>, String> {
        kms_notes_vault_ipc_service::kms_list_notes(self).await
    }

    async fn kms_load_note(self, path: String) -> Result<String, String> {
        kms_notes_vault_ipc_service::kms_load_note(self, path).await
    }

    async fn kms_save_note(self, path: String, content: String) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_save_note(self, path, content).await
    }

    async fn kms_set_note_favorite(self, path: String, favorite: bool) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_set_note_favorite(self, path, favorite).await
    }

    async fn kms_get_recent_note_paths(self) -> Result<Vec<String>, String> {
        kms_notes_vault_ipc_service::kms_get_recent_note_paths(self).await
    }

    async fn kms_set_recent_note_paths(self, paths: Vec<String>) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_set_recent_note_paths(self, paths).await
    }

    async fn kms_get_favorite_path_order(self) -> Result<Vec<String>, String> {
        kms_notes_vault_ipc_service::kms_get_favorite_path_order(self).await
    }

    async fn kms_set_favorite_path_order(self, paths: Vec<String>) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_set_favorite_path_order(self, paths).await
    }

    async fn kms_delete_note(self, path: String) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_delete_note(self, path).await
    }

    async fn kms_rename_note(self, old_path: String, new_name: String) -> Result<String, String> {
        kms_notes_vault_ipc_service::kms_rename_note(self, old_path, new_name).await
    }

    async fn kms_rename_folder(self, old_path: String, new_name: String) -> Result<String, String> {
        kms_notes_vault_ipc_service::kms_rename_folder(self, old_path, new_name).await
    }

    async fn kms_delete_folder(self, path: String) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_delete_folder(self, path).await
    }

    async fn kms_move_item(self, path: String, new_parent_path: String) -> Result<String, String> {
        kms_notes_vault_ipc_service::kms_move_item(self, path, new_parent_path).await
    }

    async fn kms_get_logs(self, limit: u32) -> Result<Vec<KmsLogDto>, String> {
        kms_logs_ipc_service::kms_get_logs(self, limit).await
    }

    async fn kms_clear_logs(self) -> Result<(), String> {
        kms_logs_ipc_service::kms_clear_logs(self).await
    }

    async fn kms_create_folder(self, path: String) -> Result<(), String> {
        kms_notes_vault_ipc_service::kms_create_folder(self, path).await
    }

    async fn kms_get_vault_structure(self) -> Result<KmsFileSystemItemDto, String> {
        kms_notes_vault_ipc_service::kms_get_vault_structure(self).await
    }

    async fn kms_list_vault_media(self) -> Result<Vec<String>, String> {
        kms_notes_vault_ipc_service::kms_list_vault_media(self).await
    }

    async fn kms_list_unused_vault_media(self) -> Result<Vec<String>, String> {
        kms_notes_vault_ipc_service::kms_list_unused_vault_media(self).await
    }

    async fn kms_search_semantic(
        self,
        query: String,
        modality: Option<String>,
        limit: u32,
        search_mode: Option<String>,
    ) -> Result<Vec<SearchResultDto>, String> {
        kms_search_ipc_service::kms_search_semantic(self, query, modality, limit, search_mode).await
    }

    async fn kms_reindex_all(self) -> Result<(), String> {
        kms_reindex_ipc_service::kms_reindex_all(self).await
    }

    async fn kms_reindex_type(self, provider_id: String) -> Result<u32, String> {
        kms_reindex_ipc_service::kms_reindex_type(self, provider_id).await
    }

    async fn kms_get_indexing_status(self) -> Result<Vec<IndexingStatusDto>, String> {
        kms_indexing_diag_ipc_service::kms_get_indexing_status(self).await
    }

    async fn kms_get_indexing_details(self, provider_id: String) -> Result<Vec<KmsIndexStatusRow>, String> {
        kms_indexing_diag_ipc_service::kms_get_indexing_details(self, provider_id).await
    }

    async fn kms_retry_item(self, provider_id: String, entity_id: String) -> Result<(), String> {
        kms_indexing_diag_ipc_service::kms_retry_item(self, provider_id, entity_id).await
    }

    async fn kms_retry_failed(self, provider_id: String) -> Result<(), String> {
        kms_indexing_diag_ipc_service::kms_retry_failed(self, provider_id).await
    }

    async fn kms_repair_database(self) -> Result<(), String> {
        kms_indexing_diag_ipc_service::kms_repair_database(self).await
    }

    async fn kms_reindex_note(self, rel_path: String) -> Result<(), String> {
        kms_reindex_ipc_service::kms_reindex_note(self, rel_path).await
    }

    async fn kms_request_note_embedding_migration(self) -> Result<u64, String> {
        kms_embedding_ops_ipc_service::kms_request_note_embedding_migration(self).await
    }

    async fn kms_cancel_note_embedding_migration(self) -> Result<(), String> {
        kms_embedding_ops_ipc_service::kms_cancel_note_embedding_migration(self).await
    }

    async fn kms_get_embedding_policy_diagnostics(self) -> Result<KmsEmbeddingPolicyDiagnosticsDto, String> {
        kms_embedding_ops_ipc_service::kms_get_embedding_policy_diagnostics(self).await
    }

    async fn kms_get_embedding_diagnostic_log_path(self) -> Result<Option<String>, String> {
        kms_embedding_ops_ipc_service::kms_get_embedding_diagnostic_log_path(self).await
    }

    async fn kms_get_vault_path(self) -> Result<String, String> {
        kms_vault_path_ipc_service::kms_get_vault_path(self).await
    }

    async fn kms_set_vault_path(self, new_path: String, migrate: bool) -> Result<(), String> {
        kms_vault_path_ipc_service::kms_set_vault_path(self, new_path, migrate).await
    }

    async fn kms_list_skills(self) -> Result<Vec<SkillDto>, String> {
        kms_skills_ipc_service::kms_list_skills(self).await
    }

    async fn kms_get_skill(self, name: String) -> Result<Option<SkillDto>, String> {
        kms_skills_ipc_service::kms_get_skill(self, name).await
    }

    async fn kms_save_skill(self, skill: SkillDto, overwrite: bool) -> Result<(), String> {
        kms_skills_ipc_service::kms_save_skill(self, skill, overwrite).await
    }

    async fn kms_add_skill_resource(
        self,
        skill_name: String,
        source_path: String,
        target_subdir: Option<String>,
    ) -> Result<SkillResourceDto, String> {
        kms_skills_ipc_service::kms_add_skill_resource(self, skill_name, source_path, target_subdir).await
    }

    async fn kms_remove_skill_resource(self, skill_name: String, rel_path: String) -> Result<(), String> {
        kms_skills_ipc_service::kms_remove_skill_resource(self, skill_name, rel_path).await
    }

    async fn kms_delete_skill(self, name: String) -> Result<(), String> {
        kms_skills_ipc_service::kms_delete_skill(self, name).await
    }

    async fn kms_sync_skills(self) -> Result<(), String> {
        kms_skills_ipc_service::kms_sync_skills(self).await
    }

    async fn kms_check_skill_conflicts(
        self,
        skill_name: String,
        sync_targets: Vec<String>,
    ) -> Result<Vec<SyncConflictDto>, String> {
        kms_skills_ipc_service::kms_check_skill_conflicts(self, skill_name, sync_targets).await
    }

    async fn kms_get_diagnostics(self) -> Result<KmsDiagnosticsDto, String> {
        kms_indexing_diag_ipc_service::kms_get_diagnostics(self).await
    }

    async fn kms_prune_history(self) -> Result<String, String> {
        kms_git_history_ipc_service::kms_prune_history(self).await
    }
}

#[cfg(test)]
mod tests {
    use crate::settings_bundle_model::normalized_selected_groups;
    use crate::clipboard_text_persistence::{
        default_copy_to_clipboard_config, normalize_clipboard_path_or_default, write_clipboard_text_json_record,
    };
    use crate::appearance_enforcement::{
        effective_rules_for_enforcement, normalize_process_key, sort_appearance_rules_deterministic,
    };
    #[cfg(target_os = "windows")]
    use crate::appearance_enforcement::process_name_matches;
    use crate::AppearanceTransparencyRuleDto;

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_exact_and_case_insensitive() {
        assert!(process_name_matches("cursor.exe", "Cursor.EXE"));
        assert!(process_name_matches("CURSOR.EXE", "cursor.exe"));
        assert!(process_name_matches("cursor", "cursor"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_with_optional_exe_suffix() {
        assert!(process_name_matches("cursor", "cursor.exe"));
        assert!(process_name_matches("cursor.exe", "cursor"));
        assert!(!process_name_matches("cursor", "code.exe"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_rejects_empty_values() {
        assert!(!process_name_matches("", "cursor.exe"));
        assert!(!process_name_matches("cursor.exe", ""));
        assert!(!process_name_matches("   ", "cursor.exe"));
    }

    #[test]
    fn normalize_process_key_handles_exe_suffix_and_case() {
        assert_eq!(normalize_process_key("Cursor.EXE"), "cursor");
        assert_eq!(normalize_process_key("cursor"), "cursor");
        assert_eq!(normalize_process_key("  Code.ExE  "), "code");
    }

    #[test]
    fn appearance_rules_sort_is_deterministic() {
        let mut rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "zeta.exe".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 180,
                enabled: false,
            },
            AppearanceTransparencyRuleDto {
                app_process: "alpha.exe".to_string(),
                opacity: 140,
                enabled: true,
            },
        ];
        sort_appearance_rules_deterministic(&mut rules);
        assert_eq!(rules[0].app_process, "alpha.exe");
        assert_eq!(rules[1].app_process, "zeta.exe");
        assert_eq!(rules[2].app_process, "cursor");
    }

    #[test]
    fn selected_groups_defaults_to_all_when_empty() {
        let groups = normalized_selected_groups(&[]);
        assert!(groups.contains(&"templates".to_string()));
        assert!(groups.contains(&"appearance".to_string()));
        assert!(groups.len() >= 9);
    }

    #[test]
    fn selected_groups_normalizes_aliases_and_deduplicates() {
        let input = vec![
            "Ghost Follower".to_string(),
            "ghost-follower".to_string(),
            "appearance".to_string(),
            "invalid".to_string(),
        ];
        let groups = normalized_selected_groups(&input);
        assert_eq!(groups, vec!["ghost_follower".to_string(), "appearance".to_string()]);
    }

    #[test]
    fn appearance_integration_save_restart_reenforce_is_deterministic() {
        let saved_rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor.exe".to_string(),
                opacity: 120,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "code.exe".to_string(),
                opacity: 180,
                enabled: true,
            },
        ];

        // Simulate startup/enforcement after restart by recomputing effective rules.
        let first_start = effective_rules_for_enforcement(saved_rules.clone());
        let second_start = effective_rules_for_enforcement(saved_rules);
        assert_eq!(first_start.len(), 2);
        assert_eq!(second_start.len(), 2);
        assert_eq!(first_start[0].app_process, second_start[0].app_process);
        assert_eq!(first_start[0].opacity, second_start[0].opacity);
        assert_eq!(first_start[1].app_process, second_start[1].app_process);
        assert_eq!(first_start[1].opacity, second_start[1].opacity);
    }

    #[test]
    fn appearance_stress_many_rules_remains_stable() {
        let mut rules = Vec::new();
        for i in 0..5000u32 {
            let base = format!("app{}", i % 300);
            rules.push(AppearanceTransparencyRuleDto {
                app_process: if i % 2 == 0 { base.clone() } else { format!("{base}.exe") },
                opacity: 20 + (i % 236),
                enabled: i % 5 != 0,
            });
        }

        let first = effective_rules_for_enforcement(rules.clone());
        let second = effective_rules_for_enforcement(rules);
        assert_eq!(first.len(), second.len());
        for idx in 0..first.len() {
            assert_eq!(first[idx].app_process, second[idx].app_process);
            assert_eq!(first[idx].opacity, second[idx].opacity);
            assert!(first[idx].enabled);
        }
        assert!(first.len() <= 300);
    }

    #[test]
    fn copy_to_clipboard_defaults_include_output_directories() {
        let cfg = default_copy_to_clipboard_config(5000);
        assert!(!cfg.json_output_dir.trim().is_empty());
        assert!(!cfg.image_storage_dir.trim().is_empty());
    }

    #[test]
    fn clipboard_text_json_record_is_written_to_selected_directory() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let out = tmp.path().join("json-output");
        let out_s = out.to_string_lossy().to_string();
        write_clipboard_text_json_record(&out_s, "hello world", "Cursor.exe", "Editor")
            .expect("json write");
        let entries = std::fs::read_dir(&out)
            .expect("read output dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("dir entries");
        assert_eq!(entries.len(), 1);
        let payload = std::fs::read_to_string(entries[0].path()).expect("read json file");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("parse json");
        assert_eq!(parsed.get("entry_type").and_then(|v| v.as_str()), Some("text"));
        assert_eq!(parsed.get("content").and_then(|v| v.as_str()), Some("hello world"));

        let fallback = normalize_clipboard_path_or_default("   ", out.clone());
        assert_eq!(fallback, out);
    }

    #[test]
    fn test_extract_links_from_markdown() {
        use crate::kms_sync_orchestration::{extract_links_from_markdown, LinkCandidate};

        let content = "Check [[WikiLink]] and [StandardLink](./Note.md#anchor) and [External](https://google.com)";
        let candidates = extract_links_from_markdown(content);
        assert_eq!(candidates.len(), 2);
        match &candidates[0] {
            LinkCandidate::Wiki(t) => assert_eq!(t, "WikiLink"),
            _ => panic!("Expected WikiLink"),
        }
        match &candidates[1] {
            LinkCandidate::Path(p) => assert_eq!(p, "./Note.md#anchor"),
            _ => panic!("Expected Path"),
        }
    }
}
