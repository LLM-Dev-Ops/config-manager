//! Audit log validation

use crate::errors::{SecurityError, SecurityResult};
use serde::{Deserialize, Serialize};

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Enable integrity verification
    pub enable_integrity_check: bool,
    /// Enable completeness check
    pub enable_completeness_check: bool,
    /// Expected event rate (events per second)
    pub expected_event_rate: Option<f64>,
    /// Maximum gap between events (seconds)
    pub max_event_gap_seconds: Option<u64>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_integrity_check: true,
            enable_completeness_check: true,
            expected_event_rate: None,
            max_event_gap_seconds: Some(300), // 5 minutes
        }
    }
}

/// Audit validator
pub struct AuditValidator {
    config: AuditConfig,
}

impl Default for AuditValidator {
    fn default() -> Self {
        Self::new(AuditConfig::default())
    }
}

impl AuditValidator {
    /// Create a new audit validator
    pub fn new(config: AuditConfig) -> Self {
        Self { config }
    }

    /// Validate audit event
    pub fn validate_event(&self, event: &AuditEvent) -> SecurityResult<()> {
        // Validate required fields
        if event.timestamp.is_none() {
            return Err(SecurityError::AuditError("Missing timestamp".to_string()));
        }

        if event.user_id.is_empty() {
            return Err(SecurityError::AuditError("Missing user_id".to_string()));
        }

        if event.action.is_empty() {
            return Err(SecurityError::AuditError("Missing action".to_string()));
        }

        if event.resource.is_empty() {
            return Err(SecurityError::AuditError("Missing resource".to_string()));
        }

        // Validate timestamp is not in the future
        if let Some(timestamp) = event.timestamp {
            if timestamp > chrono::Utc::now() {
                return Err(SecurityError::AuditError(
                    "Timestamp in the future".to_string(),
                ));
            }
        }

        // Validate severity
        match event.severity {
            EventSeverity::Low
            | EventSeverity::Medium
            | EventSeverity::High
            | EventSeverity::Critical => Ok(()),
        }
    }

