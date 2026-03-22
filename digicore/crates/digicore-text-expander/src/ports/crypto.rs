//! CryptoPort - framework-agnostic local encryption/decryption.

/// Port for local encryption/decryption (e.g. DPAPI on Windows).
pub trait CryptoPort: Send + Sync {
    /// Encrypt a string locally.
    fn encrypt_local(&self, text: &str) -> Result<String, String>;

    /// Decrypt a string locally.
    fn decrypt_local(&self, encrypted: &str) -> Option<String>;
}

/// No-op implementation for platforms/runtimes where encryption is not available.
#[derive(Debug, Default)]
pub struct NoopCryptoPort;

impl CryptoPort for NoopCryptoPort {
    fn encrypt_local(&self, text: &str) -> Result<String, String> {
        Ok(text.to_string())
    }

    fn decrypt_local(&self, encrypted: &str) -> Option<String> {
        Some(encrypted.to_string())
    }
}
