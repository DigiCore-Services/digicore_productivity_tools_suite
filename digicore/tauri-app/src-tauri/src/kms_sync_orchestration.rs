//! KMS filesystem-to-database sync, per-note indexing, link extraction, and post-sync graph jobs.
//! Not an IPC layer; used from lifecycle, vault path, watcher, reindex, indexing providers, and config.

use digicore_text_expander::application::app_state::AppState;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::kms_ipc_boundary::kms_request_id;
use crate::embedding_service;
use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_service;
use crate::kms_repository;

pub(crate) async fn sync_note_index_internal(
    rel_path_raw: &str,
    title: &str,
    content: &str,
    embedding_model_id: &str,
    chunk_cfg: digicore_kms_ports::KmsTextEmbeddingChunkConfig,
) -> Result<(), String> {
    let request_id = kms_request_id("sync_note_index");
    let rel_path = kms_graph_service::norm_vault_rel_path(rel_path_raw);
    let preview = content.chars().take(200).collect::<String>();
    let tags = crate::kms_note_tags::parse_tags_from_note_markdown(content);

    KmsDiagnosticService::debug(&format!("Indexing note: {}", rel_path), None);

    kms_repository::upsert_note(&rel_path, title, &preview, "indexed", None, &tags).map_err(|e| e.to_string())?;

    if let Err(e) = kms_repository::delete_links_for_source(&rel_path) {
        log::warn!(
            "[KMS][Sync] event_code=KMS_SYNC_DELETE_LINKS_WARN request_id={} path={} error={}",
            request_id,
            rel_path,
            e
        );
    }
    let candidates = extract_links_from_markdown(content);

    if !candidates.is_empty() {
        if let Ok(all_notes) = kms_repository::list_notes() {
            let title_map: HashMap<String, String> = all_notes
                .iter()
                .map(|n| {
                    (
                        n.title.to_lowercase(),
                        kms_graph_service::norm_vault_rel_path(&n.path),
                    )
                })
                .collect();
            let path_map: HashSet<String> = all_notes
                .iter()
                .map(|n| kms_graph_service::norm_vault_rel_path(&n.path))
                .collect();

            let source_path = PathBuf::from(&rel_path);
            let source_parent = source_path.parent().unwrap_or(Path::new(""));

            for candidate in candidates {
                match candidate {
                    LinkCandidate::Wiki(target_title) => {
                        if let Some(target_path) = title_map.get(&target_title.to_lowercase()) {
                            if let Err(e) = kms_repository::upsert_link(&rel_path, target_path) {
                                log::warn!(
                                    "[KMS][Sync] event_code=KMS_SYNC_UPSERT_WIKI_LINK_WARN request_id={} source={} target={} error={}",
                                    request_id,
                                    rel_path,
                                    target_path,
                                    e
                                );
                            }
                        }
                    }
                    LinkCandidate::Path(mut target_path_str) => {
                        if let Some(hash_idx) = target_path_str.find('#') {
                            target_path_str.truncate(hash_idx);
                        }

                        let target_path_str = target_path_str.replace('\\', "/");
                        if target_path_str.is_empty() {
                            continue;
                        }

                        let resolved_path = if target_path_str.starts_with("./")
                            || target_path_str.starts_with("../")
                        {
                            source_parent
                                .join(&target_path_str)
                                .components()
                                .fold(PathBuf::new(), |mut acc, comp| {
                                    match comp {
                                        std::path::Component::CurDir => {}
                                        std::path::Component::ParentDir => {
                                            acc.pop();
                                        }
                                        std::path::Component::Normal(c) => {
                                            acc.push(c);
                                        }
                                        _ => {
                                            acc.push(comp);
                                        }
                                    }
                                    acc
                                })
                        } else {
                            PathBuf::from(&target_path_str)
                        };

                        let resolved_str = resolved_path.to_string_lossy().replace('\\', "/");
                        let resolved_norm = kms_graph_service::norm_vault_rel_path(&resolved_str);

                        if path_map.contains(&resolved_norm) {
                            if let Err(e) = kms_repository::upsert_link(&rel_path, &resolved_norm) {
                                log::warn!(
                                    "[KMS][Sync] event_code=KMS_SYNC_UPSERT_PATH_LINK_WARN request_id={} source={} target={} error={}",
                                    request_id,
                                    rel_path,
                                    resolved_norm,
                                    e
                                );
                            }
                        } else {
                            let stem = resolved_path
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string().to_lowercase())
                                .unwrap_or_default();
                            if let Some(target_path) = title_map.get(&stem) {
                                if let Err(e) = kms_repository::upsert_link(&rel_path, target_path) {
                                    log::warn!(
                                        "[KMS][Sync] event_code=KMS_SYNC_UPSERT_PATH_FALLBACK_LINK_WARN request_id={} source={} target={} error={}",
                                        request_id,
                                        rel_path,
                                        target_path,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let content_to_embed = content.to_string();
    let rel_path_clone = rel_path.clone();
    let model_clone = embedding_model_id.to_string();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = crate::embedding_pipeline::embed_note_text_blocking(
            &rel_path_clone,
            &content_to_embed,
            &model_clone,
            &chunk_cfg,
        ) {
            log::warn!("[KMS][Embed] note {}: {}", rel_path_clone, e);
        }
    });

    crate::kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();

    Ok(())
}

pub(crate) enum LinkCandidate {
    Wiki(String),
    Path(String),
}

pub(crate) fn extract_links_from_markdown(content: &str) -> Vec<LinkCandidate> {
    let mut candidates = Vec::new();

    let wiki_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
    for cap in wiki_re.captures_iter(content) {
        candidates.push(LinkCandidate::Wiki(cap[1].trim().to_string()));
    }

    let md_re = Regex::new(r"(?i)\[[^\]]+\]\(([^\)]+)\)").unwrap();
    for cap in md_re.captures_iter(content) {
        let path = cap[1].trim().to_string();
        if !path.starts_with("http") && !path.starts_with("mailto:") {
            candidates.push(LinkCandidate::Path(path));
        }
    }

    candidates
}

static KMS_GRAPH_PR_ON_SETTINGS_DEBOUNCE_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn schedule_debounced_background_wiki_pagerank_on_settings(app: &AppHandle, vault: PathBuf) {
    let gen = KMS_GRAPH_PR_ON_SETTINGS_DEBOUNCE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let app_c = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        if KMS_GRAPH_PR_ON_SETTINGS_DEBOUNCE_GEN.load(Ordering::SeqCst) != gen {
            return;
        }
        spawn_background_wiki_pagerank_after_vault_sync(&app_c, &vault);
    });
}

fn spawn_background_wiki_pagerank_after_vault_sync(app: &AppHandle, vault_path: &Path) {
    let vault_buf = vault_path.to_path_buf();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let (skip, off_scope, bg_disabled, pr_it, pr_d) = {
            let state = app_clone.state::<Arc<Mutex<AppState>>>();
            let guard = match state.lock() {
                Ok(g) => g,
                Err(e) => {
                    log::warn!(
                        "[KMS][Graph] background wiki PageRank: state lock poisoned: {}",
                        e
                    );
                    return;
                }
            };
            let p = crate::kms_graph_effective_params::effective_graph_build_params(&*guard, &vault_buf);
            let off_scope = kms_graph_service::resolve_pagerank_scope(&p.pagerank_scope, false)
                == kms_graph_service::KmsPagerankScopeMode::Off;
            let bg_disabled = !p.background_wiki_pagerank_enabled;
            let skip = off_scope || bg_disabled;
            let pr_it = p.pagerank_iterations.max(4) as usize;
            let pr_d = p.pagerank_damping.clamp(0.5, 0.99);
            (skip, off_scope, bg_disabled, pr_it, pr_d)
        };
        if skip {
            if off_scope {
                log::debug!("[KMS][Graph] background wiki PageRank skipped (pagerank_scope=off)");
            } else if bg_disabled {
                log::debug!(
                    "[KMS][Graph] background wiki PageRank skipped (effective background_wiki_pagerank_enabled=false)"
                );
            }
            return;
        }
        log::info!(
            "[KMS][Graph] background wiki PageRank started after vault sync (iters={} damping={:.2})",
            pr_it,
            pr_d
        );
        let t0 = std::time::Instant::now();
        let res = tokio::task::spawn_blocking(move || {
            kms_graph_service::materialize_wiki_pagerank_full_vault(&vault_buf, pr_it, pr_d)
        })
        .await;
        match res {
            Ok(Ok(n)) => {
                if n > 0 {
                    log::info!(
                        "[KMS][Graph] background wiki PageRank finished: {} nodes in {}ms",
                        n,
                        t0.elapsed().as_millis()
                    );
                    if let Err(e) = app_clone.emit("kms-wiki-pagerank-ready", n as u64) {
                        log::warn!("[KMS][Graph] event_code=KMS_WIKI_PAGERANK_READY_EMIT_WARN error={}", e);
                    }
                } else {
                    log::debug!("[KMS][Graph] background wiki PageRank: no notes to score");
                }
            }
            Ok(Err(e)) => log::warn!("[KMS][Graph] background wiki PageRank failed: {}", e),
            Err(e) => log::warn!("[KMS][Graph] background wiki PageRank join: {}", e),
        }
    });
}

pub(crate) async fn sync_vault_files_to_db_internal(
    app: &tauri::AppHandle,
    vault_path: &Path,
) -> Result<(), String> {
    let request_id = kms_request_id("sync_vault");
    if !vault_path.exists() {
        return Ok(());
    }

    let (embedding_model_id, chunk_cfg) = {
        let st = app.state::<Arc<Mutex<AppState>>>();
        let g = st.lock().map_err(|e| e.to_string())?;
        (
            embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id),
            crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, vault_path),
        )
    };

    let db_notes = kms_repository::list_notes().map_err(|e| e.to_string())?;
    let mut db_paths: HashMap<String, (String, String, Option<String>)> = db_notes
        .into_iter()
        .map(|n| {
            let k = kms_graph_service::norm_vault_rel_path(&n.path);
            (k, (n.title, n.sync_status, n.last_modified))
        })
        .collect();

    let mut disk_files = Vec::new();
    fn scan_recursive(dir: &Path, root: &Path, files: &mut Vec<(PathBuf, String)>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_recursive(&path, root, files);
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
                    .unwrap_or(false)
                {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let rel_str = kms_graph_service::norm_vault_rel_path(
                            &rel.to_string_lossy().replace('\\', "/"),
                        );
                        files.push((path, rel_str));
                    }
                }
            }
        }
    }
    scan_recursive(vault_path, vault_path, &mut disk_files);

    for (abs_path, rel_path) in disk_files {
        let current_title = abs_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let db_info = db_paths.get(&rel_path);
        let status = db_info.map(|i| i.1.as_str()).unwrap_or("");
        let db_last_modified = db_info.and_then(|i| i.2.as_ref());

        let mut disk_newer = false;
        if let Ok(metadata) = abs_path.metadata() {
            if let Ok(modified) = metadata.modified() {
                let disk_time: chrono::DateTime<chrono::Utc> = modified.into();
                let disk_time_str = disk_time.to_rfc3339();
                if let Some(db_time_str) = db_last_modified {
                    if disk_time_str > *db_time_str {
                        log::info!(
                            "[KMS][Sync] External change detected for: {}. Disk: {}, DB: {}",
                            rel_path,
                            disk_time_str,
                            db_time_str
                        );
                        disk_newer = true;
                    }
                }
            }
        }

        let needs_index = db_info.is_none() || status == "failed" || status == "pending" || disk_newer;
        let needs_rename = db_info.map(|t| t.0 != current_title).unwrap_or(false);

        if needs_index || needs_rename {
            KmsDiagnosticService::info(&format!("Syncing: {}", rel_path), None);

            if let Err(e) = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None, &[]) {
                log::warn!(
                    "[KMS][Sync] event_code=KMS_SYNC_VAULT_PENDING_UPSERT_WARN request_id={} path={} error={}",
                    request_id,
                    rel_path,
                    e
                );
            }

            match std::fs::read_to_string(&abs_path) {
                Ok(content) => {
                    if let Err(e) = sync_note_index_internal(
                        &rel_path,
                        &current_title,
                        &content,
                        &embedding_model_id,
                        chunk_cfg,
                    )
                    .await
                    {
                        KmsDiagnosticService::error(&format!("Failed to sync {}: {}", rel_path, e), None);
                    }
                }
                Err(e) => {
                    KmsDiagnosticService::error(&format!("Failed to read {}: {}", rel_path, e), None);
                    if let Err(upsert_err) =
                        kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e.to_string()), &[])
                    {
                        log::warn!(
                            "[KMS][Sync] event_code=KMS_SYNC_VAULT_FAILED_UPSERT_WARN request_id={} path={} error={}",
                            request_id,
                            rel_path,
                            upsert_err
                        );
                    }
                }
            }
        }

        db_paths.remove(&rel_path);
    }

    for (stale_rel_path, _) in db_paths {
        log::info!("[KMS][Sync] Cleaning up stale DB record: {}", stale_rel_path);
        if let Err(e) = kms_repository::delete_note(&stale_rel_path) {
            log::warn!(
                "[KMS][Sync] event_code=KMS_SYNC_VAULT_DELETE_STALE_NOTE_WARN request_id={} path={} error={}",
                request_id,
                stale_rel_path,
                e
            );
        }
    }

    crate::kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();

    spawn_background_wiki_pagerank_after_vault_sync(app, vault_path);

    Ok(())
}

