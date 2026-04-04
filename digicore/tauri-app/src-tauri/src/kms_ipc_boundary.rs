//! Structured KMS/IPC error JSON and request-id generation (colocated with `IpcErrorDto`).

use std::sync::atomic::{AtomicU64, Ordering};

/// Structured IPC error (serialized JSON string on the `Err` side of `Result<T, String>` for graph calls).
#[taurpc::ipc_type]
pub struct IpcErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_code: Option<String>,
    pub details: Option<String>,
}

pub(crate) fn ipc_error(code: &str, message: impl Into<String>, details: Option<String>) -> String {
    serde_json::to_string(&IpcErrorDto {
        code: code.to_string(),
        message: message.into(),
        request_id: None,
        event_code: None,
        details,
    })
    .unwrap_or_else(|_| {
        r#"{"code":"IPC_JSON","message":"failed to serialize IpcErrorDto","details":null}"#.to_string()
    })
}

pub(crate) fn kms_request_id(scope: &str) -> String {
    static KMS_REQUEST_SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = KMS_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("kms_{}_{}_{}", scope, ts, seq)
}

pub(crate) fn kms_ipc_error(
    request_id: &str,
    code: &str,
    event_code: &str,
    message: impl Into<String>,
    details: Option<String>,
) -> String {
    serde_json::to_string(&IpcErrorDto {
        code: code.to_string(),
        message: message.into(),
        request_id: Some(request_id.to_string()),
        event_code: Some(event_code.to_string()),
        details,
    })
    .unwrap_or_else(|_| {
        r#"{"code":"IPC_JSON","message":"failed to serialize IpcErrorDto","request_id":null,"event_code":null,"details":null}"#.to_string()
    })
}
