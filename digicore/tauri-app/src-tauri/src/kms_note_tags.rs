//! Read-only parse of `tags` from YAML frontmatter (first `---` ... `---` block).

/// Supports `tags: [a, b]`, `tags: a`, or block list under `tags:` with `- item` lines.
pub fn parse_tags_from_note_markdown(content: &str) -> Vec<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Vec::new();
    }
    let rest = trimmed[3..].trim_start();
    let end = match rest.find("\n---") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let fm = &rest[..end];
    let mut out = Vec::new();
    let mut in_tags = false;
    for line in fm.lines() {
        let tline = line.trim();
        if tline.is_empty() || tline.starts_with('#') {
            continue;
        }
        if in_tags {
            if tline.starts_with('-') {
                let v = tline
                    .trim_start_matches(|c| c == '-' || c == ' ')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !v.is_empty() {
                    out.push(v.to_string());
                }
            } else if tline.contains(':') && !tline.starts_with('-') {
                in_tags = false;
            }
        }
        if !in_tags && tline.starts_with("tags:") {
            let after = tline["tags:".len()..].trim();
            if after.is_empty() {
                in_tags = true;
            } else if after.starts_with('[') && after.ends_with(']') {
                let inner = &after[1..after.len().saturating_sub(1)];
                for part in inner.split(',') {
                    let s = part.trim().trim_matches('"').trim_matches('\'');
                    if !s.is_empty() {
                        out.push(s.to_string());
                    }
                }
            } else {
                out.push(after.trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_array() {
        let md = "---\ntags: [work, review]\n---\n# Hi";
        let t = parse_tags_from_note_markdown(md);
        assert_eq!(t, vec!["review", "work"]);
    }

    #[test]
    fn parses_block_list() {
        let md = "---\ntags:\n  - alpha\n  - beta\n---\n";
        let t = parse_tags_from_note_markdown(md);
        assert_eq!(t, vec!["alpha", "beta"]);
    }
}