pub(crate) async fn sync_single_note_to_db_internal(
    app: &tauri::AppHandle,
    abs_path: &Path,
) -> Result<(), String> {
    let request_id = kms_request_id("sync_single_note");
    let vault_path = kms_repository::get_vault_path().map_err(|e| e.to_string())?;
    let rel_path = abs_path
        .strip_prefix(&vault_path)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            format!(
                "Path {} is not in vault {}",
                abs_path.display(),
                vault_path.display()
            )
        })?;

    let (embedding_model_id, chunk_cfg) = {
        let st = app.state::<Arc<Mutex<AppState>>>();
        let g = st.lock().map_err(|e| e.to_string())?;
        (
            embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id),
            crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, &vault_path),
        )
    };

    let current_title = abs_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    if let Err(e) = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None, &[]) {
        log::warn!(
            "[KMS][Sync] event_code=KMS_SYNC_SINGLE_PENDING_UPSERT_WARN request_id={} path={} error={}",
            request_id,
            rel_path,
            e
        );
    }

    match std::fs::read_to_string(abs_path) {
        Ok(content) => {
            sync_note_index_internal(
                &rel_path,
                &current_title,
                &content,
                &embedding_model_id,
                chunk_cfg,
            )
            .await?;
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "[KMS][Sync] Failed to read {}: {}. Marking as failed.",
                rel_path,
                e
            );
            if let Err(upsert_err) =
                kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e.to_string()), &[])
            {
                log::warn!(
                    "[KMS][Sync] event_code=KMS_SYNC_SINGLE_FAILED_UPSERT_WARN request_id={} path={} error={}",
                    request_id,
                    rel_path,
                    upsert_err
                );
            }
            Err(e.to_string())
        }
    }
}
