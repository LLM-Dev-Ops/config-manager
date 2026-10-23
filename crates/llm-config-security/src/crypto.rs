//! Cryptographic validation and security

use crate::errors::{SecurityError, SecurityResult};
use constant_time_eq::constant_time_eq;
use secrecy::{ExposeSecret, Secret};
use zeroize::Zeroizing;

/// Minimum key size in bytes
const MIN_KEY_SIZE: usize = 32; // 256 bits

/// Maximum key age in days
const MAX_KEY_AGE_DAYS: i64 = 90;

/// Crypto validator
pub struct CryptoValidator {
    strict_mode: bool,
}

impl CryptoValidator {
    /// Create a new crypto validator
    pub fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }

    /// Create in strict mode
    pub fn strict() -> Self {
        Self::new(true)
    }

    /// Validate encryption key
    pub fn validate_key(&self, key: &[u8]) -> SecurityResult<()> {
        // Check key size
        if key.len() < MIN_KEY_SIZE {
            return Err(SecurityError::CryptoError(format!(
                "Key size too small: {} bytes (minimum: {})",
                key.len(),
                MIN_KEY_SIZE
            )));
        }

        // Check for weak keys (all zeros, all ones, etc.)
        if self.is_weak_key(key) {
            return Err(SecurityError::CryptoError("Weak key detected".to_string()));
        }

        // Check entropy in strict mode
        if self.strict_mode && !self.has_sufficient_entropy(key) {
            return Err(SecurityError::CryptoError(
                "Key has insufficient entropy".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if a key is weak
    fn is_weak_key(&self, key: &[u8]) -> bool {
        if key.is_empty() {
            return true;
        }

        // All zeros
        if key.iter().all(|&b| b == 0) {
            return true;
        }

        // All ones
        if key.iter().all(|&b| b == 0xFF) {
            return true;
        }

        // All same byte
        let first = key[0];
        if key.iter().all(|&b| b == first) {
            return true;
        }

        false
    }

    /// Check if key has sufficient entropy (basic check)
    fn has_sufficient_entropy(&self, key: &[u8]) -> bool {
        if key.len() < 16 {
            return false;
        }

        // Count unique bytes
        let mut seen = [false; 256];
        let mut unique_count = 0;

        for &byte in key {
            if !seen[byte as usize] {
                seen[byte as usize] = true;
                unique_count += 1;
            }
        }

        // Should have at least 50% unique bytes
        let min_unique = key.len() / 2;
        unique_count >= min_unique
    }

    /// Validate password strength
    pub fn validate_password(&self, password: &str, min_length: usize) -> SecurityResult<()> {
        // Check length
        if password.len() < min_length {
            return Err(SecurityError::WeakPassword(format!(
                "Password too short (minimum {} characters)",
                min_length
            )));
        }

        if password.len() > 128 {
            return Err(SecurityError::WeakPassword(
                "Password too long (maximum 128 characters)".to_string(),
            ));
        }

        // Check for common passwords
        if self.is_common_password(password) {
            return Err(SecurityError::WeakPassword(
                "Common password detected".to_string(),
            ));
        }

        // Check complexity in strict mode
        if self.strict_mode {
            self.check_password_complexity(password)?;
        }

        Ok(())
    }

    /// Check password complexity
    fn check_password_complexity(&self, password: &str) -> SecurityResult<()> {
        let has_lowercase = password.chars().any(|c| c.is_lowercase());
        let has_uppercase = password.chars().any(|c| c.is_uppercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password.chars().any(|c| !c.is_alphanumeric());

        if !(has_lowercase && has_uppercase && has_digit && has_special) {
            return Err(SecurityError::WeakPassword(
                "Password must contain lowercase, uppercase, digits, and special characters"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// Check if password is in common password list
    fn is_common_password(&self, password: &str) -> bool {
        // Top 100 most common passwords
        const COMMON_PASSWORDS: &[&str] = &[
            "password",
            "123456",
            "123456789",
            "12345678",
            "12345",
            "1234567",
            "password1",
            "123123",
            "1234567890",
            "000000",
            "admin",
            "qwerty",
            "abc123",
            "letmein",
            "welcome",
            "monkey",
            "dragon",
            "master",
            "sunshine",
            "princess",
        ];

        let lower = password.to_lowercase();
        COMMON_PASSWORDS.contains(&lower.as_str())
    }

    /// Hash password using Argon2
    pub fn hash_password(&self, password: &str) -> SecurityResult<String> {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| SecurityError::CryptoError(format!("Password hashing failed: {}", e)))
    }

    /// Verify password against hash
    pub fn verify_password(&self, password: &str, hash: &str) -> SecurityResult<bool> {
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Argon2,
        };

        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| SecurityError::CryptoError(format!("Invalid password hash: {}", e)))?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Constant-time comparison of secrets
    pub fn constant_time_compare(&self, a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        constant_time_eq(a, b)
    }
}

/// Key validator for key rotation and management
pub struct KeyValidator {
    max_age_days: i64,
}

impl Default for KeyValidator {
    fn default() -> Self {
        Self::new(MAX_KEY_AGE_DAYS)
    }
}

impl KeyValidator {
    /// Create a new key validator
    pub fn new(max_age_days: i64) -> Self {
        Self { max_age_days }
    }

    /// Check if a key should be rotated based on age
    pub fn should_rotate(&self, created_at: chrono::DateTime<chrono::Utc>) -> bool {
        let age = chrono::Utc::now().signed_duration_since(created_at);
        age.num_days() >= self.max_age_days
    }

    /// Calculate days until rotation
    pub fn days_until_rotation(&self, created_at: chrono::DateTime<chrono::Utc>) -> i64 {
        let age = chrono::Utc::now().signed_duration_since(created_at);
        self.max_age_days - age.num_days()
    }

    /// Validate key metadata
    pub fn validate_metadata(
        &self,
        created_at: chrono::DateTime<chrono::Utc>,
        algorithm: &str,
    ) -> SecurityResult<()> {
        // Check if key is too old
        if self.should_rotate(created_at) {
            return Err(SecurityError::CryptoError(
                "Key rotation required".to_string(),
            ));
        }

        // Validate algorithm
        match algorithm {
            "aes-256-gcm" | "chacha20-poly1305" => Ok(()),
            _ => Err(SecurityError::CryptoError(format!(
                "Unsupported algorithm: {}",
                algorithm
            ))),
        }
    }
}

/// Secure secret wrapper
pub struct SecureSecret {
    inner: Secret<Zeroizing<Vec<u8>>>,
}

impl SecureSecret {
    /// Create from bytes
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            inner: Secret::new(Zeroizing::new(data)),
        }
    }

    /// Expose the secret (use with caution)
    pub fn expose(&self) -> &[u8] {
        self.inner.expose_secret()
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.inner.expose_secret().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.expose_secret().is_empty()
    }
}

impl Drop for SecureSecret {
    fn drop(&mut self) {
        // Zeroizing will handle zeroing the memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_validation() {
        let validator = CryptoValidator::strict();

        // Valid key
        let valid_key: Vec<u8> = (0..32).map(|i| (i * 7) as u8).collect();
        assert!(validator.validate_key(&valid_key).is_ok());

        // Too short
        let short_key = vec![1u8; 16];
        assert!(validator.validate_key(&short_key).is_err());

        // All zeros (weak)
        let weak_key = vec![0u8; 32];
        assert!(validator.validate_key(&weak_key).is_err());

        // All ones (weak)
        let weak_key = vec![0xFFu8; 32];
        assert!(validator.validate_key(&weak_key).is_err());
    }

    #[test]
    fn test_password_validation() {
        let validator = CryptoValidator::strict();

        // Valid password
        assert!(validator.validate_password("MyP@ssw0rd123!", 12).is_ok());

        // Too short
        assert!(validator.validate_password("short", 12).is_err());

        // Common password
        assert!(validator.validate_password("password123", 8).is_err());

        // Weak complexity
        assert!(validator.validate_password("allowercase", 12).is_err());
    }

    #[test]
    fn test_password_hashing() {
        let validator = CryptoValidator::new(false);

        let password = "MySecurePassword123!";
        let hash = validator.hash_password(password).unwrap();

        // Verify correct password
        assert!(validator.verify_password(password, &hash).unwrap());

        // Verify incorrect password
        assert!(!validator.verify_password("WrongPassword", &hash).unwrap());
    }

    #[test]
    fn test_constant_time_compare() {
        let validator = CryptoValidator::new(false);

        let a = b"secret";
        let b = b"secret";
        let c = b"Secret";

        assert!(validator.constant_time_compare(a, b));
        assert!(!validator.constant_time_compare(a, c));
    }

    #[test]
    fn test_key_rotation() {
        let validator = KeyValidator::default();

        // Recent key
        let recent = chrono::Utc::now() - chrono::Duration::days(30);
        assert!(!validator.should_rotate(recent));
        assert!(validator.days_until_rotation(recent) > 0);

        // Old key
        let old = chrono::Utc::now() - chrono::Duration::days(100);
        assert!(validator.should_rotate(old));
        assert!(validator.days_until_rotation(old) < 0);
    }

    #[test]
    fn test_algorithm_validation() {
        let validator = KeyValidator::default();
        let now = chrono::Utc::now();

        assert!(validator.validate_metadata(now, "aes-256-gcm").is_ok());
        assert!(validator
            .validate_metadata(now, "chacha20-poly1305")
            .is_ok());
        assert!(validator.validate_metadata(now, "md5").is_err());
    }

    #[test]
    fn test_secure_secret() {
        let secret = SecureSecret::new(vec![1, 2, 3, 4, 5]);
        assert_eq!(secret.len(), 5);
        assert!(!secret.is_empty());
        assert_eq!(secret.expose(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_password_complexity() {
        let validator = CryptoValidator::strict();

        // All required character types
        assert!(validator.check_password_complexity("Abc123!@#").is_ok());

        // Missing special characters
        assert!(validator.check_password_complexity("Abc123456").is_err());

        // Missing uppercase
        assert!(validator.check_password_complexity("abc123!@#").is_err());
    }

    #[test]
    fn test_entropy_check() {
        let validator = CryptoValidator::strict();

        // Good entropy
        let good_key: Vec<u8> = (0..32).map(|i| (i * 7) as u8).collect();
        assert!(validator.has_sufficient_entropy(&good_key));

        // Poor entropy (repeating pattern)
        let poor_key = vec![1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2];
        assert!(!validator.has_sufficient_entropy(&poor_key));
    }
}
