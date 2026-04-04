//! D6: background re-embed indexed notes when the configured embedding model id or chunk policy changes.

use crate::embedding_pipeline;
use crate::kms_embed_diagnostic_log::{
    append_file_only, debug_file_if_enabled, error_emit, session_d6_start, warn_emit,
    KMS_EMBED_LOG_TARGET,
};
use crate::kms_repository;
use digicore_kms_ports::KmsTextEmbeddingChunkConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

static MIGRATION_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Cap how many per-note failures we ship in each progress event (UI + localStorage).
const MIGRATE_FAILURE_LIST_CAP: usize = 200;

#[derive(Clone, serde::Serialize)]
pub struct KmsEmbeddingMigrateFailure {
    pub path: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct KmsEmbeddingMigrateProgressPayload {
    pub generation: u64,
    pub phase: String,
    pub done: u32,
    pub total: u32,
    pub current_path: Option<String>,
    pub failed: u32,
    /// Milliseconds since this job started (for client ETA).
    pub elapsed_ms: u64,
    /// Human-readable status (empty DB, all fresh, errors, summary).
    pub detail: Option<String>,
    /// Per-note failures recorded so far this job (capped); for health reports in the UI.
    pub failures: Vec<KmsEmbeddingMigrateFailure>,
    /// True when `failed` exceeds the number of rows in `failures` (list was capped).
    pub failures_truncated: bool,
}

fn emit_progress(app: &AppHandle, payload: KmsEmbeddingMigrateProgressPayload) {
    let _ = app.emit("kms-embedding-migrate-progress", &payload);
}

fn push_failure(acc: &mut Vec<KmsEmbeddingMigrateFailure>, path: String, message: String) {
    if acc.len() >= MIGRATE_FAILURE_LIST_CAP {
        return;
    }
    acc.push(KmsEmbeddingMigrateFailure { path, message });
}

fn progress_payload(
    gen: u64,
    phase: &str,
    done: u32,
    total: u32,
    current_path: Option<String>,
    failed: u32,
    started: Instant,
    detail: Option<String>,
    failures: &[KmsEmbeddingMigrateFailure],
) -> KmsEmbeddingMigrateProgressPayload {
    KmsEmbeddingMigrateProgressPayload {
        generation: gen,
        phase: phase.to_string(),
        done,
        total,
        current_path,
        failed,
        elapsed_ms: started.elapsed().as_millis() as u64,
        detail,
        failures: failures.to_vec(),
        failures_truncated: failed as usize > failures.len(),
    }
}

/// Increment generation so the current background job exits with phase `cancelled` (no new work scheduled).
pub fn bump_embedding_migration_generation() {
    let n = MIGRATION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    log::info!("[KMS][D6] cancel requested: bumped migration generation to {}", n);
}

/// Starts a background job; returns a generation id (newer jobs invalidate older ones).
///
/// `force_reembed_all_indexed`: when **true** (Config **Re-embed vault now**), every indexed note is re-embedded.
/// When **false** (automatic queue after model/chunk change), only notes whose stored model/fingerprint differ.
pub fn spawn_note_embedding_migration(
    app: &AppHandle,
    vault: PathBuf,
    target_model_id: String,
    batch: u32,
    chunk_cfg: KmsTextEmbeddingChunkConfig,
    force_reembed_all_indexed: bool,
) -> u64 {
    let gen = MIGRATION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    let batch = batch.max(1);
    let c = chunk_cfg.clamped();
    let target_policy_sig = kms_repository::note_embedding_policy_sig(
        &target_model_id,
        c.enabled,
        c.max_chars,
        c.overlap_chars,
        kms_repository::KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS,
    );
    let sig_preview = if target_policy_sig.len() > 120 {
        format!("{}...", &target_policy_sig[..120])
    } else {
        target_policy_sig.clone()
    };
    log::info!(
        "[KMS][D6] spawn gen={} force_full_vault={} vault={} model={} chunk={{ enabled:{} max_chars:{} overlap:{} }} target_policy_sig_preview={}",
        gen,
        force_reembed_all_indexed,
        vault.display(),
        target_model_id,
        c.enabled,
        c.max_chars,
        c.overlap_chars,
        sig_preview
    );
    log::info!(
        target: KMS_EMBED_LOG_TARGET,
        "[KMS][D6] Verbose KMS embedding logs: set RUST_LOG=kms_embed=debug (or kms_embed=trace). Failures also emit WARN on this target."
    );
    session_d6_start(
        gen,
        &vault,
        &target_model_id,
        c.enabled,
        c.max_chars,
        c.overlap_chars,
        &sig_preview,
    );
    append_file_only(
        "INFO",
        "session",
        "Also: general app logs may be under the Tauri log dir (see tauri-plugin-log). This file is KMS embedding failures + session markers only.",
    );
    tokio::spawn(async move {
        let started = Instant::now();
        let total = if force_reembed_all_indexed {
            match tokio::task::spawn_blocking(|| kms_repository::count_indexed_notes()).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    error_emit(
                        "D6",
                        format!("gen={} count_indexed_notes failed: {}", gen, e),
                    );
                    emit_progress(
                        &app,
                        progress_payload(
                            gen,
                            "error",
                            0,
                            0,
                            None,
                            0,
                            started,
                            Some(format!("Database error (count notes): {}", e)),
                            &[],
                        ),
                    );
                    return;
                }
                Err(e) => {
                    log::error!("[KMS][D6] gen={} count_indexed_notes join failed: {}", gen, e);
                    emit_progress(
                        &app,
                        progress_payload(
                            gen,
                            "error",
                            0,
                            0,
                            None,
                            0,
                            started,
                            Some(format!("Task error: {}", e)),
                            &[],
                        ),
                    );
                    return;
                }
            }
        } else {
            match tokio::task::spawn_blocking({
                let tid = target_model_id.clone();
                let sig = target_policy_sig.clone();
                move || kms_repository::count_notes_needing_embedding_migration(&tid, &sig)
            })
            .await
            {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    error_emit(
                        "D6",
                        format!(
                            "gen={} count_notes_needing_embedding_migration failed: {}",
                            gen, e
                        ),
                    );
                    emit_progress(
                        &app,
                        progress_payload(
                            gen,
                            "error",
                            0,
                            0,
                            None,
                            0,
                            started,
                            Some(format!("Database error (stale count): {}", e)),
                            &[],
                        ),
                    );
                    return;
                }
                Err(e) => {
                    error_emit(
                        "D6",
                        format!("gen={} stale count join failed: {}", gen, e),
                    );
                    emit_progress(
                        &app,
                        progress_payload(
                            gen,
                            "error",
                            0,
                            0,
                            None,
                            0,
                            started,
                            Some(format!("Task error: {}", e)),
                            &[],
                        ),
                    );
                    return;
                }
            }
        };

