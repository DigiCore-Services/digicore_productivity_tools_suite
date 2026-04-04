//! Snippet library persistence, path, and CRUD (hotstring sync + KMS snippet indexing).

use super::*;
use digicore_core::domain::Snippet;
use std::sync::Arc;

pub(crate) async fn load_library(host: ApiImpl) -> Result<u32, String> {
    let mut guard = host.state.lock().map_err(|e| e.to_string())?;
    let count = guard.try_load_library().map_err(|e| e.to_string())? as u32;
    update_library(guard.library.clone());
    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(count)
}

pub(crate) async fn save_library(host: ApiImpl) -> Result<(), String> {
    let mut guard = host.state.lock().map_err(|e| e.to_string())?;
    guard.try_save_library().map_err(|e| e.to_string())
}

pub(crate) async fn set_library_path(host: ApiImpl, path: String) -> Result<(), String> {
    let mut guard = host.state.lock().map_err(|e| e.to_string())?;
    guard.library_path = path;
    Ok(())
}

pub(crate) async fn add_snippet(host: ApiImpl, category: String, snippet: Snippet) -> Result<(), String> {
    let trigger = snippet.trigger.clone();
    {
        let mut guard = host.state.lock().map_err(|e| e.to_string())?;
        guard.add_snippet(&category, &snippet);
        update_library(guard.library.clone());
    }

    let handle = get_app(&host.app_handle);
    let service = handle
        .state::<Arc<crate::indexing_service::KmsIndexingService>>()
        .inner()
        .clone();
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        let _ = service
            .index_single_item(&handle_clone, "snippets", &trigger)
            .await;
    });

    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

pub(crate) async fn update_snippet(
    host: ApiImpl,
    category: String,
    snippet_idx: u32,
    snippet: Snippet,
) -> Result<(), String> {
    let new_trigger = snippet.trigger.clone();
    let old_trigger = {
        let guard = host.state.lock().map_err(|e| e.to_string())?;
        guard
            .library
            .get(&category)
            .and_then(|v| v.get(snippet_idx as usize))
            .map(|s| s.trigger.clone())
    };

    {
        let mut guard = host.state.lock().map_err(|e| e.to_string())?;
        guard
            .update_snippet(&category, snippet_idx as usize, &snippet)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
    }

    if let Some(old) = old_trigger {
        if old != new_trigger {
            let _ = crate::kms_repository::delete_embeddings_for_entity("snippet", &old);
            let _ = crate::kms_repository::update_index_status("snippets", &old, "deleted", None);
        }
    }

    let handle = get_app(&host.app_handle);
    let service = handle
        .state::<Arc<crate::indexing_service::KmsIndexingService>>()
        .inner()
        .clone();
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        let _ = service
            .index_single_item(&handle_clone, "snippets", &new_trigger)
            .await;
    });

    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

pub(crate) async fn delete_snippet(host: ApiImpl, category: String, snippet_idx: u32) -> Result<(), String> {
    let trigger = {
        let guard = host.state.lock().map_err(|e| e.to_string())?;
        guard
            .library
            .get(&category)
            .and_then(|v| v.get(snippet_idx as usize))
            .map(|s| s.trigger.clone())
    };

    {
        let mut guard = host.state.lock().map_err(|e| e.to_string())?;
        guard
            .delete_snippet(&category, snippet_idx as usize)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
    }

    if let Some(t) = trigger {
        let triggers = vec![t];
        for t in triggers {
            let _ = crate::kms_repository::delete_embeddings_for_entity("snippet", &t);
            let _ = crate::kms_repository::update_index_status("snippets", &t, "deleted", None);
        }
    }

    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

