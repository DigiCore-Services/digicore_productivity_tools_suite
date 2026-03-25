use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillScope {
    Global,
    Project,
}

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
    
    // New Advanced Fields
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub compatibility: Option<String>,
    #[serde(default)]
    pub extra_metadata: Option<serde_json::Value>,
    #[serde(rename = "disable-model-invocation", default)]
    pub disable_model_invocation: Option<bool>,
    #[serde(default = "default_scope")]
    pub scope: SkillScope,
    #[serde(default, rename = "sync-targets")]
    pub sync_targets: Vec<String>,
}

fn default_scope() -> SkillScope {
    SkillScope::Global
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
    Folder,
    Other,
}

impl Skill {
    /// Generates the full SKILL.md content with YAML frontmatter.
    pub fn to_markdown(&self) -> anyhow::Result<String> {
        let mut yaml = String::new();
        yaml.push_str(&format!("name: {}\n", self.metadata.name));
        yaml.push_str(&format!("description: {}\n", self.metadata.description));
        if let Some(v) = &self.metadata.version {
            yaml.push_str(&format!("version: {}\n", v));
        }
        if let Some(a) = &self.metadata.author {
            yaml.push_str(&format!("author: {}\n", a));
        }
        if !self.metadata.tags.is_empty() {
            yaml.push_str("tags:\n");
            for tag in &self.metadata.tags {
                yaml.push_str(&format!("  - {}\n", tag));
            }
        }
        if let Some(l) = &self.metadata.license {
            yaml.push_str(&format!("license: {}\n", l));
        }
        if let Some(c) = &self.metadata.compatibility {
            yaml.push_str(&format!("compatibility: {}\n", c));
        }
        if let Some(extra) = &self.metadata.extra_metadata {
            let extra_yaml = serde_yaml::to_string(extra)?;
            let extra_trimmed = if extra_yaml.starts_with("---\n") { &extra_yaml[4..] } else { &extra_yaml };
            yaml.push_str("extra_metadata:\n");
            for line in extra_trimmed.lines() {
                yaml.push_str(&format!("  {}\n", line));
            }
        }
        if let Some(disable) = self.metadata.disable_model_invocation {
            yaml.push_str(&format!("disable-model-invocation: {}\n", disable));
        }
        if !self.metadata.sync_targets.is_empty() {
            yaml.push_str("sync-targets:\n");
            for target in &self.metadata.sync_targets {
                yaml.push_str(&format!("  - {}\n", target));
            }
        }
        yaml.push_str(&format!("scope: {:?}\n", self.metadata.scope));

        Ok(format!("---\n{}---\n\n{}", yaml, self.instructions))
    }

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

        let mut skill = Skill {
            metadata,
            instructions: instructions.to_string(),
            resources: Vec::new(),
            path,
        };
        
        // Auto-populate resources from the directory
        let _ = skill.refresh_resources();
        
        Ok(skill)
    }

    /// Scans the skill directory recursively for associated resources (scripts, assets, references).
    pub fn refresh_resources(&mut self) -> anyhow::Result<()> {
        self.resources.clear();
        if !self.path.exists() || !self.path.is_dir() {
            return Ok(());
        }

        let mut resources = Vec::new();
        self.scan_dir(&self.path, &mut resources)?;
        self.resources = resources;
        
        // Sort for consistent UI display (by name, then by path depth)
        self.resources.sort_by(|a, b| {
            let name_cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if name_cmp == std::cmp::Ordering::Equal {
                a.path.cmp(&b.path)
            } else {
                name_cmp
            }
        });
        
        Ok(())
    }

    fn scan_dir(&self, dir: &std::path::Path, resources: &mut Vec<Resource>) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip SKILL.md (case-insensitive) and hidden files/folders
            if name.to_uppercase() == "SKILL.MD" || name.starts_with('.') {
                continue;
            }

            // Skip common build/vcs artifacts if they somehow end up here
            if name == "node_modules" || name == "target" || name == "dist" || name == "build" {
                continue;
            }

            if path.is_dir() {
                // Add the directory itself as a resource
                resources.push(Resource {
                    name: name.clone(),
                    r#type: ResourceType::Folder,
                    path: path.clone(),
                });
                
                // Recurse into subdirectories
                self.scan_dir(&path, resources)?;
            } else {
                // It's a file, determine its type
                let r_type = self.determine_resource_type(&path);
                resources.push(Resource {
                    name,
                    r#type: r_type,
                    path,
                });
            }
        }
        Ok(())
    }

    fn determine_resource_type(&self, path: &std::path::Path) -> ResourceType {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase());

        // First check parent directory for clues (standard folders)
        if let Some(parent) = path.parent() {
            if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
                match parent_name.to_lowercase().as_str() {
                    "scripts" => return ResourceType::Script,
                    "references" | "docs" => return ResourceType::Reference,
                    "assets" | "resources" | "templates" => return ResourceType::Template,
                    _ => {}
                }
            }
        }

        // Fallback to extension-based detection
        match ext.as_deref() {
            Some("sh") | Some("py") | Some("js") | Some("ts") | Some("ps1") | Some("bat") | Some("lua") | Some("rb") => ResourceType::Script,
            Some("md") | Some("txt") | Some("pdf") | Some("html") | Some("docx") => ResourceType::Reference,
            Some("json") | Some("yaml") | Some("yml") | Some("xml") | Some("csv") | Some("toml") => ResourceType::Template,
            _ => {
                // Check filename prefixes for some common conventions
                if name.starts_with("test") || name.contains("example") {
                    ResourceType::Reference
                } else {
                    ResourceType::Other
                }
            }
        }
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
    // Name validation: Lowercase letters, numbers, and hyphens only.
    if metadata.name.is_empty() {
        return Err(anyhow::anyhow!("Skill name cannot be empty"));
    }

    if metadata.name.len() > 64 {
        return Err(anyhow::anyhow!("Skill name exceeds 64 characters"));
    }

    let is_valid_name = metadata.name.chars().all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    });

    if !is_valid_name {
        return Err(anyhow::anyhow!("Skill name must contain only lowercase letters, numbers, and hyphens (e.g., 'my-awesome-skill')"));
    }
    
    if metadata.name.contains("anthropic") || metadata.name.contains("claude") {
        return Err(anyhow::anyhow!("Skill name cannot contain reserved words 'anthropic' or 'claude'"));
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
