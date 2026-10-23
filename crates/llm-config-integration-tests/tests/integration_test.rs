//! Integration tests for LLM Config Manager
//!
//! These tests verify that all components work together correctly
//! in real-world scenarios.

use llm_config_audit::{AuditEventType, AuditLogger, FileAuditStorage};
use llm_config_cache::CacheManager;
use llm_config_core::{ConfigManager, ConfigValue, Environment};
use llm_config_crypto::{Algorithm, SecretKey};
use llm_config_rbac::{permissions::*, PolicyEnforcer, Role, RoleAssignment};
use llm_config_templates::{Template, TemplateEngine};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn test_end_to_end_config_lifecycle() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let manager = ConfigManager::new(temp_dir.path())
        .unwrap()
        .with_encryption_key(key);

    // Create configuration
    let entry = manager
        .set(
            "production/api",
            "endpoint",
            ConfigValue::String("https://api.example.com".to_string()),
            Environment::Production,
            "admin@example.com",
        )
        .unwrap();

    assert_eq!(entry.version, 1);
    assert_eq!(entry.namespace, "production/api");

    // Read configuration
    let retrieved = manager
        .get("production/api", "endpoint", Environment::Production)
        .unwrap()
        .unwrap();

    assert_eq!(retrieved.id, entry.id);

    // Update configuration
    let updated = manager
        .set(
            "production/api",
            "endpoint",
            ConfigValue::String("https://api-v2.example.com".to_string()),
            Environment::Production,
            "admin@example.com",
        )
        .unwrap();

    assert_eq!(updated.version, 2);

    // Verify history
    let history = manager
        .get_history("production/api", "endpoint", Environment::Production)
        .unwrap();

    assert_eq!(history.len(), 2);

    // Rollback
    let rolled_back = manager
        .rollback("production/api", "endpoint", Environment::Production, 1)
        .unwrap()
        .unwrap();

    assert_eq!(rolled_back.version, 3); // New version after rollback
    if let ConfigValue::String(s) = &rolled_back.value {
        assert_eq!(s, "https://api.example.com");
    } else {
        panic!("Expected string value");
    }

    // Delete configuration
    let deleted = manager
        .delete("production/api", "endpoint", Environment::Production)
        .unwrap();

    assert!(deleted);

    // Verify deletion
    let after_delete = manager
        .get("production/api", "endpoint", Environment::Production)
        .unwrap();

    assert!(after_delete.is_none());
}

#[test]
fn test_secrets_encryption_integration() {
    let temp_dir = TempDir::new().unwrap();
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let manager = ConfigManager::new(temp_dir.path())
        .unwrap()
        .with_encryption_key(key.clone());

    // Store secret
    let secret_data = b"super-secret-api-key-12345";
    let entry = manager
        .set_secret(
            "production/api",
            "api_key",
            secret_data,
            Environment::Production,
            "admin",
        )
        .unwrap();

    // Verify secret is encrypted in storage
    assert!(matches!(entry.value, ConfigValue::Secret(_)));

    // Retrieve and decrypt secret using get_secret()
    let retrieved = manager
        .get_secret("production/api", "api_key", Environment::Production)
        .unwrap()
        .unwrap();

    assert_eq!(retrieved, secret_data);

    // Verify that get() also decrypts secrets automatically
    let entry2 = manager
        .get("production/api", "api_key", Environment::Production)
        .unwrap()
        .unwrap();

    // When retrieved via get(), secrets are automatically decrypted to strings
    if let ConfigValue::String(decrypted) = entry2.value {
        assert_eq!(decrypted.as_bytes(), secret_data);
    } else {
        panic!("Expected decrypted string value");
    }
}

#[test]
fn test_rbac_integration() {
    let mut enforcer = PolicyEnforcer::new();

    // Assign roles
    enforcer.assign_role(RoleAssignment::new("alice", Role::Admin));
    enforcer.assign_role(RoleAssignment::new("bob", Role::Editor));
    enforcer.assign_role(RoleAssignment::new("charlie", Role::Viewer));

    // Admin can do everything
    assert!(enforcer
        .check_permission("alice", &Resource::Config, &Action::Delete, None)
        .is_ok());
    assert!(enforcer
        .check_permission("alice", &Resource::System, &Action::Update, None)
        .is_ok());

    // Editor can modify configs but not system
    assert!(enforcer
        .check_permission("bob", &Resource::Config, &Action::Update, None)
        .is_ok());
    assert!(enforcer
        .check_permission("bob", &Resource::System, &Action::Update, None)
        .is_err());

    // Viewer can only read
    assert!(enforcer
        .check_permission("charlie", &Resource::Config, &Action::Read, None)
        .is_ok());
    assert!(enforcer
        .check_permission("charlie", &Resource::Config, &Action::Update, None)
        .is_err());
    assert!(enforcer
        .check_permission("charlie", &Resource::Secret, &Action::Read, None)
        .is_err());
}

