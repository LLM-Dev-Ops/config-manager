//! Security policy enforcement

use crate::errors::{SecurityError, SecurityResult};
use crate::SecurityContext;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Security policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// Allowed IP ranges (CIDR notation)
    pub allowed_ip_ranges: Vec<String>,
    /// Blocked IP addresses
    pub blocked_ips: Vec<String>,
    /// Require TLS
    pub require_tls: bool,
    /// Minimum TLS version
    pub min_tls_version: String,
    /// Allowed origins for CORS
    pub allowed_origins: Vec<String>,
    /// Maximum request size in bytes
    pub max_request_size: usize,
    /// Session timeout in seconds
    pub session_timeout: u64,
    /// Require MFA for sensitive operations
    pub require_mfa: bool,
    /// Allowed API endpoints
    pub allowed_endpoints: Vec<String>,
    /// Blocked endpoints
    pub blocked_endpoints: Vec<String>,
    /// Enable audit logging
    pub enable_audit: bool,
    /// Data classification levels
    pub data_classifications: Vec<DataClassification>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            allowed_ip_ranges: vec!["0.0.0.0/0".to_string()],
            blocked_ips: vec![],
            require_tls: true,
            min_tls_version: "1.2".to_string(),
            allowed_origins: vec![],
            max_request_size: 10 * 1024 * 1024, // 10MB
            session_timeout: 3600,               // 1 hour
            require_mfa: false,
            allowed_endpoints: vec![],
            blocked_endpoints: vec![],
            enable_audit: true,
            data_classifications: vec![
                DataClassification::Public,
                DataClassification::Internal,
                DataClassification::Confidential,
                DataClassification::Secret,
            ],
        }
    }
}

/// Data classification levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Secret,
}

/// Policy enforcer
pub struct PolicyEnforcer {
    policy: SecurityPolicy,
    blocked_ips: HashSet<String>,
}

impl PolicyEnforcer {
    /// Create a new policy enforcer
    pub fn new(policy: SecurityPolicy) -> Self {
        let blocked_ips = policy.blocked_ips.iter().cloned().collect();
        Self {
            policy,
            blocked_ips,
        }
    }

    /// Create with default policy
    pub fn default() -> Self {
        Self::new(SecurityPolicy::default())
    }

    /// Check if an IP is allowed
    pub fn check_ip(&self, ip: &str) -> SecurityResult<()> {
        // Check if IP is blocked
        if self.blocked_ips.contains(ip) {
            return Err(SecurityError::PolicyViolation(format!(
                "IP address {} is blocked",
                ip
            )));
        }

        // Check if IP is in allowed ranges
        if !self.policy.allowed_ip_ranges.is_empty()
            && !self.policy.allowed_ip_ranges.contains(&"0.0.0.0/0".to_string())
        {
            // In a real implementation, we would use proper CIDR matching
            // For now, just check if IP is in the list
            if !self.policy.allowed_ip_ranges.contains(&ip.to_string()) {
                return Err(SecurityError::PolicyViolation(format!(
                    "IP address {} is not in allowed ranges",
                    ip
                )));
            }
        }

        Ok(())
    }

    /// Check if TLS is required
    pub fn check_tls(&self, is_tls: bool, version: &str) -> SecurityResult<()> {
        if self.policy.require_tls && !is_tls {
            return Err(SecurityError::InsecureProtocol(
                "TLS is required".to_string(),
            ));
        }

        if is_tls {
            let min_version = self.parse_tls_version(&self.policy.min_tls_version);
            let actual_version = self.parse_tls_version(version);

            if actual_version < min_version {
                return Err(SecurityError::InsecureProtocol(format!(
                    "TLS version {} is below minimum {}",
                    version, self.policy.min_tls_version
                )));
            }
        }

        Ok(())
    }

    /// Parse TLS version string to number for comparison
    fn parse_tls_version(&self, version: &str) -> u32 {
        match version {
            "1.0" => 10,
            "1.1" => 11,
            "1.2" => 12,
            "1.3" => 13,
            _ => 0,
        }
    }

