//! HttpFetcherPort (SE-5): Port trait for HTTP fetching; enables mock for tests.

/// Port for HTTP GET with optional JSON path extraction.
pub trait HttpFetcherPort: Send + Sync {
    /// Fetch URL; if json_path is Some, extract value at dot-separated path.
    /// Returns body or extracted value. Errors return [HTTP Timeout], [HTTP Error: ...], etc.
    fn fetch(&self, url: &str, json_path: Option<&str>) -> String;
}
