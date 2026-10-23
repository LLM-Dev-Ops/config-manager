//! Version control for configurations

pub use llm_config_storage::VersionEntry;

use crate::{ConfigEntry, Result};
use chrono::Utc;

/// Version control manager
pub struct VersionControl {
    storage: llm_config_storage::file::FileStorage,
}

impl VersionControl {
    pub fn new(storage: llm_config_storage::file::FileStorage) -> Self {
        Self { storage }
    }

    /// Create a version snapshot of a configuration
    pub fn create_snapshot(
        &self,
        config: &ConfigEntry,
        change_description: Option<String>,
    ) -> Result<VersionEntry> {
        let version = VersionEntry {
            version: config.version,
            config_id: config.id,
            namespace: config.namespace.clone(),
            key: config.key.clone(),
            value: config.value.clone(),
            environment: config.environment,
            created_at: Utc::now(),
            created_by: config.metadata.updated_by.clone(),
            change_description,
        };

        self.storage.store_version(version.clone())?;

        Ok(version)
    }

    /// Get version history
    pub fn get_history(
        &self,
        namespace: &str,
        key: &str,
        env: llm_config_storage::Environment,
    ) -> Result<Vec<VersionEntry>> {
        Ok(self.storage.get_versions(namespace, key, env)?)
    }

    /// Rollback to a specific version
    pub fn rollback(
        &self,
        namespace: &str,
        key: &str,
        env: llm_config_storage::Environment,
        target_version: u64,
    ) -> Result<Option<ConfigEntry>> {
        let versions = self.get_history(namespace, key, env)?;

        let target = versions.iter().find(|v| v.version == target_version);

        if let Some(version) = target {
            let mut config = ConfigEntry::new(
                version.namespace.clone(),
                version.key.clone(),
                version.value.clone(),
                version.environment,
            );

            // Increment version for the rollback
            config.version = versions.first().map(|v| v.version + 1).unwrap_or(1);
            config.metadata.updated_at = Utc::now();

            self.storage.set(config.clone())?;

            // Create a snapshot of the rollback
            self.create_snapshot(
                &config,
                Some(format!("Rollback to version {}", target_version)),
            )?;

            Ok(Some(config))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_config_storage::{file::FileStorage, ConfigValue, Environment};
    use tempfile::TempDir;

    #[test]
    fn test_version_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();
        let vc = VersionControl::new(storage.clone());

        let config = ConfigEntry::new(
            "test",
            "key",
            ConfigValue::String("value".to_string()),
            Environment::Development,
        );

        let snapshot = vc
            .create_snapshot(&config, Some("Initial version".to_string()))
            .unwrap();

        assert_eq!(snapshot.version, config.version);
        assert_eq!(snapshot.namespace, config.namespace);
        assert_eq!(snapshot.key, config.key);
    }

    #[test]
    fn test_version_history() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();
        let vc = VersionControl::new(storage.clone());

        let mut config = ConfigEntry::new(
            "test",
            "key",
            ConfigValue::String("v1".to_string()),
            Environment::Development,
        );

        vc.create_snapshot(&config, Some("Version 1".to_string()))
            .unwrap();

        config.version = 2;
        config.value = ConfigValue::String("v2".to_string());
        vc.create_snapshot(&config, Some("Version 2".to_string()))
            .unwrap();

        let history = vc
            .get_history("test", "key", Environment::Development)
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, 2); // Most recent first
        assert_eq!(history[1].version, 1);
    }
}
