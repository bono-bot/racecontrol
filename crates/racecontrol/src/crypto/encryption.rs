use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce, AeadCore};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

/// Field-level encryption and deterministic phone hashing for PII protection.
///
/// Uses AES-256-GCM for encryption (random nonce per call) and HMAC-SHA256
/// for deterministic phone number hashing (enables lookup without decryption).
pub struct FieldCipher {
    cipher: Aes256Gcm,
    hmac_key: Vec<u8>,
}

impl FieldCipher {
    /// Create a new FieldCipher from raw key material.
    pub fn new(aes_key: &[u8; 32], hmac_key: &[u8]) -> Self {
        Self {
            cipher: Aes256Gcm::new(aes_key.into()),
            hmac_key: hmac_key.to_vec(),
        }
    }

    /// Encrypt a plaintext string field. Returns base64(nonce || ciphertext).
    /// Each call produces different ciphertext due to random 12-byte nonce.
    pub fn encrypt_field(&self, plaintext: &str) -> Result<String, String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {e}"))?;

        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(&combined))
    }

    /// Decrypt a base64-encoded field (nonce || ciphertext) back to plaintext.
    pub fn decrypt_field(&self, encoded: &str) -> Result<String, String> {
        let data = BASE64
            .decode(encoded)
            .map_err(|e| format!("Base64 decode failed: {e}"))?;

        if data.len() < 13 {
            return Err("Ciphertext too short (missing nonce or data)".to_string());
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {e}"))?;

        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode failed: {e}"))
    }

    /// Deterministic HMAC-SHA256 hash of a phone number.
    /// Normalizes by trimming whitespace and stripping "+91" prefix.
    /// Returns lowercase hex string.
    pub fn hash_phone(&self, phone: &str) -> String {
        let normalized = phone.trim().strip_prefix("+91").unwrap_or(phone.trim());

        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.hmac_key)
            .expect("HMAC accepts any key length");
        mac.update(normalized.as_bytes());
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }
}

/// Load encryption keys from environment variables.
///
/// Requires:
/// - `RACECONTROL_ENCRYPTION_KEY`: 64 hex chars (32 bytes) for AES-256-GCM
/// - `RACECONTROL_HMAC_KEY`: valid hex string for HMAC-SHA256
///
/// Returns descriptive error on missing or invalid keys.
pub fn load_encryption_keys() -> Result<FieldCipher, String> {
    let enc_hex = std::env::var("RACECONTROL_ENCRYPTION_KEY")
        .map_err(|_| "RACECONTROL_ENCRYPTION_KEY env var not set".to_string())?;

    let hmac_hex = std::env::var("RACECONTROL_HMAC_KEY")
        .map_err(|_| "RACECONTROL_HMAC_KEY env var not set".to_string())?;

    let enc_bytes = hex::decode(&enc_hex)
        .map_err(|e| format!("RACECONTROL_ENCRYPTION_KEY invalid hex: {e}"))?;

    if enc_bytes.len() != 32 {
        return Err(format!(
            "RACECONTROL_ENCRYPTION_KEY must be 64 hex chars (32 bytes), got {} bytes",
            enc_bytes.len()
        ));
    }

    let hmac_bytes = hex::decode(&hmac_hex)
        .map_err(|e| format!("RACECONTROL_HMAC_KEY invalid hex: {e}"))?;

    let aes_key: [u8; 32] = enc_bytes
        .try_into()
        .map_err(|_| "Failed to convert encryption key to [u8; 32]".to_string())?;

    Ok(FieldCipher::new(&aes_key, &hmac_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cipher() -> FieldCipher {
        let aes_key = [0x42u8; 32];
        let hmac_key = [0x7Fu8; 32];
        FieldCipher::new(&aes_key, &hmac_key)
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let cipher = test_cipher();
        let encrypted = cipher.encrypt_field("hello").expect("encrypt");
        let decrypted = cipher.decrypt_field(&encrypted).expect("decrypt");
        assert_eq!(decrypted, "hello");
    }

    #[test]
    fn encrypt_produces_different_ciphertext() {
        let cipher = test_cipher();
        let a = cipher.encrypt_field("hello").expect("encrypt a");
        let b = cipher.encrypt_field("hello").expect("encrypt b");
        assert_ne!(a, b, "Two encryptions of same plaintext must differ (random nonce)");
    }

    #[test]
    fn decrypt_invalid_base64_returns_err() {
        let cipher = test_cipher();
        assert!(cipher.decrypt_field("not-base64!!!").is_err());
    }

    #[test]
    fn decrypt_too_short_returns_err() {
        let cipher = test_cipher();
        let short = BASE64.encode([1u8, 2, 3, 4, 5]);
        assert!(cipher.decrypt_field(&short).is_err());
    }

    #[test]
    fn hash_phone_deterministic() {
        let cipher = test_cipher();
        let a = cipher.hash_phone("9876543210");
        let b = cipher.hash_phone("9876543210");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_phone_strips_plus91_prefix() {
        let cipher = test_cipher();
        let with_prefix = cipher.hash_phone("+919876543210");
        let without = cipher.hash_phone("9876543210");
        assert_eq!(with_prefix, without);
    }

    #[test]
    fn hash_phone_different_numbers_differ() {
        let cipher = test_cipher();
        let a = cipher.hash_phone("9876543210");
        let b = cipher.hash_phone("1234567890");
        assert_ne!(a, b);
    }

    #[test]
    fn load_keys_missing_env_vars() {
        // SAFETY: test runs single-threaded (--test-threads=1), no concurrent env access
        unsafe {
            std::env::remove_var("RACECONTROL_ENCRYPTION_KEY");
            std::env::remove_var("RACECONTROL_HMAC_KEY");
        }
        let result = load_encryption_keys();
        let err = result.err().expect("should be Err");
        assert!(
            err.contains("RACECONTROL_ENCRYPTION_KEY"),
            "Error should mention the missing env var"
        );
    }

    #[test]
    fn load_keys_valid_hex() {
        let enc_key = "a".repeat(64); // 32 bytes of 0xAA
        let hmac_key = "b".repeat(64);
        // SAFETY: test runs single-threaded (--test-threads=1), no concurrent env access
        unsafe {
            std::env::set_var("RACECONTROL_ENCRYPTION_KEY", &enc_key);
            std::env::set_var("RACECONTROL_HMAC_KEY", &hmac_key);
        }
        let result = load_encryption_keys();
        assert!(result.is_ok(), "Valid 64-char hex keys should succeed");
        unsafe {
            std::env::remove_var("RACECONTROL_ENCRYPTION_KEY");
            std::env::remove_var("RACECONTROL_HMAC_KEY");
        }
    }

    #[test]
    fn load_keys_wrong_length() {
        // SAFETY: test runs single-threaded (--test-threads=1), no concurrent env access
        unsafe {
            std::env::set_var("RACECONTROL_ENCRYPTION_KEY", "abcd"); // only 2 bytes
            std::env::set_var("RACECONTROL_HMAC_KEY", "b".repeat(64));
        }
        let result = load_encryption_keys();
        let err = result.err().expect("should be Err");
        assert!(err.contains("32 bytes"));
        unsafe {
            std::env::remove_var("RACECONTROL_ENCRYPTION_KEY");
            std::env::remove_var("RACECONTROL_HMAC_KEY");
        }
    }
}
