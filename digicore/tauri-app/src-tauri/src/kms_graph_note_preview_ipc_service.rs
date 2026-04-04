//! Bounded service for note graph preview IPC orchestration.

use std::path::PathBuf;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_repository;

use super::ApiImpl;
use super::ipc_error;
use super::KmsNoteGraphPreviewDto;

fn graph_preview_excerpt(raw: &str, max: usize) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("```") || t == "---" {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(t);
        if out.len() >= max {
            break;
        }
    }
    out.chars().take(max).collect()
}

pub(crate) fn get_note_graph_preview_dto(
    host: ApiImpl,
    path: String,
    max_chars: u32,
    request_id: String,
) -> Result<KmsNoteGraphPreviewDto, String> {
    let max = (max_chars as usize).clamp(80, 4000);
    let path_buf = PathBuf::from(&path);
    let rel = host
        .get_relative_path(&path_buf)
        .map_err(|e| ipc_error("KMS_PATH_OUTSIDE_VAULT", e, None))?;
    let abs = host.resolve_absolute_path(&rel);
    let abs_s = abs.to_string_lossy().to_string();
    let row = kms_repository::get_note_by_path(&rel)
        .map_err(|e| ipc_error("KMS_REPO_NOTE", e.to_string(), None))?
        .ok_or_else(|| ipc_error("KMS_NOTE_NOT_INDEXED", "Note is not indexed", None))?;
    let title = row.title.clone();
    let last_modified = row.last_modified.clone();
    let mut excerpt: Option<String> = row
        .content_preview
        .as_ref()
        .map(|s| s.chars().take(max).collect::<String>())
        .filter(|s| !s.is_empty());
    let short = excerpt.as_ref().map(|s| s.len()).unwrap_or(0) < max / 4;
    if (excerpt.is_none() || short) && abs.exists() {
        if let Ok(raw) = std::fs::read_to_string(&abs) {
            let e = graph_preview_excerpt(&raw, max);
            if !e.is_empty() {
                excerpt = Some(e);
            }
        }
    }
    log::info!(
        "[KMS][Graph] note_preview request_id={} max_chars={} title_len={}",
        request_id,
        max,
        title.len()
    );
    KmsDiagnosticService::debug(
        &format!(
            "[KMS][Graph] note_preview request_id={} rel={} excerpt_len={}",
            request_id,
            rel,
            excerpt.as_ref().map(|s| s.len()).unwrap_or(0)
        ),
        None,
    );
    Ok(KmsNoteGraphPreviewDto {
        path: abs_s,
        title,
        excerpt: excerpt.unwrap_or_else(|| "(No preview available)".to_string()),
        last_modified,
        request_id,
    })
}

