use std::path::Path;
use crate::kms_error::{KmsError, KmsResult};
use crate::kms_repository;
use crate::kms_diagnostic_service::KmsDiagnosticService;
use chrono::{DateTime, Local};
use walkdir::WalkDir;

pub struct KmsSyncService;

impl KmsSyncService {
    /// Performs a boot-time synchronization and conflict detection.
    pub async fn run_boot_sync() -> KmsResult<()> {
        let vault_path = kms_repository::get_vault_path()?;
        let notes_dir = vault_path.join("notes");
        if !notes_dir.exists() {
            return Ok(());
        }

        KmsDiagnosticService::info("Starting boot-time conflict check...", None);

        let mut conflicts_found = 0;
        let mut synced_count = 0;

        for entry in WalkDir::new(&notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            let disk_path = entry.path();
            let rel_path = Self::get_relative_path(&vault_path, disk_path)?;
            
            // 1. Get DB metadata
            let db_note = kms_repository::get_note_by_path(&rel_path)?;
            
            if let Some(note) = db_note {
                // 2. Compare modification times
                let metadata = std::fs::metadata(disk_path).map_err(KmsError::Io)?;
                let disk_mtime: DateTime<Local> = metadata.modified().map_err(KmsError::Io)?.into();
                
                let db_mtime = note.last_modified
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Local))
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap().into());

                // Use a small buffer (2 seconds) to avoid false positives due to filesystem jitter
                if disk_mtime.timestamp() > db_mtime.timestamp() + 2 {
                    KmsDiagnosticService::warn(
                        &format!("External modification detected: {}", rel_path),
                        Some(format!("Disk: {}, DB: {}", disk_mtime, db_mtime))
                    );
                    
                    Self::handle_conflict(&vault_path, disk_path, &rel_path).await?;
                    conflicts_found += 1;
                }
            }
            synced_count += 1;
        }

        if conflicts_found > 0 {
            KmsDiagnosticService::info(
                &format!("Boot sync complete. Found {} conflicts across {} notes.", conflicts_found, synced_count),
                None
            );
        } else {
            KmsDiagnosticService::debug("Boot sync complete. No conflicts detected.", None);
        }

        Ok(())
    }

    fn get_relative_path(vault: &Path, abs_path: &Path) -> KmsResult<String> {
        let rel = abs_path.strip_prefix(vault)
            .map_err(|_| KmsError::Path(format!("Path {} is not within vault {}", abs_path.display(), vault.display())))?;
        Ok(rel.to_string_lossy().to_string().replace('\\', "/"))
    }

    async fn handle_conflict(vault: &Path, disk_path: &Path, rel_path: &str) -> KmsResult<()> {
        let conflicts_dir = vault.join("Conflicts");
        if !conflicts_dir.exists() {
            std::fs::create_dir_all(&conflicts_dir).map_err(KmsError::Io)?;
        }

        let file_name = disk_path.file_name().unwrap_or_default().to_string_lossy();
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let conflict_name = format!("{}_{}.md", file_name.trim_end_matches(".md"), timestamp);
        let conflict_path = conflicts_dir.join(conflict_name);

        // Copy the conflicting file to the Conflicts folder
        std::fs::copy(disk_path, &conflict_path).map_err(KmsError::Io)?;
        
        KmsDiagnosticService::info(
            &format!("Moved conflicting version of '{}' to Conflicts notebook.", rel_path),
            Some(format!("Backup path: {}", conflict_path.display()))
        );

        Ok(())
    }
}
