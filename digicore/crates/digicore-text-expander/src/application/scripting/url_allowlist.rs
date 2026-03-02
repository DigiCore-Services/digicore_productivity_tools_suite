//! URL allowlist helpers (SE-22): Shared by blocking and async HTTP fetchers.
//! SRP: Single module for host extraction and allowlist checks.

/// Extract host from URL (e.g. https://api.example.com/path -> api.example.com). SE-22.
pub fn url_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

/// Check if URL host is allowed by allowlist. Empty allowlist = allow all. SE-22.
pub fn is_url_allowed(url: &str, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return true;
    }
    let host = match url_host(url) {
        Some(h) => h,
        None => return false,
    };
    for allowed in allowlist {
        let a = allowed.trim().to_lowercase();
        if a.is_empty() {
            continue;
        }
        if host == a || host.ends_with(&format!(".{}", a)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_host() {
        assert_eq!(url_host("https://api.example.com/path"), Some("api.example.com".to_string()));
        assert_eq!(url_host("http://localhost:8080/"), Some("localhost:8080".to_string()));
    }

    #[test]
    fn test_is_url_allowed_empty_allowlist() {
        assert!(is_url_allowed("https://any.com", &[]));
    }

    #[test]
    fn test_is_url_allowed_match() {
        let list = vec!["api.example.com".to_string()];
        assert!(is_url_allowed("https://api.example.com/", &list));
    }

    #[test]
    fn test_is_url_allowed_subdomain() {
        let list = vec!["example.com".to_string()];
        assert!(is_url_allowed("https://api.example.com/", &list));
    }

    #[test]
    fn test_is_url_allowed_denied() {
        let list = vec!["allowed.com".to_string()];
        assert!(!is_url_allowed("https://evil.com/", &list));
    }
}
