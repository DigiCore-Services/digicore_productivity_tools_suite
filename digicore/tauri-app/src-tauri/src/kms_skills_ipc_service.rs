//! KMS Skill Hub: list/get/save skills, resources, sync, and sync-target conflict checks.

use super::*;
use digicore_core::domain::entities::skill::{Skill, SkillMetadata, SkillScope};
use digicore_text_expander::ports::skill::SkillRepository;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) async fn kms_list_skills(_host: ApiImpl) -> Result<Vec<SkillDto>, String> {
    let request_id = kms_request_id("list_skills");
    let repo = kms_repository::KmsSkillRepository;
    let mut skills = repo.list_skills().await.map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_LIST",
            "KMS_LIST_SKILLS_FAIL",
            "Failed to list skills",
            Some(e.to_string()),
        )
    })?;

    let mut dtos = Vec::new();
    for s in &mut skills {
        if let Err(e) = s.refresh_resources() {
            log::warn!(
                "[KMS][Skill] event_code=KMS_LIST_SKILLS_REFRESH_RESOURCES_WARN request_id={} skill={} error={}",
                request_id,
                s.metadata.name,
                e
            );
        }
        dtos.push(SkillDto {
            metadata: SkillMetadataDto {
                name: s.metadata.name.clone(),
                description: s.metadata.description.clone(),
                version: s
                    .metadata
                    .version
                    .clone()
                    .unwrap_or_else(|| "1.0.0".to_string()),
                author: s.metadata.author.clone(),
                tags: s.metadata.tags.clone(),
                license: s.metadata.license.clone(),
                compatibility: s.metadata.compatibility.clone(),
                metadata: s.metadata.extra_metadata.as_ref().map(|v| v.to_string()),
                disable_model_invocation: s.metadata.disable_model_invocation,
                scope: match s.metadata.scope {
                    digicore_core::domain::entities::skill::SkillScope::Global => {
                        "Global".to_string()
                    }
                    digicore_core::domain::entities::skill::SkillScope::Project => {
                        "Project".to_string()
                    }
                },
                sync_targets: s.metadata.sync_targets.clone(),
            },
            path: Some(s.path.to_string_lossy().to_string()),
            instructions: Some(s.instructions.clone()),
            resources: s
                .resources
                .iter()
                .map(|r| SkillResourceDto {
                    name: r.name.clone(),
                    r#type: format!("{:?}", r.r#type),
                    rel_path: r
                        .path
                        .strip_prefix(&s.path)
                        .unwrap_or(&r.path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                })
                .collect(),
        });
    }
    Ok(dtos)
}

pub(crate) async fn kms_get_skill(_host: ApiImpl, name: String) -> Result<Option<SkillDto>, String> {
    let request_id = kms_request_id("get_skill");
    let repo = kms_repository::KmsSkillRepository;
    let skill_opt = repo.get_skill(&name).await.map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_GET",
            "KMS_GET_SKILL_FAIL",
            "Failed to get skill",
            Some(e.to_string()),
        )
    })?;

    if let Some(mut s) = skill_opt {
        if let Err(e) = s.refresh_resources() {
            log::warn!(
                "[KMS][Skill] event_code=KMS_GET_SKILL_REFRESH_RESOURCES_WARN request_id={} skill={} error={}",
                request_id,
                name,
                e
            );
        }
        Ok(Some(SkillDto {
            metadata: SkillMetadataDto {
                name: s.metadata.name,
                description: s.metadata.description,
                version: s.metadata.version.unwrap_or_else(|| "1.0.0".to_string()),
                author: s.metadata.author,
                tags: s.metadata.tags,
                license: s.metadata.license,
                compatibility: s.metadata.compatibility,
                metadata: s.metadata.extra_metadata.map(|v| v.to_string()),
                disable_model_invocation: s.metadata.disable_model_invocation,
                scope: match s.metadata.scope {
                    digicore_core::domain::entities::skill::SkillScope::Global => {
                        "Global".to_string()
                    }
                    digicore_core::domain::entities::skill::SkillScope::Project => {
                        "Project".to_string()
                    }
                },
                sync_targets: s.metadata.sync_targets.clone(),
            },
            path: Some(s.path.to_string_lossy().to_string()),
            instructions: Some(s.instructions),
            resources: s
                .resources
                .into_iter()
                .map(|r| SkillResourceDto {
                    name: r.name,
                    r#type: format!("{:?}", r.r#type),
                    rel_path: r
                        .path
                        .strip_prefix(&s.path)
                        .unwrap_or(&r.path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                })
                .collect(),
        }))
    } else {
        Ok(None)
    }
}