    /// Check CORS origin
    pub fn check_origin(&self, origin: &str) -> SecurityResult<()> {
        if self.policy.allowed_origins.is_empty() {
            return Ok(()); // No CORS restrictions
        }

        if self.policy.allowed_origins.contains(&origin.to_string())
            || self.policy.allowed_origins.contains(&"*".to_string())
        {
            Ok(())
        } else {
            Err(SecurityError::PolicyViolation(format!(
                "Origin {} is not allowed",
                origin
            )))
        }
    }

    /// Maximum actual body size permitted by this policy.
    pub fn max_request_size(&self) -> usize {
        self.policy.max_request_size
    }

    /// Check request size
    pub fn check_request_size(&self, size: usize) -> SecurityResult<()> {
        if size > self.policy.max_request_size {
            return Err(SecurityError::RequestTooLarge(size));
        }
        Ok(())
    }

    /// Check if endpoint is allowed
    pub fn check_endpoint(&self, endpoint: &str) -> SecurityResult<()> {
        // Check if endpoint is blocked
        if self.is_endpoint_blocked(endpoint) {
            return Err(SecurityError::PolicyViolation(format!(
                "Endpoint {} is blocked",
                endpoint
            )));
        }

        // Check if endpoint is in allowed list (if list is not empty)
        if !self.policy.allowed_endpoints.is_empty()
            && !self.is_endpoint_allowed(endpoint)
        {
            return Err(SecurityError::PolicyViolation(format!(
                "Endpoint {} is not in allowed list",
                endpoint
            )));
        }

        Ok(())
    }

    /// Check if endpoint matches allowed patterns
    fn is_endpoint_allowed(&self, endpoint: &str) -> bool {
        self.policy
            .allowed_endpoints
            .iter()
            .any(|pattern| self.matches_pattern(endpoint, pattern))
    }

    /// Check if endpoint matches blocked patterns
    fn is_endpoint_blocked(&self, endpoint: &str) -> bool {
        self.policy
            .blocked_endpoints
            .iter()
            .any(|pattern| self.matches_pattern(endpoint, pattern))
    }