        let start_detail = if force_reembed_all_indexed {
            Some(format!(
                "Re-embedding all {} indexed note(s) (manual full vault).",
                total
            ))
        } else if total == 0 {
            Some(
                "No stale notes (all indexed rows match current model and embedding policy)."
                    .to_string(),
            )
        } else {
            Some(format!(
                "Re-embedding {} note(s) with stale model or policy fingerprint.",
                total
            ))
        };

        emit_progress(
            &app,
            progress_payload(
                gen,
                "starting",
                0,
                total,
                None,
                0,
                started,
                start_detail,
                &[],
            ),
        );

        if total == 0 {
            let phase = if force_reembed_all_indexed {
                "nothing_to_do"
            } else {
                "complete"
            };
            let msg = if force_reembed_all_indexed {
                "No indexed notes found. Open KMS and sync the vault, or check that notes are not all in failed state."
            } else {
                "Nothing to do: embeddings already match settings."
            };
            log::info!("[KMS][D6] gen={} {} — {}", gen, phase, msg);
            emit_progress(
                &app,
                progress_payload(
                    gen,
                    phase,
                    0,
                    0,
                    None,
                    0,
                    started,
                    Some(msg.to_string()),
                    &[],
                ),
            );
            return;
        }

        let mut done: u32 = 0;
        let mut failed: u32 = 0;
        let mut failures_acc: Vec<KmsEmbeddingMigrateFailure> = Vec::new();
        // SQL OFFSET for full-vault listing (without this, every loop repeats the same LIMIT rows).
        let mut full_vault_list_offset: u32 = 0;

