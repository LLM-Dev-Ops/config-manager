//! Audit event definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Audit event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Types of audit events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEventType {
    /// Configuration was created
    ConfigCreated {
        namespace: String,
        key: String,
        environment: String,
    },

    /// Configuration was updated
    ConfigUpdated {
        namespace: String,
        key: String,
        environment: String,
        old_version: u64,
        new_version: u64,
    },

    /// Configuration was deleted
    ConfigDeleted {
        namespace: String,
        key: String,
        environment: String,
    },

    /// Configuration was accessed (read)
    ConfigAccessed {
        namespace: String,
        key: String,
        environment: String,
    },

    /// Configuration was rolled back
    ConfigRolledBack {
        namespace: String,
        key: String,
        environment: String,
        from_version: u64,
        to_version: u64,
    },

    /// Secret was created or updated
    SecretModified {
        namespace: String,
        key: String,
        environment: String,
    },

    /// Secret was accessed
    SecretAccessed {
        namespace: String,
        key: String,
        environment: String,
    },

    /// Authentication attempt
    AuthAttempt {
        user: String,
        method: String,
        success: bool,
    },

    /// Authorization check
    AuthzCheck {
        user: String,
        resource: String,
        action: String,
        allowed: bool,
    },

    /// System event
    SystemEvent { component: String, message: String },

    /// Security event
    SecurityEvent {
        threat_type: String,
        details: String,
    },
}

/// Complete audit event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Event severity
    pub severity: AuditSeverity,

    /// Event type with details
    #[serde(flatten)]
    pub event_type: AuditEventType,

    /// User who triggered the event
    pub user: String,

    /// Source IP address (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,

    /// Request ID for correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType, user: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            severity: AuditSeverity::Info,
            event_type,
            user: user.into(),
            source_ip: None,
            request_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the severity level
    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the source IP address
    pub fn with_source_ip(mut self, ip: impl Into<String>) -> Self {
        self.source_ip = Some(ip.into());
        self
    }

    /// Set the request ID
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get a human-readable summary of the event
    pub fn summary(&self) -> String {
        match &self.event_type {
            AuditEventType::ConfigCreated { namespace, key, .. } => {
                format!("Created config {}/{}  by {}", namespace, key, self.user)
            }
            AuditEventType::ConfigUpdated { namespace, key, .. } => {
                format!("Updated config {}/{} by {}", namespace, key, self.user)
            }
            AuditEventType::ConfigDeleted { namespace, key, .. } => {
                format!("Deleted config {}/{} by {}", namespace, key, self.user)
            }
            AuditEventType::ConfigAccessed { namespace, key, .. } => {
                format!("Accessed config {}/{} by {}", namespace, key, self.user)
            }
            AuditEventType::ConfigRolledBack {
                namespace,
                key,
                from_version,
                to_version,
                ..
            } => {
                format!(
                    "Rolled back config {}/{} from v{} to v{} by {}",
                    namespace, key, from_version, to_version, self.user
                )
            }
            AuditEventType::SecretModified { namespace, key, .. } => {
                format!("Modified secret {}/{} by {}", namespace, key, self.user)
            }
            AuditEventType::SecretAccessed { namespace, key, .. } => {
                format!("Accessed secret {}/{} by {}", namespace, key, self.user)
            }
            AuditEventType::AuthAttempt {
                user,
                method,
                success,
            } => {
                format!(
                    "Auth attempt for {} via {} - {}",
                    user,
                    method,
                    if *success { "SUCCESS" } else { "FAILED" }
                )
            }
            AuditEventType::AuthzCheck {
                user,
                resource,
                action,
                allowed,
            } => {
                format!(
                    "Authz check: {} -> {} on {} - {}",
                    user,
                    action,
                    resource,
                    if *allowed { "ALLOWED" } else { "DENIED" }
                )
            }
            AuditEventType::SystemEvent { component, message } => {
                format!("System: {} - {}", component, message)
            }
            AuditEventType::SecurityEvent {
                threat_type,
                details,
            } => {
                format!("Security: {} - {}", threat_type, details)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = AuditEvent::new(
            AuditEventType::ConfigCreated {
                namespace: "test".to_string(),
                key: "key1".to_string(),
                environment: "dev".to_string(),
            },
            "test-user",
        );

        assert_eq!(event.user, "test-user");
        assert_eq!(event.severity, AuditSeverity::Info);
    }

    #[test]
    fn test_event_with_metadata() {
        let event = AuditEvent::new(
            AuditEventType::SystemEvent {
                component: "test".to_string(),
                message: "test message".to_string(),
            },
            "system",
        )
        .with_severity(AuditSeverity::Warning)
        .with_source_ip("127.0.0.1")
        .with_metadata("key", "value");

        assert_eq!(event.severity, AuditSeverity::Warning);
        assert_eq!(event.source_ip, Some("127.0.0.1".to_string()));
        assert_eq!(event.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_event_summary() {
        let event = AuditEvent::new(
            AuditEventType::ConfigUpdated {
                namespace: "ns".to_string(),
                key: "key".to_string(),
                environment: "prod".to_string(),
                old_version: 1,
                new_version: 2,
            },
            "admin",
        );

        let summary = event.summary();
        assert!(summary.contains("Updated"));
        assert!(summary.contains("ns/key"));
    }

    #[test]
    fn test_event_serialization() {
        let event = AuditEvent::new(
            AuditEventType::ConfigCreated {
                namespace: "test".to_string(),
                key: "key".to_string(),
                environment: "dev".to_string(),
            },
            "user",
        );

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.user, deserialized.user);
    }
}
