use digicore_core::domain::entities::skill::{Skill, SkillMetadata};
use std::path::PathBuf;

#[test]
fn test_parse_valid_skill() {
    let content = r#"---
name: test-skill
description: A valid test skill description.
version: 1.0.0
---
# Test Skill
These are the instructions.
"#;
    let path = PathBuf::from("/tmp/test-skill");
    let result = Skill::from_markdown(content.to_string(), path.clone());
    
    assert!(result.is_ok());
    let skill = result.unwrap();
    assert_eq!(skill.metadata.name, "test-skill");
    assert_eq!(skill.metadata.description, "A valid test skill description.");
    assert_eq!(skill.metadata.version, Some("1.0.0".to_string()));
    assert_eq!(skill.instructions, "# Test Skill\nThese are the instructions.");
    assert_eq!(skill.path, path);
}

#[test]
fn test_parse_skill_no_frontmatter() {
    let content = "# No Frontmatter\nJust content.";
    let result = Skill::from_markdown(content.to_string(), PathBuf::from("."));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Missing YAML frontmatter (no starting '---')");
}

#[test]
fn test_parse_skill_invalid_yaml() {
    let content = r#"---
name: test-skill
description: : invalid yaml
---
content
"#;
    let result = Skill::from_markdown(content.to_string(), PathBuf::from("."));
    assert!(result.is_err());
}

#[test]
fn test_validate_name_length() {
    let long_name = "a".repeat(65);
    let content = format!(r#"---
name: {}
description: desc
---
content
"#, long_name);
    let result = Skill::from_markdown(content, PathBuf::from("."));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("exceeds 64 characters"));
}

#[test]
fn test_validate_name_reserved_words() {
    let content = r#"---
name: my-anthropic-skill
description: desc
---
content
"#;
    let result = Skill::from_markdown(content.to_string(), PathBuf::from("."));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot contain reserved words"));
}

#[test]
fn test_validate_name_characters() {
    let content = r#"---
name: Invalid_Name!
description: desc
---
content
"#;
    let result = Skill::from_markdown(content.to_string(), PathBuf::from("."));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be lowercase alphanumeric"));
}

#[test]
fn test_validate_empty_description() {
    let content = r#"---
name: test-skill
description: ""
---
content
"#;
    let result = Skill::from_markdown(content.to_string(), PathBuf::from("."));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("description cannot be empty"));
}

#[test]
fn test_validate_description_length() {
    let long_desc = "a".repeat(1025);
    let content = format!(r#"---
name: test-skill
description: {}
---
content
"#, long_desc);
    let result = Skill::from_markdown(content, PathBuf::from("."));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("description exceeds 1024 characters"));
}
