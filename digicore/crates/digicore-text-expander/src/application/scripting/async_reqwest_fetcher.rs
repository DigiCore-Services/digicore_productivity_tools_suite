//! AsyncReqwestHttpFetcher: Adapter implementing HttpFetcherPort (SE-24).
//! Uses tokio + async reqwest; block_on for sync port interface.
//! SE-9: HTTP retry with exponential backoff.

use super::config::get_config;
use super::http_fetcher::extract_json_path;
use super::http_port::HttpFetcherPort;
use super::url_allowlist::is_url_allowed;
use std::time::Duration;

/// Async reqwest-based HTTP fetcher. Implements HttpFetcherPort via block_on.
pub struct AsyncReqwestHttpFetcher;

impl HttpFetcherPort for AsyncReqwestHttpFetcher {
    fn fetch(&self, url: &str, json_path: Option<&str>) -> String {
        let cfg = get_config();
        if !is_url_allowed(url, &cfg.http.url_allowlist) {
            return "[HTTP Error: domain not in allowlist]".to_string();
        }
        let timeout_secs = cfg.http.timeout_secs;
        let retry_count = cfg.http.retry_count;
        let retry_delay_ms = cfg.http.retry_delay_ms;
        let url = url.to_string();
        let json_path = json_path.map(String::from);

        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => return format!("[HTTP Error: {}]", e),
        };

        rt.block_on(async {
            let client = match reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()
            {
                Ok(c) => c,
                Err(e) => return format!("[HTTP Error: {}]", e),
            };

            let mut last_error = String::new();
            let mut delay_ms = retry_delay_ms;

            for attempt in 0..=retry_count {
                match client.get(&url).send().await {
                    Ok(response) => {
                        let status = response.status();
                        if !status.is_success() {
                            last_error = format!("[HTTP Error: {}]", status);
                            if attempt < retry_count {
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                delay_ms = (delay_ms * 2).min(10_000);
                                continue;
                            }
                            return last_error;
                        }
                        match response.text().await {
                            Ok(body) => {
                                return if let Some(ref path) = json_path {
                                    extract_json_path(&body, path)
                                } else {
                                    body
                                };
                            }
                            Err(e) => {
                                last_error = format!("[HTTP Error: {}]", e);
                            }
                        }
                    }
                    Err(e) => {
                        last_error = if e.is_timeout() {
                            "[HTTP Timeout]".to_string()
                        } else {
                            format!("[HTTP Error: {}]", e)
                        };
                    }
                }
                if attempt < retry_count {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = (delay_ms * 2).min(10_000);
                }
            }
            last_error
        })
    }
}
