//! HTTP fetcher for {http:url|path} placeholder.
//!
//! Fetches URL via GET; if path is provided, extracts JSON path (e.g. ip, slip.advice).

use serde_json::Value;

const HTTP_TIMEOUT_SECS: u64 = 5;

/// Fetch URL and optionally extract JSON path. Returns body or extracted value.
/// Errors return [HTTP Timeout], [HTTP Error: ...], [Path Error: ...].
pub fn fetch_http(url: &str, json_path: Option<&str>) -> String {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("[HTTP Error: {}]", e),
    };

    let response = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            if e.is_timeout() {
                return "[HTTP Timeout]".to_string();
            }
            return format!("[HTTP Error: {}]", e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        return format!("[HTTP Error: {}]", status);
    }

    let body = match response.text() {
        Ok(b) => b,
        Err(e) => return format!("[HTTP Error: {}]", e),
    };

    if let Some(path) = json_path {
        extract_json_path(&body, path)
    } else {
        body
    }
}

/// Extract value at dot-separated path (e.g. "ip", "slip.advice", "current_weather.temperature").
pub fn extract_json_path(body: &str, path: &str) -> String {
    let value: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return "[Path Error: invalid JSON]".to_string(),
    };

    let mut current = &value;
    for part in path.split('.') {
        current = match current {
            Value::Object(map) => match map.get(part) {
                Some(v) => v,
                None => return format!("[Path Error: key '{}' not found]", part),
            },
            Value::Array(arr) => {
                let idx: usize = part.parse().unwrap_or(0);
                match arr.get(idx) {
                    Some(v) => v,
                    None => return format!("[Path Error: index {} out of range]", idx),
                }
            }
            _ => return "[Path Error: not an object or array]".to_string(),
        };
    }

    match current {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => serde_json::to_string(current).unwrap_or_else(|_| String::new()),
    }
}
