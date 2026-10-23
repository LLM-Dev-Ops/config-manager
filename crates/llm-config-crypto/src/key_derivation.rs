//! Key derivation using Argon2id

use crate::{Algorithm, CryptoError, Result, SecretKey, KEY_SIZE};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};

/// Derive a key from a password using Argon2id
pub fn derive_key_from_password(password: &str, salt: Option<&str>) -> Result<(SecretKey, String)> {
    let salt_string = match salt {
        Some(s) => SaltString::from_b64(s)
            .map_err(|e| CryptoError::KeyGenerationFailed(format!("Invalid salt: {}", e)))?,
        None => SaltString::generate(&mut OsRng),
    };

    // Argon2 parameters: memory=64MB, iterations=3, parallelism=4
    let params = Params::new(65536, 3, 4, Some(KEY_SIZE))
        .map_err(|e| CryptoError::KeyGenerationFailed(format!("Invalid Argon2 params: {}", e)))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .map_err(|e| CryptoError::KeyGenerationFailed(format!("Hash failed: {}", e)))?;

    // Extract the raw key bytes
    let hash_bytes = password_hash
        .hash
        .ok_or_else(|| CryptoError::KeyGenerationFailed("No hash output".to_string()))?;

    let key = SecretKey::from_bytes(Algorithm::Aes256Gcm, hash_bytes.as_bytes().to_vec())?;

    Ok((key, password_hash.to_string()))
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash_str: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(hash_str)
        .map_err(|e| CryptoError::KeyGenerationFailed(format!("Invalid hash: {}", e)))?;

    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let password = "my-secure-password";
        let (key, hash) = derive_key_from_password(password, None).unwrap();

        assert_eq!(key.as_bytes().len(), KEY_SIZE);
        assert!(!hash.is_empty());
    }

    #[test]
    fn test_password_verification() {
        let password = "correct-password";
        let (_key, hash) = derive_key_from_password(password, None).unwrap();

        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong-password", &hash).unwrap());
    }

    #[test]
    fn test_deterministic_with_salt() {
        let password = "test-password";
        let salt = SaltString::generate(&mut OsRng).as_str().to_string();

        let (key1, _) = derive_key_from_password(password, Some(&salt)).unwrap();
        let (key2, _) = derive_key_from_password(password, Some(&salt)).unwrap();

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }
}
