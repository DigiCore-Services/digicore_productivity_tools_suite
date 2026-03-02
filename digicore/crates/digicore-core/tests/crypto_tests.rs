//! Unit and integration tests for AesCryptoAdapter.

#![cfg(feature = "sync")]

use digicore_core::adapters::crypto::aes_ring::AesCryptoAdapter;
use digicore_core::domain::ports::CryptoPort;

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let crypto = AesCryptoAdapter;
    let plaintext = b"Hello, World!";
    let password = "test-password";

    let encrypted = crypto.encrypt(plaintext, password).unwrap();
    assert!(!encrypted.is_empty());
    assert_ne!(encrypted.as_bytes(), plaintext);

    let decrypted = crypto.decrypt(&encrypted, password).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_decrypt_wrong_password_fails() {
    let crypto = AesCryptoAdapter;
    let plaintext = b"secret";
    let encrypted = crypto.encrypt(plaintext, "right").unwrap();

    let result = crypto.decrypt(&encrypted, "wrong");
    assert!(result.is_err());
}

#[test]
fn test_encrypt_different_nonce_per_call() {
    let crypto = AesCryptoAdapter;
    let plaintext = b"same";
    let e1 = crypto.encrypt(plaintext, "pwd").unwrap();
    let e2 = crypto.encrypt(plaintext, "pwd").unwrap();
    assert_ne!(e1, e2, "Random nonce should produce different ciphertexts");
    assert_eq!(crypto.decrypt(&e1, "pwd").unwrap(), plaintext);
    assert_eq!(crypto.decrypt(&e2, "pwd").unwrap(), plaintext);
}
