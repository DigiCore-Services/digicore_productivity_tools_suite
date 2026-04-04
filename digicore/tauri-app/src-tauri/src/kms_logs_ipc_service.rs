//! KMS diagnostic log listing and clear.

use super::*;

pub(crate) async fn kms_get_logs(_host: ApiImpl, limit: u32) -> Result<Vec<KmsLogDto>, String> {
    let request_id = kms_request_id("get_logs");
    let logs = kms_repository::list_logs(limit).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_LOGS",
            "KMS_LOGS_LOAD_FAIL",
            "Failed to load KMS logs",
            Some(e.to_string()),
        )
    })?;
    Ok(logs
        .into_iter()
        .map(|l| KmsLogDto {
            id: l.id,
            level: l.level,
            message: l.message,
            details: l.details,
            timestamp: l.timestamp,
        })
        .collect())
}

pub(crate) async fn kms_clear_logs(_host: ApiImpl) -> Result<(), String> {
    let request_id = kms_request_id("clear_logs");
    kms_repository::clear_logs().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_LOGS",
            "KMS_LOGS_CLEAR_FAIL",
            "Failed to clear KMS logs",
            Some(e.to_string()),
        )
    })
}

