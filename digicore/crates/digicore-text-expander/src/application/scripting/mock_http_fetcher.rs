//! Mock HttpFetcherPort (SE-20): For integration tests without network.

use super::http_port::HttpFetcherPort;
use std::collections::HashMap;
use std::sync::Mutex;

/// Mock HTTP fetcher for tests. Returns predefined results by (url, path).
pub struct MockHttpFetcher {
    results: Mutex<HashMap<(String, Option<String>), String>>,
}

impl MockHttpFetcher {
    pub fn new() -> Self {
        Self {
            results: Mutex::new(HashMap::new()),
        }
    }

    /// Register expected result for (url, path). Use None for path when no JSON path.
    pub fn expect(&self, url: &str, path: Option<&str>, result: impl Into<String>) {
        if let Ok(mut g) = self.results.lock() {
            g.insert(
                (url.to_string(), path.map(String::from)),
                result.into(),
            );
        }
    }

    /// Default mock returning fixed IP for common ipify-style requests.
    pub fn with_ipify_default() -> Self {
        let m = Self::new();
        m.expect(
            "https://api.ipify.org?format=json",
            Some("ip"),
            "192.168.1.1",
        );
        m
    }
}

impl Default for MockHttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFetcherPort for MockHttpFetcher {
    fn fetch(&self, url: &str, json_path: Option<&str>) -> String {
        let key = (url.to_string(), json_path.map(String::from));
        if let Ok(g) = self.results.lock() {
            if let Some(r) = g.get(&key) {
                return r.clone();
            }
        }
        "[MockHttpFetcher: no expectation]".to_string()
    }
}