    /// Check for suspicious patterns
    pub fn check_suspicious_patterns(&self, event: &AuditEvent) -> SecurityResult<()> {
        // Check for potential privilege escalation
        if event.action.contains("permission")
            && event.action.contains("grant")
            && event.metadata.get("new_role").map(String::as_str) == Some("admin")
        {
            return Err(SecurityError::SuspiciousActivity(
                "Potential privilege escalation detected".to_string(),
            ));
        }

        // Check for mass deletion
        if event.action.contains("delete") {
            if let Some(count_str) = event.metadata.get("count") {
                if let Ok(count) = count_str.parse::<usize>() {
                    if count > 1000 {
                        return Err(SecurityError::SuspiciousActivity(
                            "Mass deletion detected".to_string(),
                        ));
                    }
                }
            }
        }

        // Check for unusual access patterns
        if event.action.contains("access") {
            if let Some(ip) = event.metadata.get("ip_address") {
                // In a real implementation, we would check against known patterns
                if ip.starts_with("0.") || ip.starts_with("255.") {
                    return Err(SecurityError::SuspiciousActivity(
                        "Access from suspicious IP".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate audit log sequence
    pub fn validate_sequence(&self, events: &[AuditEvent]) -> SecurityResult<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Check chronological order
        for i in 1..events.len() {
            if let (Some(prev), Some(curr)) = (events[i - 1].timestamp, events[i].timestamp) {
                if curr < prev {
                    return Err(SecurityError::AuditError(
                        "Events not in chronological order".to_string(),
                    ));
                }

                // Check for suspicious gaps
                if let Some(max_gap) = self.config.max_event_gap_seconds {
                    let gap = curr.signed_duration_since(prev).num_seconds();
                    if gap > max_gap as i64 {
                        return Err(SecurityError::AuditError(format!(
                            "Suspicious gap of {} seconds between events",
                            gap
                        )));
                    }
                }
            }
        }

        // Check for missing sequence numbers (if present)
        let sequence_numbers: Vec<_> = events
            .iter()
            .filter_map(|e| e.metadata.get("sequence_number"))
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();

        if !sequence_numbers.is_empty() {
            for i in 1..sequence_numbers.len() {
                if sequence_numbers[i] != sequence_numbers[i - 1] + 1 {
                    return Err(SecurityError::AuditError(
                        "Missing sequence number in audit log".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Calculate audit statistics
    pub fn calculate_stats(&self, events: &[AuditEvent]) -> AuditStats {
        if events.is_empty() {
            return AuditStats::default();
        }

        let mut stats = AuditStats {
            total_events: events.len(),
            ..AuditStats::default()
        };

        // Count by severity
        for event in events {
            match event.severity {
                EventSeverity::Low => stats.low_severity += 1,
                EventSeverity::Medium => stats.medium_severity += 1,
                EventSeverity::High => stats.high_severity += 1,
                EventSeverity::Critical => stats.critical_severity += 1,
            }
        }

        // Calculate time range
        if let (Some(first), Some(last)) = (
            events.first().and_then(|e| e.timestamp),
            events.last().and_then(|e| e.timestamp),
        ) {
            stats.time_range_seconds = last.signed_duration_since(first).num_seconds();

            if stats.time_range_seconds > 0 {
                stats.events_per_second =
                    stats.total_events as f64 / stats.time_range_seconds as f64;
            }
        }

        // Check against expected rate
        if let Some(expected_rate) = self.config.expected_event_rate {
            if stats.events_per_second < expected_rate * 0.5 {
                stats
                    .anomalies
                    .push("Event rate significantly below expected".to_string());
            } else if stats.events_per_second > expected_rate * 2.0 {
                stats
                    .anomalies
                    .push("Event rate significantly above expected".to_string());
            }
        }

        stats
    }
}

/// Audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub result: String,
    pub severity: EventSeverity,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Event severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Audit statistics
#[derive(Debug, Clone, Default)]
pub struct AuditStats {
    pub total_events: usize,
    pub low_severity: usize,
    pub medium_severity: usize,
    pub high_severity: usize,
    pub critical_severity: usize,
    pub time_range_seconds: i64,
    pub events_per_second: f64,
    pub anomalies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_event(user_id: &str, action: &str) -> AuditEvent {
        AuditEvent {
            timestamp: Some(chrono::Utc::now()),
            user_id: user_id.to_string(),
            action: action.to_string(),
            resource: "test_resource".to_string(),
            result: "success".to_string(),
            severity: EventSeverity::Medium,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_event_validation() {
        let validator = AuditValidator::default();

        // Valid event
        let event = create_test_event("user123", "read");
        assert!(validator.validate_event(&event).is_ok());

        // Missing user_id
        let mut event = create_test_event("", "read");
        assert!(validator.validate_event(&event).is_err());

        // Missing action
        event = create_test_event("user123", "");
        assert!(validator.validate_event(&event).is_err());
    }

    #[test]
    fn test_suspicious_patterns() {
        let validator = AuditValidator::default();

        // Privilege escalation
        let mut event = create_test_event("user123", "permission_grant");
        event
            .metadata
            .insert("new_role".to_string(), "admin".to_string());
        assert!(validator.check_suspicious_patterns(&event).is_err());

        // Mass deletion
        let mut event = create_test_event("user123", "delete");
        event
            .metadata
            .insert("count".to_string(), "5000".to_string());
        assert!(validator.check_suspicious_patterns(&event).is_err());

        // Suspicious IP
        let mut event = create_test_event("user123", "access");
        event
            .metadata
            .insert("ip_address".to_string(), "0.0.0.1".to_string());
        assert!(validator.check_suspicious_patterns(&event).is_err());
    }

    #[test]
    fn test_sequence_validation() {
        let validator = AuditValidator::default();

        // Valid sequence
        let mut events = vec![];
        let base = chrono::Utc::now();
        for i in 0..5 {
            let mut event = create_test_event("user123", "read");
            event.timestamp = Some(base + chrono::Duration::seconds(i));
            events.push(event);
        }
        assert!(validator.validate_sequence(&events).is_ok());

        // Out of order
        events.reverse();
        assert!(validator.validate_sequence(&events).is_err());
    }

    #[test]
    fn test_stats_calculation() {
        let validator = AuditValidator::default();

        let mut events = vec![];
        let base = chrono::Utc::now();

        // Add events with different severities
        for i in 0..10 {
            let mut event = create_test_event("user123", "read");
            event.timestamp = Some(base + chrono::Duration::seconds(i));
            event.severity = match i % 4 {
                0 => EventSeverity::Low,
                1 => EventSeverity::Medium,
                2 => EventSeverity::High,
                _ => EventSeverity::Critical,
            };
            events.push(event);
        }

        let stats = validator.calculate_stats(&events);
        assert_eq!(stats.total_events, 10);
        assert!(stats.low_severity > 0);
        assert!(stats.medium_severity > 0);
        assert!(stats.high_severity > 0);
        assert!(stats.critical_severity > 0);
    }

    #[test]
    fn test_gap_detection() {
        let config = AuditConfig {
            max_event_gap_seconds: Some(60),
            ..Default::default()
        };
        let validator = AuditValidator::new(config);

        let base = chrono::Utc::now();
        let mut events = vec![];

        // First event
        let mut event1 = create_test_event("user123", "read");
        event1.timestamp = Some(base);
        events.push(event1);

        // Second event with large gap
        let mut event2 = create_test_event("user123", "read");
        event2.timestamp = Some(base + chrono::Duration::seconds(120));
        events.push(event2);

        assert!(validator.validate_sequence(&events).is_err());
    }
}
