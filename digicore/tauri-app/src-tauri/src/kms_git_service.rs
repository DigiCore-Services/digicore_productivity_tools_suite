use std::path::Path;
use git2::{Repository, Signature};
use crate::kms_error::{KmsError, KmsResult};
use crate::kms_repository;

/// Service for silent, background Git versioning of the KMS vault.
pub struct KmsGitService;

impl KmsGitService {
    /// Ensures a Git repository exists in the vault.
    pub fn ensure_repo() -> KmsResult<()> {
        let vault_path = kms_repository::get_vault_path()?;
        let git_dir = vault_path.join(".git");

        if !git_dir.exists() {
            Repository::init(&vault_path).map_err(|e| KmsError::General(format!("Git Init Error: {}", e)))?;
            
            // Create a .gitignore if it doesn't exist
            let gitignore_path = vault_path.join(".gitignore");
            if !gitignore_path.exists() {
                let _ = std::fs::write(&gitignore_path, "Conflicts/\n.kms/embeddings/\n");
                // Commit the .gitignore
                let _ = Self::commit_path(".gitignore", "Initial commit: Add .gitignore");
            }
        }
        Ok(())
    }

    /// Commits a specific file relative to the vault root.
    pub fn commit_path(rel_path: &str, message: &str) -> KmsResult<()> {
        let vault_path = kms_repository::get_vault_path()?;
        let repo = Repository::open(&vault_path).map_err(|e| KmsError::General(format!("Git Open Error: {}", e)))?;
        
        // 1. Add to index
        let mut index = repo.index().map_err(|e| KmsError::General(format!("Git Index Error: {}", e)))?;
        index.add_path(Path::new(rel_path)).map_err(|e| KmsError::General(format!("Git Add Error: {}", e)))?;
        index.write().map_err(|e| KmsError::General(format!("Git Index Write Error: {}", e)))?;
        
        let oid = index.write_tree().map_err(|e| KmsError::General(format!("Git Tree Error: {}", e)))?;
        let tree = repo.find_tree(oid).map_err(|e| KmsError::General(format!("Git Find Tree Error: {}", e)))?;
        
        // 2. Commit
        let signature = Signature::now("DigiCore KMS", "kms@digicore.local").map_err(|e| KmsError::General(format!("Git Signature Error: {}", e)))?;
        
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit().map_err(|e| KmsError::General(format!("Git Peel Error: {}", e)))?),
            Err(_) => None,
        };

        let parents = match &parent_commit {
            Some(c) => vec![c],
            None => vec![],
        };

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        ).map_err(|e| KmsError::General(format!("Git Commit Error: {}", e)))?;

        Ok(())
    }

    /// Lists history for a specific file.
    pub fn get_history(_rel_path: &str) -> KmsResult<Vec<KmsVersion>> {
        let vault_path = kms_repository::get_vault_path()?;
        let repo = Repository::open(&vault_path).map_err(|e| KmsError::General(format!("Git Open Error: {}", e)))?;
        
        let mut revwalk = repo.revwalk().map_err(|e| KmsError::General(format!("Git Revwalk Error: {}", e)))?;
        revwalk.push_head().map_err(|e| KmsError::General(format!("Git Push Head Error: {}", e)))?;
        
        let mut versions = Vec::new();
        for oid_res in revwalk {
            let oid = oid_res.map_err(|e| KmsError::General(format!("Git Walk Error: {}", e)))?;
            let commit = repo.find_commit(oid).map_err(|e| KmsError::General(format!("Git Find Commit Error: {}", e)))?;
            
            // Optimization: check if this commit modified the specific file
            // For now, just return all commits that might contain it
            versions.push(KmsVersion {
                hash: commit.id().to_string(),
                message: commit.message().unwrap_or("").to_string(),
                timestamp: commit.time().seconds() as f64,
                author: commit.author().name().unwrap_or("").to_string(),
            });

            if versions.len() >= 50 { break; } // limit history for now
        }

        Ok(versions)
    }

    /// Restores a file to a specific version.
    pub fn restore_version(hash: &str, rel_path: &str) -> KmsResult<()> {
        let vault_path = kms_repository::get_vault_path()?;
        let repo = Repository::open(&vault_path).map_err(|e| KmsError::General(format!("Git Open Error: {}", e)))?;
        
        let oid = git2::Oid::from_str(hash).map_err(|e| KmsError::General(format!("Invalid OID: {}", e)))?;
        let commit = repo.find_commit(oid).map_err(|e| KmsError::General(format!("Commit not found: {}", e)))?;
        let tree = commit.tree().map_err(|e| KmsError::General(format!("Tree not found: {}", e)))?;
        
        let entry = tree.get_path(Path::new(rel_path)).map_err(|e| KmsError::General(format!("File not found in history: {}", e)))?;
        let blob = repo.find_blob(entry.id()).map_err(|e| KmsError::General(format!("Blob not found: {}", e)))?;
        
        // Write the blob content back to the file
        let abs_path = vault_path.join(rel_path);
        std::fs::write(&abs_path, blob.content()).map_err(KmsError::Io)?;
        
        // Also commit the restoration
        Self::commit_path(rel_path, &format!("Restored version {}", hash))?;
        
        Ok(())
    }

    /// Prunes the Git history. Currently just triggers a GC.
    pub fn prune_history() -> KmsResult<String> {
        let vault_path = kms_repository::get_vault_path()?;
        
        // Use standard git command if available, or just notify.
        // git2-rs doesn't have a high-level 'gc' command easily accessible.
        // We'll use a Command spawn for simplicity as this is a background maintenance task.
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&vault_path)
            .arg("gc")
            .arg("--prune=now")
            .arg("--aggressive")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                Ok("Git history pruned successfully.".to_string())
            }
            Ok(out) => {
                Err(KmsError::General(format!("Git GC failed: {}", String::from_utf8_lossy(&out.stderr))))
            }
            Err(e) => {
                Err(KmsError::General(format!("Failed to execute git gc: {}", e)))
            }
        }
    }
}

#[derive(serde::Serialize, specta::Type)]
pub struct KmsVersion {
    pub hash: String,
    pub message: String,
    pub timestamp: f64,
    pub author: String,
}