pub(crate) async fn kms_save_skill(
    host: ApiImpl,
    skill: SkillDto,
    overwrite: bool,
) -> Result<(), String> {
    let request_id = kms_request_id("save_skill");
    let repo = kms_repository::KmsSkillRepository;

    let scope = if skill.metadata.scope == "Project" {
        SkillScope::Project
    } else {
        SkillScope::Global
    };

    let skill_entity = Skill {
        metadata: SkillMetadata {
            name: skill.metadata.name,
            description: skill.metadata.description,
            version: Some(skill.metadata.version),
            author: skill.metadata.author,
            tags: skill.metadata.tags,
            license: skill.metadata.license,
            compatibility: skill.metadata.compatibility,
            extra_metadata: skill
                .metadata
                .metadata
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
            disable_model_invocation: skill.metadata.disable_model_invocation,
            scope,
            sync_targets: skill.metadata.sync_targets,
        },
        instructions: skill.instructions.unwrap_or_default(),
        resources: Vec::new(),
        path: PathBuf::from(skill.path.unwrap_or_default()),
    };

    repo.save_skill(&skill_entity).await.map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_SAVE",
            "KMS_SAVE_SKILL_FAIL",
            "Failed to save skill",
            Some(e.to_string()),
        )
    })?;

    if let Err(e) = skill_sync::sync_skill_to_targets(&skill_entity, overwrite).await {
        log::warn!(
            "[KMS][Skill] event_code=KMS_SAVE_SKILL_SYNC_TARGETS_WARN request_id={} skill={} error={}",
            request_id,
            skill_entity.metadata.name,
            e
        );
    }

    let app = get_app(&host.app_handle);
    let indexing_service = app.state::<Arc<indexing_service::KmsIndexingService>>();
    if let Err(e) = indexing_service
        .index_single_item(&app, "skills", &skill_entity.metadata.name)
        .await
    {
        log::warn!(
            "[KMS][Skill] event_code=KMS_SAVE_SKILL_REINDEX_WARN request_id={} skill={} error={}",
            request_id,
            skill_entity.metadata.name,
            e
        );
    }

    Ok(())
}

pub(crate) async fn kms_add_skill_resource(
    _host: ApiImpl,
    skill_name: String,
    source_path: String,
    target_subdir: Option<String>,
) -> Result<SkillResourceDto, String> {
    let request_id = kms_request_id("add_skill_resource");
    let repo = kms_repository::KmsSkillRepository;
    let mut skill = repo
        .get_skill(&skill_name)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_GET",
                "KMS_ADD_SKILL_RESOURCE_SKILL_LOOKUP_FAIL",
                "Failed to look up skill",
                Some(e.to_string()),
            )
        })?
        .ok_or_else(|| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_NOT_FOUND",
                "KMS_ADD_SKILL_RESOURCE_SKILL_NOT_FOUND",
                "Skill not found",
                Some(skill_name.clone()),
            )
        })?;

    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_SKILL_RESOURCE_SOURCE_MISSING",
            "KMS_ADD_SKILL_RESOURCE_SOURCE_NOT_FOUND",
            "Source path does not exist",
            Some(source_path),
        ));
    }

    let target_dir = if let Some(sub) = target_subdir {
        let d = skill.path.join(sub);
        std::fs::create_dir_all(&d).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_DIR_CREATE",
                "KMS_ADD_SKILL_RESOURCE_DIR_CREATE_FAIL",
                "Failed to create target directory",
                Some(e.to_string()),
            )
        })?;
        d
    } else {
        skill.path.clone()
    };

    let filename = source.file_name().ok_or_else(|| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_RESOURCE_INVALID_SOURCE",
            "KMS_ADD_SKILL_RESOURCE_INVALID_SOURCE",
            "Invalid source filename",
            Some(source.to_string_lossy().to_string()),
        )
    })?;
    let target_path = target_dir.join(filename);

    if source.is_dir() {
        copy_dir_recursive(&source, &target_path).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_COPY",
                "KMS_ADD_SKILL_RESOURCE_COPY_DIR_FAIL",
                "Failed to copy source directory",
                Some(e.to_string()),
            )
        })?;
    } else {
        std::fs::copy(&source, &target_path).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_COPY",
                "KMS_ADD_SKILL_RESOURCE_COPY_FILE_FAIL",
                "Failed to copy source file",
                Some(e.to_string()),
            )
        })?;
    }

    skill.refresh_resources().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_RESOURCE_REFRESH",
            "KMS_ADD_SKILL_RESOURCE_REFRESH_FAIL",
            "Failed to refresh skill resources",
            Some(e.to_string()),
        )
    })?;

    let resource = skill
        .resources
        .iter()
        .find(|r| r.path == target_path)
        .ok_or_else(|| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_IDENTIFY",
                "KMS_ADD_SKILL_RESOURCE_IDENTIFY_FAIL",
                "Failed to identify newly added resource",
                None,
            )
        })?;

    Ok(SkillResourceDto {
        name: resource.name.clone(),
        r#type: format!("{:?}", resource.r#type),
        rel_path: resource
            .path
            .strip_prefix(&skill.path)
            .unwrap_or(&resource.path)
            .to_string_lossy()
            .replace('\\', "/"),
    })
}