#[tokio::test]
async fn test_audit_logging_integration() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(FileAuditStorage::new(temp_dir.path()).unwrap());
    let logger = AuditLogger::new(storage);

    // Log various events
    logger
        .log_event(
            AuditEventType::ConfigCreated {
                namespace: "prod".to_string(),
                key: "key1".to_string(),
                environment: "production".to_string(),
            },
            "admin",
        )
        .unwrap();

    logger
        .log_event(
            AuditEventType::ConfigUpdated {
                namespace: "prod".to_string(),
                key: "key1".to_string(),
                environment: "production".to_string(),
                old_version: 1,
                new_version: 2,
            },
            "admin",
        )
        .unwrap();

    logger
        .log_event(
            AuditEventType::SecretAccessed {
                namespace: "prod".to_string(),
                key: "secret1".to_string(),
                environment: "production".to_string(),
            },
            "bob",
        )
        .unwrap();

    // Wait for async processing
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query events
    let start = chrono::Utc::now() - chrono::Duration::hours(1);
    let end = chrono::Utc::now() + chrono::Duration::hours(1);
    let events = logger.query(start, end, None).unwrap();

    assert_eq!(events.len(), 3);

    // Query by user
    let admin_events = logger.query_by_user("admin", None).unwrap();
    assert_eq!(admin_events.len(), 2);

    let bob_events = logger.query_by_user("bob", None).unwrap();
    assert_eq!(bob_events.len(), 1);
}

#[tokio::test]
async fn test_cache_integration() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("cache");
    let storage_dir = temp_dir.path().join("storage");

    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();
    let manager = ConfigManager::new(&storage_dir)
        .unwrap()
        .with_encryption_key(key);

    let cache = CacheManager::new(100, &cache_dir).unwrap();

    // Create config
    let entry = manager
        .set(
            "app",
            "config1",
            ConfigValue::String("value1".to_string()),
            Environment::Development,
            "user",
        )
        .unwrap();

    // Put in cache
    cache.put(entry.clone()).unwrap();

    // Get from cache (L1 hit)
    let cached = cache.get("app", "config1", "development").unwrap();
    assert_eq!(cached.id, entry.id);

    let stats = cache.l1_stats();
    assert_eq!(stats.hit_count, 1);
    assert_eq!(stats.miss_count, 0);

    // Clear L1 cache
    cache.clear_l1();

    // Get again (L2 hit, promoted to L1)
    let cached2 = cache.get("app", "config1", "development").unwrap();
    assert_eq!(cached2.id, entry.id);

    // L1 should now have it
    let stats2 = cache.l1_stats();
    assert_eq!(stats2.size, 1);
}

#[test]
fn test_template_integration() {
    let mut engine = TemplateEngine::new();

    // Register database config template
    let db_template = Template::new(
        "database",
        "postgresql://{{user}}:{{password}}@{{host}}:{{port}}/{{database}}",
    )
    .unwrap()
    .with_description("PostgreSQL connection string template")
    .with_default("port", "5432");

    engine.register(db_template);

    // Render for development
    let mut dev_vars = HashMap::new();
    dev_vars.insert("user".to_string(), "devuser".to_string());
    dev_vars.insert("password".to_string(), "devpass".to_string());
    dev_vars.insert("host".to_string(), "localhost".to_string());
    dev_vars.insert("database".to_string(), "dev_db".to_string());

    let dev_config = engine.render_template("database", &dev_vars).unwrap();
    assert_eq!(
        dev_config,
        "postgresql://devuser:devpass@localhost:5432/dev_db"
    );

    // Render for production
    let mut prod_vars = HashMap::new();
    prod_vars.insert("user".to_string(), "produser".to_string());
    prod_vars.insert("password".to_string(), "prodpass".to_string());
    prod_vars.insert("host".to_string(), "db.prod.example.com".to_string());
    prod_vars.insert("port".to_string(), "5433".to_string());
    prod_vars.insert("database".to_string(), "prod_db".to_string());

    let prod_config = engine.render_template("database", &prod_vars).unwrap();
    assert_eq!(
        prod_config,
        "postgresql://produser:prodpass@db.prod.example.com:5433/prod_db"
    );
}

