//! Audit log storage backends

use crate::{events::AuditEvent, AuditError, Result};
use chrono::{DateTime, Utc};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Trait for audit log storage backends
pub trait AuditStorage: Send + Sync {
    /// Store an audit event
    fn store(&self, event: &AuditEvent) -> Result<()>;

    /// Query audit events within a time range
    fn query(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>>;

    /// Query events for a specific user
    fn query_by_user(&self, user: &str, limit: Option<usize>) -> Result<Vec<AuditEvent>>;

    /// Get total event count
    fn count(&self) -> Result<usize>;
}

/// File-based audit log storage
pub struct FileAuditStorage {
    log_path: PathBuf,
}

impl FileAuditStorage {
    /// Create a new file-based audit storage
    pub fn new(log_dir: impl AsRef<Path>) -> Result<Self> {
        let log_dir = log_dir.as_ref();
        std::fs::create_dir_all(log_dir)?;

        let log_path = log_dir.join("audit.log");

        Ok(Self { log_path })
    }

    /// Get the current log file path
    #[allow(dead_code)]
    fn log_file_path(&self) -> &Path {
        &self.log_path
    }

    /// Rotate log files (for future implementation)
    #[allow(dead_code)]
    fn rotate_logs(&self) -> Result<()> {
        // TODO: Implement log rotation
        // - Check file size
        // - Compress old logs
        // - Keep N most recent log files
        Ok(())
    }
}

impl AuditStorage for FileAuditStorage {
    fn store(&self, event: &AuditEvent) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        let json =
            serde_json::to_string(event).map_err(|e| AuditError::Serialization(e.to_string()))?;

        writeln!(file, "{}", json)?;
        file.sync_all()?;

        Ok(())
    }

    fn query(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEvent>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let event: AuditEvent = serde_json::from_str(&line)
                .map_err(|e| AuditError::Serialization(e.to_string()))?;

            if event.timestamp >= start && event.timestamp <= end {
                events.push(event);

                if let Some(limit) = limit {
                    if events.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(events)
    }

    fn query_by_user(&self, user: &str, limit: Option<usize>) -> Result<Vec<AuditEvent>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let event: AuditEvent = serde_json::from_str(&line)
                .map_err(|e| AuditError::Serialization(e.to_string()))?;

            if event.user == user {
                events.push(event);

                if let Some(limit) = limit {
                    if events.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(events)
    }

    fn count(&self) -> Result<usize> {
        if !self.log_path.exists() {
            return Ok(0);
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let count = reader.lines().filter(|l| l.is_ok()).count();

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AuditEventType;
    use tempfile::TempDir;

    #[test]
    fn test_file_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileAuditStorage::new(temp_dir.path()).unwrap();

        // Storage directory should exist (file is created on first write)
        assert!(temp_dir.path().exists());
        assert_eq!(storage.count().unwrap(), 0);
    }

    #[test]
    fn test_store_and_query() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileAuditStorage::new(temp_dir.path()).unwrap();

        let event = AuditEvent::new(
            AuditEventType::ConfigCreated {
                namespace: "test".to_string(),
                key: "key1".to_string(),
                environment: "dev".to_string(),
            },
            "test-user",
        );

        storage.store(&event).unwrap();

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let events = storage.query(start, end, None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
    }

    #[test]
    fn test_query_by_user() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileAuditStorage::new(temp_dir.path()).unwrap();

        // Store events for different users
        for i in 0..5 {
            let user = if i % 2 == 0 { "user1" } else { "user2" };
            let event = AuditEvent::new(
                AuditEventType::ConfigAccessed {
                    namespace: "test".to_string(),
                    key: format!("key{}", i),
                    environment: "dev".to_string(),
                },
                user,
            );
            storage.store(&event).unwrap();
        }

        let user1_events = storage.query_by_user("user1", None).unwrap();
        let user2_events = storage.query_by_user("user2", None).unwrap();

        assert_eq!(user1_events.len(), 3); // Events 0, 2, 4
        assert_eq!(user2_events.len(), 2); // Events 1, 3
    }

    #[test]
    fn test_count() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileAuditStorage::new(temp_dir.path()).unwrap();

        assert_eq!(storage.count().unwrap(), 0);

        for i in 0..10 {
            let event = AuditEvent::new(
                AuditEventType::ConfigAccessed {
                    namespace: "test".to_string(),
                    key: format!("key{}", i),
                    environment: "dev".to_string(),
                },
                "user",
            );
            storage.store(&event).unwrap();
        }

        assert_eq!(storage.count().unwrap(), 10);
    }

    #[test]
    fn test_query_with_limit() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileAuditStorage::new(temp_dir.path()).unwrap();

        for i in 0..20 {
            let event = AuditEvent::new(
                AuditEventType::ConfigAccessed {
                    namespace: "test".to_string(),
                    key: format!("key{}", i),
                    environment: "dev".to_string(),
                },
                "user",
            );
            storage.store(&event).unwrap();
        }

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let events = storage.query(start, end, Some(10)).unwrap();
        assert_eq!(events.len(), 10);
    }
}