    /// Simple pattern matching (supports wildcards)
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            text.starts_with(prefix)
        } else if pattern.starts_with('*') {
            let suffix = &pattern[1..];
            text.ends_with(suffix)
        } else {
            text == pattern
        }
    }

    /// Check if MFA is required
    pub fn check_mfa(&self, has_mfa: bool, is_sensitive: bool) -> SecurityResult<()> {
        if self.policy.require_mfa && is_sensitive && !has_mfa {
            return Err(SecurityError::PolicyViolation(
                "MFA is required for sensitive operations".to_string(),
            ));
        }
        Ok(())
    }

    /// Check session validity
    pub fn check_session(
        &self,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> SecurityResult<()> {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(created_at)
            .num_seconds() as u64;

        if elapsed > self.policy.session_timeout {
            return Err(SecurityError::InvalidSession(
                "Session expired".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate data classification
    pub fn check_data_classification(
        &self,
        classification: &DataClassification,
    ) -> SecurityResult<()> {
        if !self.policy.data_classifications.contains(classification) {
            return Err(SecurityError::PolicyViolation(format!(
                "Data classification {:?} is not allowed",
                classification
            )));
        }
        Ok(())
    }

    /// Comprehensive security check
    pub fn check_request(&self, context: &SecurityContext) -> SecurityResult<()> {
        // Check IP
        self.check_ip(&context.ip_address)?;

        // Check session if present
        if let Some(ref session_id) = context.session_id {
            if !session_id.is_empty() {
                self.check_session(context.timestamp)?;
            }
        }

        Ok(())
    }

    /// Add an IP to the blocklist
    pub fn block_ip(&mut self, ip: String) {
        self.blocked_ips.insert(ip.clone());
        if !self.policy.blocked_ips.contains(&ip) {
            self.policy.blocked_ips.push(ip);
        }
    }

    /// Remove an IP from the blocklist
    pub fn unblock_ip(&mut self, ip: &str) {
        self.blocked_ips.remove(ip);
        self.policy.blocked_ips.retain(|x| x != ip);
    }

    /// Get the current policy
    pub fn get_policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Update the policy
    pub fn update_policy(&mut self, policy: SecurityPolicy) {
        self.blocked_ips = policy.blocked_ips.iter().cloned().collect();
        self.policy = policy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_blocking() {
        let mut policy = SecurityPolicy::default();
        policy.blocked_ips.push("192.168.1.100".to_string());

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.check_ip("192.168.1.1").is_ok());
        assert!(enforcer.check_ip("192.168.1.100").is_err());
    }

    #[test]
    fn test_tls_check() {
        let policy = SecurityPolicy {
            require_tls: true,
            min_tls_version: "1.2".to_string(),
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.check_tls(true, "1.2").is_ok());
        assert!(enforcer.check_tls(true, "1.3").is_ok());
        assert!(enforcer.check_tls(true, "1.1").is_err());
        assert!(enforcer.check_tls(false, "1.2").is_err());
    }

    #[test]
    fn test_origin_check() {
        let policy = SecurityPolicy {
            allowed_origins: vec!["https://example.com".to_string()],
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer
            .check_origin("https://example.com")
            .is_ok());
        assert!(enforcer
            .check_origin("https://evil.com")
            .is_err());
    }

    #[test]
    fn test_request_size() {
        let policy = SecurityPolicy {
            max_request_size: 1024,
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.check_request_size(512).is_ok());
        assert!(enforcer.check_request_size(2048).is_err());
    }

    #[test]
    fn test_endpoint_patterns() {
        let policy = SecurityPolicy {
            allowed_endpoints: vec!["/api/*".to_string()],
            blocked_endpoints: vec!["/api/admin/*".to_string()],
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.check_endpoint("/api/users").is_ok());
        assert!(enforcer.check_endpoint("/api/admin/users").is_err());
    }

    #[test]
    fn test_mfa_requirement() {
        let policy = SecurityPolicy {
            require_mfa: true,
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer.check_mfa(true, true).is_ok());
        assert!(enforcer.check_mfa(false, false).is_ok());
        assert!(enforcer.check_mfa(false, true).is_err());
    }

    #[test]
    fn test_session_timeout() {
        let policy = SecurityPolicy {
            session_timeout: 3600,
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        // Recent session
        let recent = chrono::Utc::now() - chrono::Duration::seconds(1800);
        assert!(enforcer.check_session(recent).is_ok());

        // Expired session
        let expired = chrono::Utc::now() - chrono::Duration::seconds(7200);
        assert!(enforcer.check_session(expired).is_err());
    }

    #[test]
    fn test_data_classification() {
        let policy = SecurityPolicy {
            data_classifications: vec![
                DataClassification::Public,
                DataClassification::Internal,
            ],
            ..Default::default()
        };

        let enforcer = PolicyEnforcer::new(policy);

        assert!(enforcer
            .check_data_classification(&DataClassification::Public)
            .is_ok());
        assert!(enforcer
            .check_data_classification(&DataClassification::Secret)
            .is_err());
    }

    #[test]
    fn test_dynamic_blocking() {
        let mut enforcer = PolicyEnforcer::default();

        enforcer.block_ip("10.0.0.1".to_string());
        assert!(enforcer.check_ip("10.0.0.1").is_err());

        enforcer.unblock_ip("10.0.0.1");
        assert!(enforcer.check_ip("10.0.0.1").is_ok());
    }

    #[test]
    fn test_comprehensive_check() {
        let enforcer = PolicyEnforcer::default();

        let context = SecurityContext::new("user123", "192.168.1.1")
            .with_session("sess_abc");

        assert!(enforcer.check_request(&context).is_ok());
    }

    #[test]
    fn test_pattern_matching() {
        let enforcer = PolicyEnforcer::default();

        assert!(enforcer.matches_pattern("/api/users", "/api/*"));
        assert!(enforcer.matches_pattern("/api/users", "*/users"));
        assert!(enforcer.matches_pattern("/api/users", "/api/users"));
        assert!(!enforcer.matches_pattern("/api/users", "/admin/*"));
    }
}