pub(crate) async fn kms_remove_skill_resource(
    _host: ApiImpl,
    skill_name: String,
    rel_path: String,
) -> Result<(), String> {
    let request_id = kms_request_id("remove_skill_resource");
    let repo = kms_repository::KmsSkillRepository;
    let skill = repo
        .get_skill(&skill_name)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_GET",
                "KMS_REMOVE_SKILL_RESOURCE_SKILL_LOOKUP_FAIL",
                "Failed to look up skill",
                Some(e.to_string()),
            )
        })?
        .ok_or_else(|| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_NOT_FOUND",
                "KMS_REMOVE_SKILL_RESOURCE_SKILL_NOT_FOUND",
                "Skill not found",
                Some(skill_name.clone()),
            )
        })?;

    let abs_path = skill.path.join(rel_path.replace('/', "\\"));

    if !abs_path.exists() {
        return Ok(());
    }

    if abs_path.is_dir() {
        std::fs::remove_dir_all(&abs_path).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_DELETE",
                "KMS_REMOVE_SKILL_RESOURCE_DIR_DELETE_FAIL",
                "Failed to remove resource directory",
                Some(e.to_string()),
            )
        })?;
    } else {
        std::fs::remove_file(&abs_path).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_RESOURCE_DELETE",
                "KMS_REMOVE_SKILL_RESOURCE_FILE_DELETE_FAIL",
                "Failed to remove resource file",
                Some(e.to_string()),
            )
        })?;
    }

    Ok(())
}

pub(crate) async fn kms_delete_skill(_host: ApiImpl, name: String) -> Result<(), String> {
    let request_id = kms_request_id("delete_skill");
    let repo = kms_repository::KmsSkillRepository;
    repo.delete_skill(&name).await.map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_SKILL_DELETE",
            "KMS_DELETE_SKILL_FAIL",
            "Failed to delete skill",
            Some(e.to_string()),
        )
    })?;

    if let Err(e) = kms_repository::delete_embeddings_for_entity("skill", &name) {
        log::warn!(
            "[KMS][Skill] event_code=KMS_DELETE_SKILL_EMBED_CLEANUP_WARN request_id={} skill={} error={}",
            request_id,
            name,
            e
        );
    }
    if let Err(e) = kms_repository::update_index_status("skills", &name, "deleted", None) {
        log::warn!(
            "[KMS][Skill] event_code=KMS_DELETE_SKILL_INDEX_STATUS_WARN request_id={} skill={} error={}",
            request_id,
            name,
            e
        );
    }

    Ok(())
}

pub(crate) async fn kms_sync_skills(host: ApiImpl) -> Result<(), String> {
    let request_id = kms_request_id("sync_skills");
    let app = get_app(&host.app_handle);
    let service = app.state::<Arc<indexing_service::KmsIndexingService>>();
    // Triggers SkillIndexProvider::index_all (DB-backed listing; future: scan filesystem first).

    service
        .index_provider_by_id(&app, "skills")
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_SKILL_SYNC",
                "KMS_SYNC_SKILLS_FAIL",
                "Failed to sync/reindex skills",
                Some(e),
            )
        })?;
    Ok(())
}

pub(crate) async fn kms_check_skill_conflicts(
    _host: ApiImpl,
    skill_name: String,
    sync_targets: Vec<String>,
) -> Result<Vec<SyncConflictDto>, String> {
    let request_id = kms_request_id("check_skill_conflicts");
    let mut conflicts = Vec::new();
    let home = dirs::home_dir().ok_or_else(|| {
        kms_ipc_error(
            &request_id,
            "KMS_HOME_DIR",
            "KMS_CHECK_SKILL_CONFLICTS_HOME_DIR_FAIL",
            "Could not find home directory",
            None,
        )
    })?;

    for target in &sync_targets {
        let base_path = if target.starts_with('.') {
            home.join(target)
        } else {
            PathBuf::from(target)
        };

        let skill_path = base_path.join(&skill_name);
        if skill_path.exists() {
            conflicts.push(SyncConflictDto {
                target: target.clone(),
                existing_name: skill_name.clone(),
                conflict_type: "NameCollision".to_string(),
            });
        }
    }

    Ok(conflicts)
}

