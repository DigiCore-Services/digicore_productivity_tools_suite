#[cfg(target_os = "windows")]
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// Encrypts a string locally using Windows DPAPI.
/// Returns the encrypted string prefixed with "ENC:".
pub fn encrypt_local(text: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    unsafe {
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: text.len() as u32,
            pbData: text.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        if CryptProtectData(&mut data_in, None, None, None, None, 0, &mut data_out).is_ok() {
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let encoded = BASE64.encode(slice);
            windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _)));
            Ok(format!("ENC:{}", encoded))
        } else {
            Err("DPAPI encryption failed".to_string())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        Err("Encryption only supported on Windows".to_string())
    }
}

/// Decrypts a string locally using Windows DPAPI.
/// If the string does not start with "ENC:", it is returned as-is (legacy support).
pub fn decrypt_local(encrypted: &str) -> Option<String> {
    if !encrypted.starts_with("ENC:") {
        return Some(encrypted.to_string());
    }
    let b64 = &encrypted[4..];
    let slice = BASE64.decode(b64).ok()?;
    
    #[cfg(target_os = "windows")]
    unsafe {
        let mut data_in = CRYPT_INTEGER_BLOB {
            cbData: slice.len() as u32,
            pbData: slice.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        if CryptUnprotectData(&mut data_in, None, None, None, None, 0, &mut data_out).is_ok() {
            let decrypted_slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let result = String::from_utf8_lossy(decrypted_slice).to_string();
            windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(data_out.pbData as *mut _)));
            Some(result)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(encrypted.to_string())
    }
}

/// Helper to encrypt an optional string.
pub fn encrypt_opt(text: Option<&str>) -> Option<String> {
    text.map(|t| encrypt_local(t).unwrap_or_else(|_| t.to_string()))
}

/// Helper to decrypt an optional string.
pub fn decrypt_opt(text: Option<String>) -> Option<String> {
    text.map(|t| decrypt_local(&t).unwrap_or(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_local() {
        let plaintext = "Secret data 123! @#$";
        let encrypted = encrypt_local(plaintext).expect("Encryption failed");
        assert_ne!(plaintext, encrypted, "Encrypted data should differ from plaintext");
        assert!(encrypted.starts_with("ENC:"), "Encrypted data should have ENC: prefix");
        
        let decrypted = decrypt_local(&encrypted).expect("Decryption failed");
        assert_eq!(plaintext, decrypted, "Decrypted data should match original plaintext");
    }

    #[test]
    fn test_decrypt_plain_text() {
        let plain = "just some text";
        let result = decrypt_local(plain).expect("Should return some");
        assert_eq!(plain, result, "Non-prefixed text should be returned as-is");
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let invalid = "ENC:not-base64-!";
        let result = decrypt_local(invalid);
        assert!(result.is_none(), "Invalid base64 should return None");
    }
}
