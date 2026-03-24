use digicore_core::domain::entities::skill::Skill;
use std::path::PathBuf;
use async_trait::async_trait;

#[async_trait]
pub trait SkillRepository: Send + Sync {
    /// List all skills in the repository (Metadata only for Level 1 disclosure).
    async fn list_skills(&self) -> anyhow::Result<Vec<Skill>>;

    /// Get a specific skill by its unique name (Full instructions for Level 2).
    async fn get_skill(&self, name: &str) -> anyhow::Result<Option<Skill>>;

    /// Save a new or existing skill to the KMS vault.
    async fn save_skill(&self, skill: &Skill) -> anyhow::Result<()>;

    /// Delete a skill by its name.
    async fn delete_skill(&self, name: &str) -> anyhow::Result<()>;

    /// Delete a skill by its filesystem path.
    async fn delete_skill_by_path(&self, path: &std::path::Path) -> anyhow::Result<()>;

    /// Find a skill name by its filesystem path.
    async fn find_skill_name_by_path(&self, path: &std::path::Path) -> anyhow::Result<Option<String>>;

    /// Refresh the repository by scanning the KMS vault filesystem.
    async fn refresh(&self) -> anyhow::Result<()>;
    
    /// Get the root path of the skill vault.
    fn vault_path(&self) -> PathBuf;
}
