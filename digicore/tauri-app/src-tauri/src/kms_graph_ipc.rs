//! Graph-related TauRPC procedure bodies (invoked from `api.rs` resolvers).
//! Kept in a sibling file via `#[path]` so `api.rs` stays the single module root without an `api/` directory.

use rand::Rng;

use super::ApiImpl;
use super::{KmsGraphDto, KmsGraphPathDto, KmsNoteGraphPreviewDto};

fn new_kms_graph_request_id() -> String {
    let mut r = rand::thread_rng();
    format!("kms{:08x}{:08x}", r.gen::<u32>(), r.gen::<u32>())
}

pub(crate) async fn kms_get_graph(
    host: ApiImpl,
    offset: u32,
    limit: u32,
    time_from_utc: Option<String>,
    time_to_utc: Option<String>,
) -> Result<KmsGraphDto, String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_full_ipc_service::get_graph_dto(host, offset, limit, time_from_utc, time_to_utc, rid).await
}

pub(crate) async fn kms_get_local_graph(host: ApiImpl, path: String, depth: u32) -> Result<KmsGraphDto, String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_local_ipc_service::get_local_graph_dto(host, path, depth, rid)
}

pub(crate) async fn kms_get_graph_shortest_path(
    host: ApiImpl,
    from_path: String,
    to_path: String,
) -> Result<KmsGraphPathDto, String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_path_ipc_service::get_graph_shortest_path_dto(host, from_path, to_path, rid)
}

pub(crate) async fn kms_get_note_graph_preview(
    host: ApiImpl,
    path: String,
    max_chars: u32,
) -> Result<KmsNoteGraphPreviewDto, String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_note_preview_ipc_service::get_note_graph_preview_dto(host, path, max_chars, rid)
}

pub(crate) async fn kms_export_graph_diagnostics(host: ApiImpl, path: String) -> Result<(), String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_export_ipc_service::export_graph_diagnostics(host, path, rid).await
}

pub(crate) async fn kms_export_wiki_links_json(host: ApiImpl, path: String) -> Result<(), String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_export_ipc_service::export_wiki_links_json(host, path, rid).await
}

pub(crate) async fn kms_export_graph_graphml(host: ApiImpl, path: String) -> Result<(), String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_export_ipc_service::export_graph_graphml(host, path, rid).await
}

/// Full **unpaged** graph build as JSON matching [`KmsGraphDto`] shape (plus envelope: schema, vault fingerprint, timestamps).
pub(crate) async fn kms_export_graph_dto_json(host: ApiImpl, path: String) -> Result<(), String> {
    let rid = new_kms_graph_request_id();
    super::kms_graph_export_ipc_service::export_graph_dto_json(host, path, rid).await
}

pub(crate) async fn kms_get_vault_graph_overrides_json(host: ApiImpl) -> Result<String, String> {
    super::kms_graph_overrides_ipc_service::get_vault_graph_overrides_json(host).await
}

pub(crate) async fn kms_set_vault_graph_overrides_json(
    host: ApiImpl,
    json: String,
) -> Result<(), String> {
    super::kms_graph_overrides_ipc_service::set_vault_graph_overrides_json(host, json).await
}

pub(crate) async fn kms_clear_vault_graph_overrides_json(host: ApiImpl) -> Result<(), String> {
    super::kms_graph_overrides_ipc_service::clear_vault_graph_overrides_json(host).await
}
