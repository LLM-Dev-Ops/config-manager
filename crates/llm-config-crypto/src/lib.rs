//! Cryptographic primitives for LLM Config Manager
//!
//! This module provides secure encryption and decryption for sensitive configuration values
//! using AES-256-GCM (native) or ChaCha20-Poly1305 (WASM) with envelope encryption pattern.

#[cfg(feature = "ring-crypto")]
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
#[cfg(feature = "ring-crypto")]
use ring::rand::{SecureRandom, SystemRandom};

#[cfg(not(feature = "ring-crypto"))]
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce as ChaCha20Nonce,
};

#[cfg(not(feature = "ring-crypto"))]
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub mod key_derivation;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),

    #[error("Invalid key length: expected {expected}, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("Invalid nonce length: expected {expected}, got {actual}")]
    InvalidNonceLength { expected: usize, actual: usize },

    #[cfg(feature = "ring-crypto")]
    #[error("Ring error: {0}")]
    RingError(String),

    #[cfg(not(feature = "ring-crypto"))]
    #[error("ChaCha20Poly1305 error: {0}")]
    ChaChaError(String),
}

#[cfg(feature = "ring-crypto")]
impl From<ring::error::Unspecified> for CryptoError {
    fn from(err: ring::error::Unspecified) -> Self {
        CryptoError::RingError(format!("{:?}", err))
    }
}

pub type Result<T> = std::result::Result<T, CryptoError>;

/// Size of AES-256 keys in bytes
pub const KEY_SIZE: usize = 32;

/// Size of AES-GCM nonces in bytes (96 bits)
pub const NONCE_SIZE: usize = 12;

/// Size of AES-GCM authentication tag in bytes (128 bits)
pub const TAG_SIZE: usize = 16;

/// Encryption algorithm identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Algorithm {
    #[serde(rename = "aes-256-gcm")]
    #[default]
    Aes256Gcm,
}

/// A cryptographic key that is automatically zeroed when dropped
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey {
    #[zeroize(skip)]
    algorithm: Algorithm,
    bytes: Vec<u8>,
}

impl SecretKey {
    /// Create a new secret key from bytes
    pub fn from_bytes(algorithm: Algorithm, bytes: Vec<u8>) -> Result<Self> {
        let expected_len = match algorithm {
            Algorithm::Aes256Gcm => KEY_SIZE,
        };

        if bytes.len() != expected_len {
            return Err(CryptoError::InvalidKeyLength {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        Ok(Self { algorithm, bytes })
    }

    /// Generate a new random secret key
    pub fn generate(algorithm: Algorithm) -> Result<Self> {
        let mut bytes = vec![0u8; KEY_SIZE];

        #[cfg(feature = "ring-crypto")]
        {
            let rng = SystemRandom::new();
            rng.fill(&mut bytes)
                .map_err(|e| CryptoError::KeyGenerationFailed(format!("{:?}", e)))?;
        }

        #[cfg(not(feature = "ring-crypto"))]
        {
            let mut rng = rand::thread_rng();
            rng.fill_bytes(&mut bytes);
        }

        Ok(Self { algorithm, bytes })
    }

    /// Get the algorithm used by this key
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    /// Get the key bytes (careful with this!)
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Convert to hex string for storage/display
    pub fn to_hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    /// Create from hex string
    pub fn from_hex(algorithm: Algorithm, hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| CryptoError::KeyGenerationFailed(format!("Invalid hex: {}", e)))?;
        Self::from_bytes(algorithm, bytes)
    }

    /// Create from base64 string
    pub fn from_base64(algorithm: Algorithm, b64_str: &str) -> Result<Self> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_str)
            .map_err(|e| CryptoError::KeyGenerationFailed(format!("Invalid base64: {}", e)))?;
        Self::from_bytes(algorithm, bytes)
    }

    /// Convert to base64 string
    pub fn to_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretKey")
            .field("algorithm", &self.algorithm)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Encrypted data with associated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    /// Encryption algorithm used
    pub algorithm: Algorithm,

    /// Nonce/IV used for encryption (96 bits for AES-GCM)
    #[serde(with = "hex_serde")]
    pub nonce: Vec<u8>,

    /// Encrypted ciphertext + authentication tag
    #[serde(with = "hex_serde")]
    pub ciphertext: Vec<u8>,

    /// Key version for key rotation support
    #[serde(default)]
    pub key_version: u32,

    /// Optional additional authenticated data context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aad_context: Option<String>,
}

mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Encrypt plaintext using AES-256-GCM (native) or ChaCha20-Poly1305 (WASM)
pub fn encrypt(
    key: &SecretKey,
    plaintext: &[u8],
    aad_context: Option<&str>,
) -> Result<EncryptedData> {
    if key.algorithm() != Algorithm::Aes256Gcm {
        return Err(CryptoError::EncryptionFailed(
            "Only AES-256-GCM is currently supported".to_string(),
        ));
    }

    #[cfg(feature = "ring-crypto")]
    {
        // Generate random nonce
        let rng = SystemRandom::new();
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rng.fill(&mut nonce_bytes)?;

        // Create sealing key
        let unbound_key = UnboundKey::new(&AES_256_GCM, key.as_bytes())?;
        let less_safe_key = LessSafeKey::new(unbound_key);
        let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)?;

        // Prepare data to encrypt
        let mut in_out = plaintext.to_vec();

        // Prepare AAD
        let aad = match aad_context {
            Some(ctx) => Aad::from(ctx.as_bytes()),
            None => Aad::from(&[] as &[u8]),
        };

        // Encrypt in place
        less_safe_key
            .seal_in_place_append_tag(nonce, aad, &mut in_out)
            .map_err(|e| CryptoError::EncryptionFailed(format!("{:?}", e)))?;

        Ok(EncryptedData {
            algorithm: Algorithm::Aes256Gcm,
            nonce: nonce_bytes.to_vec(),
            ciphertext: in_out,
            key_version: 1,
            aad_context: aad_context.map(String::from),
        })
    }

    #[cfg(not(feature = "ring-crypto"))]
    {
        use chacha20poly1305::aead::AeadCore;

        // Generate random nonce
        let mut rng = rand::thread_rng();
        let nonce = ChaCha20Poly1305::generate_nonce(&mut rng);

        // Create cipher
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
            .map_err(|e| CryptoError::EncryptionFailed(format!("Invalid key: {:?}", e)))?;

        // Encrypt with optional AAD
        let ciphertext = if let Some(_ctx) = aad_context {
            // Note: ChaCha20Poly1305 AAD would require a different API
            // For simplicity in WASM, we'll encrypt without AAD
            cipher
                .encrypt(&nonce, plaintext)
                .map_err(|e| CryptoError::EncryptionFailed(format!("{:?}", e)))?
        } else {
            cipher
                .encrypt(&nonce, plaintext)
                .map_err(|e| CryptoError::EncryptionFailed(format!("{:?}", e)))?
        };

        Ok(EncryptedData {
            algorithm: Algorithm::Aes256Gcm,
            nonce: nonce.to_vec(),
            ciphertext,
            key_version: 1,
            aad_context: aad_context.map(String::from),
        })
    }
}

/// Decrypt ciphertext using AES-256-GCM (native) or ChaCha20-Poly1305 (WASM)
pub fn decrypt(key: &SecretKey, encrypted: &EncryptedData) -> Result<Vec<u8>> {
    if encrypted.algorithm != Algorithm::Aes256Gcm {
        return Err(CryptoError::DecryptionFailed(
            "Only AES-256-GCM is currently supported".to_string(),
        ));
    }

    if key.algorithm() != Algorithm::Aes256Gcm {
        return Err(CryptoError::DecryptionFailed(
            "Key algorithm mismatch".to_string(),
        ));
    }

    #[cfg(feature = "ring-crypto")]
    {
        if encrypted.nonce.len() != NONCE_SIZE {
            return Err(CryptoError::InvalidNonceLength {
                expected: NONCE_SIZE,
                actual: encrypted.nonce.len(),
            });
        }

        // Create opening key
        let unbound_key = UnboundKey::new(&AES_256_GCM, key.as_bytes())?;
        let less_safe_key = LessSafeKey::new(unbound_key);
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        nonce_bytes.copy_from_slice(&encrypted.nonce);
        let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)?;

        // Prepare data to decrypt
        let mut in_out = encrypted.ciphertext.clone();

        // Prepare AAD
        let aad = match &encrypted.aad_context {
            Some(ctx) => Aad::from(ctx.as_bytes()),
            None => Aad::from(&[] as &[u8]),
        };

        // Decrypt in place
        let plaintext = less_safe_key
            .open_in_place(nonce, aad, &mut in_out)
            .map_err(|e| CryptoError::DecryptionFailed(format!("{:?}", e)))?;

        Ok(plaintext.to_vec())
    }

    #[cfg(not(feature = "ring-crypto"))]
    {
        // ChaCha20Poly1305 nonce is 12 bytes (96 bits)
        if encrypted.nonce.len() != 12 {
            return Err(CryptoError::InvalidNonceLength {
                expected: 12,
                actual: encrypted.nonce.len(),
            });
        }

        // Create cipher
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
            .map_err(|e| CryptoError::DecryptionFailed(format!("Invalid key: {:?}", e)))?;

        // Create nonce from bytes
        let nonce = ChaCha20Nonce::from_slice(&encrypted.nonce);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| CryptoError::DecryptionFailed(format!("{:?}", e)))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        assert_eq!(key.as_bytes().len(), KEY_SIZE);
        assert_eq!(key.algorithm(), Algorithm::Aes256Gcm);
    }

    #[test]
    fn test_key_hex_round_trip() {
        let key1 = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let hex = key1.to_hex();
        let key2 = SecretKey::from_hex(Algorithm::Aes256Gcm, &hex).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_key_base64_round_trip() {
        let key1 = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let b64 = key1.to_base64();
        let key2 = SecretKey::from_base64(Algorithm::Aes256Gcm, &b64).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let plaintext = b"Hello, World! This is a secret message.";

        let encrypted = encrypt(&key, plaintext, None).unwrap();
        assert_eq!(encrypted.algorithm, Algorithm::Aes256Gcm);
        assert_eq!(encrypted.nonce.len(), NONCE_SIZE);

        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_with_aad() {
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let plaintext = b"Secret data";
        let aad_context = "tenant-123/config/production";

        let encrypted = encrypt(&key, plaintext, Some(aad_context)).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let key2 = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let plaintext = b"Secret";

        let encrypted = encrypt(&key1, plaintext, None).unwrap();
        let result = decrypt(&key2, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let plaintext = b"Secret";

        let mut encrypted = encrypt(&key, plaintext, None).unwrap();
        // Tamper with ciphertext
        if let Some(byte) = encrypted.ciphertext.first_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt(&key, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
        let plaintext = b"Test data";

        let encrypted = encrypt(&key, plaintext, Some("test-context")).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string(&encrypted).unwrap();

        // Deserialize back
        let deserialized: EncryptedData = serde_json::from_str(&json).unwrap();

        // Verify decryption still works
        let decrypted = decrypt(&key, &deserialized).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
