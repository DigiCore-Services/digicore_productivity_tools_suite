use crate::kms_repository;
use digicore_core::domain::entities::skill::Skill;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Emitter};
use std::sync::{Arc, Mutex, OnceLock};
use notify::{Watcher, RecursiveMode, Config};
use digicore_text_expander::ports::SkillRepository;

pub async fn sync_skills_dir(app: &AppHandle, path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    log::info!("[SkillSync] Scanning direction: {}", path.display());
    
    let mut skills = Vec::new();
    collect_skills_recursive(path, &mut skills)?;
    
    log::info!("[SkillSync] Found {} skills in {}", skills.len(), path.display());
    
    for skill in skills {
        kms_repository::KmsSkillRepository.save_skill(&skill).await?;
        // Trigger indexing
        let service = app.state::<Arc<crate::indexing_service::KmsIndexingService>>();
        let _ = service.index_single_item(app, "skills", &skill.metadata.name).await;
    }
    
    Ok(())
}

fn collect_skills_recursive(dir: &Path, skills: &mut Vec<Skill>) -> anyhow::Result<()> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skills can be in subdirectories
                collect_skills_recursive(&path, skills)?;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                // Check if it's a skill (has metadata or is named SKILL.md)
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(skill) = Skill::from_markdown(content, path.clone()) {
                        skills.push(skill);
                    }
                }
            }
        }
    }
    Ok(())
}

static SKILL_WATCHER: OnceLock<Mutex<Option<notify::RecommendedWatcher>>> = OnceLock::new();

pub fn start_skill_watcher(app: AppHandle, paths: Vec<PathBuf>) {
    let app_handle = app.clone();
    
    let (tx, rx) = std::sync::mpsc::channel();
    
    let mut watcher = match notify::RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            log::error!("[SkillSync] Failed to create watcher: {}", e);
            return;
        }
    };
    
    for path in &paths {
        if path.exists() {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                log::error!("[SkillSync] Failed to watch {}: {}", path.display(), e);
            } else {
                log::info!("[SkillSync] Watching skills at {}", path.display());
            }
        }
    }
    
    let _ = SKILL_WATCHER.get_or_init(|| Mutex::new(Some(watcher)));
    
    std::thread::spawn(move || {
        for res in rx {
            match res {
                Ok(event) => {
                    // Simple debounce/filter logic
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        for path in event.paths {
                            if path.extension().map(|e| e == "md").unwrap_or(false) {
                                let h = app_handle.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = handle_file_event(&h, &path).await;
                                });
                            }
                        }
                    }
                },
                Err(e) => log::error!("[SkillSync] Watcher error: {}", e),
            }
        }
    });
}

async fn handle_file_event(app: &AppHandle, path: &Path) -> anyhow::Result<()> {
    let repo = kms_repository::KmsSkillRepository;

    if !path.exists() {
        // Assume deleted
        if let Ok(Some(name)) = repo.find_skill_name_by_path(path).await {
            log::info!("[SkillSync] Deleting skill due to file removal: {}", name);
            repo.delete_skill(&name).await?;
            let _ = kms_repository::delete_embeddings_for_entity("skill", &name);
            let _ = kms_repository::update_index_status("skills", &name, "deleted", None);
            let _ = app.emit("skill-sync-deleted", &name);
        }
        return Ok(());
    }
    
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(skill) = Skill::from_markdown(content, path.to_path_buf()) {
            log::info!("[SkillSync] Updating skill from file: {}", skill.metadata.name);
            repo.save_skill(&skill).await?;
            let service = app.state::<Arc<crate::indexing_service::KmsIndexingService>>();
            let _ = service.index_single_item(app, "skills", &skill.metadata.name).await;
            let _ = app.emit("skill-sync-updated", &skill.metadata.name);
        }
    }
    
    Ok(())
}

pub fn get_default_skill_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    #[cfg(target_os = "windows")]
    {
        if let Some(user_profile) = std::env::var_os("USERPROFILE") {
            let base = PathBuf::from(user_profile);
            paths.push(base.join(".cursor").join("skills"));
            paths.push(base.join(".claude").join("skills"));
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let base = PathBuf::from(home);
            paths.push(base.join(".cursor").join("skills"));
            paths.push(base.join(".claude").join("skills"));
        }
    }
    
    paths
}
