//! WebDAVAdapter - implements SyncPort for WebDAV push/pull.
//!
//! F33: WebDAV push/pull
//! F35: Retry on failure (3x, 2s delay)

use crate::adapters::crypto::aes_ring::AesCryptoAdapter;
use crate::domain::ports::{CryptoPort, SyncPort};
use anyhow::{Context, Result};
use std::thread;
use std::time::Duration;

const RETRY_COUNT: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;

/// WebDAV sync adapter with built-in AES crypto.
#[derive(Debug)]
pub struct WebDAVAdapter {
    client: reqwest::blocking::Client,
    crypto: AesCryptoAdapter,
}

impl WebDAVAdapter {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Build HTTP client")?;
        Ok(Self {
            client,
            crypto: AesCryptoAdapter,
        })
    }

    fn with_retry<F, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let mut last_err = None;
        for attempt in 0..RETRY_COUNT {
            match f() {
                Ok(t) => return Ok(t),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < RETRY_COUNT - 1 {
                        thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }
}

impl Default for WebDAVAdapter {
    fn default() -> Self {
        Self::new().expect("WebDAVAdapter init")
    }
}

impl SyncPort for WebDAVAdapter {
    fn push(&self, library_json: &[u8], url: &str, password: &str) -> Result<()> {
        self.with_retry(|| {
            let encrypted = self.crypto.encrypt(library_json, password)?;
            self.client
                .put(url)
                .body(encrypted.into_bytes())
                .send()
                .context("PUT request")?
                .error_for_status()
                .context("PUT failed")?;
            Ok(())
        })
    }

    fn pull(&self, url: &str, password: &str) -> Result<Vec<u8>> {
        self.with_retry(|| {
            let resp = self
                .client
                .get(url)
                .send()
                .context("GET request")?
                .error_for_status()
                .context("GET failed")?;
            let bytes = resp.bytes().context("Read body")?;
            let ciphertext_b64 = String::from_utf8_lossy(&bytes);
            self.crypto.decrypt(&ciphertext_b64, password)
        })
    }
}