#[test]
fn test_environment_override_integration() {
    let temp_dir = TempDir::new().unwrap();
    let manager = ConfigManager::new(temp_dir.path()).unwrap();

    // Set base config
    manager
        .set(
            "app",
            "timeout",
            ConfigValue::Integer(30),
            Environment::Base,
            "admin",
        )
        .unwrap();

    // Override for production
    manager
        .set(
            "app",
            "timeout",
            ConfigValue::Integer(60),
            Environment::Production,
            "admin",
        )
        .unwrap();

    // Get with overrides for development (should use base)
    let dev_value = manager
        .get_with_overrides("app", "timeout", Environment::Development)
        .unwrap()
        .unwrap();

    if let ConfigValue::Integer(n) = dev_value {
        assert_eq!(n, 30);
    } else {
        panic!("Expected integer value");
    }

    // Get with overrides for production (should use override)
    let prod_value = manager
        .get_with_overrides("app", "timeout", Environment::Production)
        .unwrap()
        .unwrap();

    if let ConfigValue::Integer(n) = prod_value {
        assert_eq!(n, 60);
    } else {
        panic!("Expected integer value");
    }
}

#[tokio::test]
async fn test_multi_component_integration() {
    // This test combines multiple components in a realistic scenario
    let temp_dir = TempDir::new().unwrap();
    let key = SecretKey::generate(Algorithm::Aes256Gcm).unwrap();

    // Setup config manager
    let config_manager = ConfigManager::new(temp_dir.path().join("configs"))
        .unwrap()
        .with_encryption_key(key);

    // Setup RBAC
    let mut rbac = PolicyEnforcer::new();
    rbac.assign_role(RoleAssignment::new("admin", Role::Admin));
    rbac.assign_role(RoleAssignment::new("developer", Role::Editor));

    // Setup audit logging
    let audit_storage = Arc::new(FileAuditStorage::new(temp_dir.path().join("audit")).unwrap());
    let audit_logger = AuditLogger::new(audit_storage);

    // Setup cache
    let cache = CacheManager::new(100, temp_dir.path().join("cache")).unwrap();

    // Setup templates
    let mut template_engine = TemplateEngine::new();
    template_engine.register(
        Template::new("api-config", "{{protocol}}://{{host}}:{{port}}/{{path}}")
            .unwrap()
            .with_default("protocol", "https")
            .with_default("port", "443")
            .with_default("path", "api/v1"),
    );

    // Scenario: Admin creates API configuration

    // 1. Check permissions
    assert!(rbac
        .check_permission("admin", &Resource::Config, &Action::Create, None)
        .is_ok());

    // 2. Render template
    let mut vars = HashMap::new();
    vars.insert("host".to_string(), "api.example.com".to_string());
    let api_url = template_engine
        .render_template("api-config", &vars)
        .unwrap();

    // 3. Store configuration
    let entry = config_manager
        .set(
            "production/api",
            "endpoint",
            ConfigValue::String(api_url.clone()),
            Environment::Production,
            "admin",
        )
        .unwrap();

    // 4. Cache the entry
    cache.put(entry.clone()).unwrap();

    // 5. Log the action
    audit_logger
        .log_event(
            AuditEventType::ConfigCreated {
                namespace: "production/api".to_string(),
                key: "endpoint".to_string(),
                environment: "production".to_string(),
            },
            "admin",
        )
        .unwrap();

    // Scenario: Developer reads configuration

    // 1. Check permissions
    assert!(rbac
        .check_permission("developer", &Resource::Config, &Action::Read, None)
        .is_ok());

    // 2. Get from cache (fast path)
    let cached_entry = cache
        .get("production/api", "endpoint", "production")
        .unwrap();
    assert_eq!(cached_entry.id, entry.id);

    // 3. Log the access
    audit_logger
        .log_event(
            AuditEventType::ConfigAccessed {
                namespace: "production/api".to_string(),
                key: "endpoint".to_string(),
                environment: "production".to_string(),
            },
            "developer",
        )
        .unwrap();

    // Verify final state
    assert_eq!(api_url, "https://api.example.com:443/api/v1");
    assert_eq!(cache.l1_stats().hit_count, 1);

    // Wait for audit logs
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    assert_eq!(audit_logger.count().unwrap(), 2);
}
