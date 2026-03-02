//! AesCryptoAdapter - implements CryptoPort using ring (AES-256-GCM, PBKDF2).
//!
//! F34: AES-256 encryption before upload.
//! Uses AES-256-GCM (authenticated) + PBKDF2-SHA256 for key derivation.

use crate::domain::ports::CryptoPort;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use ring::pbkdf2;
use ring::rand::SecureRandom;
use std::num::NonZeroU32;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT: &[u8] = b"digicore-te-sync-salt-v1";
const NONCE_LEN: usize = 12;

/// Single-use nonce sequence yielding one fixed nonce.
struct SingleNonceSequence([u8; NONCE_LEN]);

impl NonceSequence for SingleNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        Ok(Nonce::assume_unique_for_key(self.0))
    }
}

/// AES-256-GCM crypto adapter via ring.
#[derive(Debug, Default)]
pub struct AesCryptoAdapter;

impl AesCryptoAdapter {
    fn derive_key(password: &str) -> Result<[u8; 32]> {
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(PBKDF2_ITERATIONS).unwrap(),
            SALT,
            password.as_bytes(),
            &mut key,
        );
        Ok(key)
    }
}

impl CryptoPort for AesCryptoAdapter {
    fn encrypt(&self, plaintext: &[u8], password: &str) -> Result<String> {
        let key_bytes = Self::derive_key(password)?;
        let unbound = UnboundKey::new(&ring::aead::AES_256_GCM, &key_bytes)
            .map_err(|e| anyhow::anyhow!("UnboundKey: {}", e))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        ring::rand::SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| anyhow::anyhow!("Rand failed"))?;

        let nonce_seq = SingleNonceSequence(nonce_bytes);
        let mut sealing_key = SealingKey::new(unbound, nonce_seq);

        let mut in_out = plaintext.to_vec();
        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|e| anyhow::anyhow!("Seal: {}", e))?;

        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&in_out);
        Ok(BASE64.encode(&result))
    }

    fn decrypt(&self, ciphertext_b64: &str, password: &str) -> Result<Vec<u8>> {
        let bytes = BASE64
            .decode(ciphertext_b64)
            .context("Base64 decode")?;
        if bytes.len() < NONCE_LEN + 16 {
            anyhow::bail!("Ciphertext too short");
        }

        let (nonce_bytes, cipher) = bytes.split_at(NONCE_LEN);
        let mut nonce_arr = [0u8; NONCE_LEN];
        nonce_arr.copy_from_slice(nonce_bytes);

        let key_bytes = Self::derive_key(password)?;
        let unbound = UnboundKey::new(&ring::aead::AES_256_GCM, &key_bytes)
            .map_err(|e| anyhow::anyhow!("UnboundKey: {}", e))?;

        let nonce_seq = SingleNonceSequence(nonce_arr);
        let mut opening_key = OpeningKey::new(unbound, nonce_seq);

        let mut in_out = cipher.to_vec();
        let len = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .map_err(|_| anyhow::anyhow!("Decrypt failed (wrong password?)"))?
            .len();
        in_out.truncate(len);
        Ok(in_out)
    }
}