        loop {
            if MIGRATION_GENERATION.load(Ordering::SeqCst) != gen {
                log::info!(
                    "[KMS][D6] gen={} cancelled after {} ok, {} failed (of {} target)",
                    gen, done, failed, total
                );
                emit_progress(
                    &app,
                    progress_payload(
                        gen,
                        "cancelled",
                        done,
                        total,
                        None,
                        failed,
                        started,
                        Some(format!(
                            "Stopped by user: {} note(s) updated, {} failed.",
                            done, failed
                        )),
                        &failures_acc,
                    ),
                );
                return;
            }

            let paths: Vec<String> = if force_reembed_all_indexed {
                let off = full_vault_list_offset;
                match tokio::task::spawn_blocking(move || {
                    kms_repository::list_indexed_note_paths(batch, off)
                })
                .await
                {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        log::error!("[KMS][D6] gen={} list_indexed_note_paths failed: {}", gen, e);
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "error",
                                done,
                                total,
                                None,
                                failed,
                                started,
                                Some(format!("Database error (list paths): {}", e)),
                                &failures_acc,
                            ),
                        );
                        return;
                    }
                    Err(e) => {
                        error_emit(
                            "D6",
                            format!("gen={} list paths join failed: {}", gen, e),
                        );
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "error",
                                done,
                                total,
                                None,
                                failed,
                                started,
                                Some(format!("Task error (list paths): {}", e)),
                                &failures_acc,
                            ),
                        );
                        return;
                    }
                }
            } else {
                match tokio::task::spawn_blocking({
                    let tid = target_model_id.clone();
                    let sig = target_policy_sig.clone();
                    move || kms_repository::list_note_paths_for_embedding_migration(&tid, &sig, batch)
                })
                .await
                {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        log::error!(
                            "[KMS][D6] gen={} list_note_paths_for_embedding_migration failed: {}",
                            gen,
                            e
                        );
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "error",
                                done,
                                total,
                                None,
                                failed,
                                started,
                                Some(format!("Database error (list stale): {}", e)),
                                &failures_acc,
                            ),
                        );
                        return;
                    }
                    Err(e) => {
                        error_emit(
                            "D6",
                            format!("gen={} list stale join failed: {}", gen, e),
                        );
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "error",
                                done,
                                total,
                                None,
                                failed,
                                started,
                                Some(format!("Task error (list stale): {}", e)),
                                &failures_acc,
                            ),
                        );
                        return;
                    }
                }
            };

            if paths.is_empty() {
                append_file_only(
                    "INFO",
                    "session",
                    &format!(
                        "D6 gen={} batch loop finished: done={} failed={} target_total={}",
                        gen, done, failed, total
                    ),
                );
                log::info!(
                    "[KMS][D6] gen={} complete: {} ok, {} failed (target was {})",
                    gen,
                    done,
                    failed,
                    total
                );
                emit_progress(
                    &app,
                    progress_payload(
                        gen,
                        "complete",
                        done,
                        total,
                        None,
                        failed,
                        started,
                        Some(format!(
                            "Finished: {} note(s) re-embedded, {} failed.",
                            done, failed
                        )),
                        &failures_acc,
                    ),
                );
                return;
            }

            let batch_len = paths.len() as u32;
            for rel_path in paths {
                if MIGRATION_GENERATION.load(Ordering::SeqCst) != gen {
                    log::info!(
                        "[KMS][D6] gen={} cancelled mid-batch after {} ok",
                        gen, done
                    );
                    emit_progress(
                        &app,
                        progress_payload(
                            gen,
                            "cancelled",
                            done,
                            total,
                            None,
                            failed,
                            started,
                            Some(format!(
                                "Stopped by user: {} note(s) updated, {} failed.",
                                done, failed
                            )),
                            &failures_acc,
                        ),
                    );
                    return;
                }

                let abs = vault.join(&rel_path);
                let abs_display = abs.display().to_string();
                let rel_path_clone = rel_path.clone();
                let content = match tokio::task::spawn_blocking(move || std::fs::read_to_string(abs)).await {
                    Ok(Ok(s)) => s,
                    Ok(Err(e)) => {
                        failed = failed.saturating_add(1);
                        let msg = format!("Read failed: {}", e);
                        push_failure(
                            &mut failures_acc,
                            rel_path_clone.clone(),
                            msg.clone(),
                        );
                        log::warn!(
                            target: KMS_EMBED_LOG_TARGET,
                            "[KMS][D6] gen={} read FAILED rel_path={} abs_path={} io_kind={:?} err={}",
                            gen,
                            rel_path_clone,
                            abs_display,
                            e.kind(),
                            e
                        );
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "batch",
                                done,
                                total,
                                Some(rel_path_clone.clone()),
                                failed,
                                started,
                                Some(msg),
                                &failures_acc,
                            ),
                        );
                        continue;
                    }
                    Err(e) => {
                        failed = failed.saturating_add(1);
                        let msg = format!("Read task join: {}", e);
                        push_failure(
                            &mut failures_acc,
                            rel_path_clone.clone(),
                            msg.clone(),
                        );
                        warn_emit(
                            "D6",
                            format!(
                                "gen={} read task join FAILED rel_path={} abs_path={} err={}",
                                gen, rel_path_clone, abs_display, e
                            ),
                        );
                        emit_progress(
                            &app,
                            progress_payload(
                                gen,
                                "batch",
                                done,
                                total,
                                Some(rel_path_clone.clone()),
                                failed,
                                started,
                                Some(msg),
                                &failures_acc,
                            ),
                        );
                        continue;
                    }
                };

                let chunk = chunk_cfg.clamped();
                let embed_start_msg = format!(
                    "gen={} embed_start rel_path={} content_chars={} content_bytes={} model_id={} vault_root={} chunk enabled={} max={} overlap={}",
                    gen,
                    rel_path,
                    content.chars().count(),
                    content.len(),
                    target_model_id,
                    vault.display(),
                    chunk.enabled,
                    chunk.max_chars,
                    chunk.overlap_chars
                );
                log::debug!(
                    target: KMS_EMBED_LOG_TARGET,
                    "[KMS][D6] {}",
                    embed_start_msg
                );
                debug_file_if_enabled("D6", &embed_start_msg);

                let rel = rel_path.clone();
                let mid = target_model_id.clone();
                let embed_outcome = tokio::task::spawn_blocking(move || {
                    embedding_pipeline::embed_note_text_blocking(&rel, &content, &mid, &chunk)
                })
                .await;

                let mut batch_progress_detail: Option<String> = None;
                match embed_outcome {
                    Ok(Ok(())) => {
                        done = done.saturating_add(1);
                        if done == 1 || done % 25 == 0 || done == total {
                            log::info!(
                                "[KMS][D6] gen={} progress {}/{} (failed {})",
                                gen,
                                done,
                                total,
                                failed
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        failed = failed.saturating_add(1);
                        let err_s = e.clone();
                        batch_progress_detail = Some(format!("Last error: {}", e));
                        push_failure(&mut failures_acc, rel_path.clone(), err_s);
                        warn_emit(
                            "D6",
                            format!(
                                "gen={} embed FAILED rel_path={} model_id={} err={}",
                                gen, rel_path, target_model_id, e
                            ),
                        );
                        let ctx = format!(
                            "gen={} embed FAILED context vault={} policy_sig_expected_prefix={}",
                            gen,
                            vault.display(),
                            if target_policy_sig.len() > 80 {
                                format!("{}...", &target_policy_sig[..80])
                            } else {
                                target_policy_sig.clone()
                            }
                        );
                        log::debug!(target: KMS_EMBED_LOG_TARGET, "[KMS][D6] {}", ctx);
                        debug_file_if_enabled("D6", &ctx);
                    }
                    Err(e) => {
                        failed = failed.saturating_add(1);
                        let err_s = e.to_string();
                        batch_progress_detail = Some(format!("Embed task join: {}", e));
                        push_failure(&mut failures_acc, rel_path.clone(), err_s);
                        warn_emit(
                            "D6",
                            format!(
                                "gen={} embed spawn_blocking join FAILED rel_path={} err={}",
                                gen, rel_path, e
                            ),
                        );
                    }
                }

                emit_progress(
                    &app,
                    progress_payload(
                        gen,
                        "batch",
                        done,
                        total,
                        Some(rel_path),
                        failed,
                        started,
                        batch_progress_detail,
                        &failures_acc,
                    ),
                );
            }

            if force_reembed_all_indexed {
                full_vault_list_offset = full_vault_list_offset.saturating_add(batch_len);
            }

            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    gen
}
