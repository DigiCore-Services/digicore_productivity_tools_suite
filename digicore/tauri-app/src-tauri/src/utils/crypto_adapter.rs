use crate::utils::crypto;
use digicore_text_expander::ports::CryptoPort;

/// Bridge between Tauri's DPAPI utility and digicore-text-expander's CryptoPort.
pub struct TauriCryptoAdapter;

impl CryptoPort for TauriCryptoAdapter {
    fn encrypt_local(&self, text: &str) -> Result<String, String> {
        crypto::encrypt_local(text)
    }

    fn decrypt_local(&self, encrypted: &str) -> Option<String> {
        crypto::decrypt_local(encrypted)
    }
}
