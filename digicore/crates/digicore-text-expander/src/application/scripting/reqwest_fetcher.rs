//! ReqwestHttpFetcher: Adapter implementing HttpFetcherPort (SE-5).
//! SE-9: HTTP retry with exponential backoff. Blocking client.

use super::config::get_config;
use super::http_fetcher::extract_json_path;
use super::http_port::HttpFetcherPort;
use super::url_allowlist::is_url_allowed;
use std::time::Duration;

/// Reqwest-based blocking HTTP fetcher using configurable timeout and retry.
pub struct ReqwestHttpFetcher;

impl HttpFetcherPort for ReqwestHttpFetcher {
    fn fetch(&self, url: &str, json_path: Option<&str>) -> String {
        let cfg = get_config();
        if !is_url_allowed(url, &cfg.http.url_allowlist) {
            return "[HTTP Error: domain not in allowlist]".to_string();
        }
        let timeout_secs = cfg.http.timeout_secs;
        let retry_count = cfg.http.retry_count;
        let retry_delay_ms = cfg.http.retry_delay_ms;

        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
        {
            Ok(c) => c,
            Err(e) => return format!("[HTTP Error: {}]", e),
        };

        let mut last_error = String::new();
        let mut delay_ms = retry_delay_ms;

        for attempt in 0..=retry_count {
            match client.get(url).send() {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        last_error = format!("[HTTP Error: {}]", status);
                        if attempt < retry_count {
                            std::thread::sleep(Duration::from_millis(delay_ms));
                            delay_ms = (delay_ms * 2).min(10_000);
                            continue;
                        }
                        return last_error;
                    }
                    match response.text() {
                        Ok(body) => {
                            return if let Some(path) = json_path {
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
                std::thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(10_000);
            }
        }
        last_error
    }
}
