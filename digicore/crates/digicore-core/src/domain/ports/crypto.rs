//! ICryptoPort - encrypt/decrypt for sync (AES-256, PBKDF2).
//!
//! F34: AES-256 encryption before upload.

use anyhow::Result;

/// Port for encryption/decryption (sync payloads).
///
/// Implementations: AesCryptoAdapter (ring).
pub trait CryptoPort: Send + Sync {
    /// Encrypt plaintext with password-derived key.
    /// Returns base64-encoded ciphertext (IV + nonce + tag + ciphertext, format adapter-specific).
    fn encrypt(&self, plaintext: &[u8], password: &str) -> Result<String>;

    /// Decrypt base64-encoded ciphertext.
    fn decrypt(&self, ciphertext_b64: &str, password: &str) -> Result<Vec<u8>>;
}
