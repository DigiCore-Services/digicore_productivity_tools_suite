use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Level 1: Metadata (always loaded)
/// Extracted from the YAML frontmatter of `SKILL.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Represents a "Skill" as defined by the Anthropic/Cursor standard.
/// Uses a three-tier progressive disclosure model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    /// Level 1: Core metadata for discovery.
    pub metadata: SkillMetadata,
    
    /// Level 2: Procedural instructions (the body of SKILL.md).
    pub instructions: String,
    
    /// Level 3: Optional resources (scripts, templates, extra docs).
    #[serde(default)]
    pub resources: Vec<Resource>,
    
    /// Filesystem path to the skill directory.
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resource {
    pub name: String,
    pub r#type: ResourceType,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceType {
    Script,
    Template,
    Reference,
    Other,
}

impl Skill {
    /// Parses a skill from a directory containing a `SKILL.md` file.
    pub fn from_dir(dir_path: PathBuf) -> anyhow::Result<Self> {
        let skill_md_path = dir_path.join("SKILL.md");
        if !skill_md_path.exists() {
            return Err(anyhow::anyhow!("SKILL.md not found in {:?}", dir_path));
        }

        let content = std::fs::read_to_string(&skill_md_path)?;
        Self::from_markdown(content, dir_path)
    }

    /// Parses a skill from markdown content (SKILL.md).
    pub fn from_markdown(content: String, path: PathBuf) -> anyhow::Result<Self> {
        let (metadata_str, instructions) = split_frontmatter(&content)?;
        
        let metadata: SkillMetadata = serde_yaml::from_str(metadata_str)?;
        
        // Validate metadata
        validate_metadata(&metadata)?;

        Ok(Skill {
            metadata,
            instructions: instructions.to_string(),
            resources: Vec::new(), // Initialized as empty, to be populated by filesystem scan
            path,
        })
    }
}

/// Splits the YAML frontmatter from the markdown content.
fn split_frontmatter(content: &str) -> anyhow::Result<(&str, &str)> {
    if !content.starts_with("---") {
        return Err(anyhow::anyhow!("Missing YAML frontmatter (no starting '---')"));
    }

    let mid = content[3..]
        .find("---")
        .ok_or_else(|| anyhow::anyhow!("Missing YAML frontmatter (no closing '---')"))?;
    
    let frontmatter = &content[3..mid + 3].trim();
    let instructions = &content[mid + 6..].trim();

    Ok((frontmatter, instructions))
}

/// Validates Level 1 metadata according to industry standards.
fn validate_metadata(metadata: &SkillMetadata) -> anyhow::Result<()> {
    // Name validation
    if metadata.name.len() > 64 {
        return Err(anyhow::anyhow!("Skill name exceeds 64 characters"));
    }
    
    if metadata.name.to_lowercase().contains("anthropic") || metadata.name.to_lowercase().contains("claude") {
        return Err(anyhow::anyhow!("Skill name cannot contain reserved words 'anthropic' or 'claude'"));
    }

    // Relaxed validation to allow more natural names, but keep them within limits
    if metadata.name.is_empty() {
        return Err(anyhow::anyhow!("Skill name cannot be empty"));
    }

    // Description validation
    if metadata.description.is_empty() {
        return Err(anyhow::anyhow!("Skill description cannot be empty"));
    }

    if metadata.description.len() > 1024 {
        return Err(anyhow::anyhow!("Skill description exceeds 1024 characters"));
    }

    Ok(())
}
