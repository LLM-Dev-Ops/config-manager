//! Metrics and monitoring for LLM Config Manager
//!
//! This crate provides comprehensive metrics collection using Prometheus.
//! It includes metrics for all major subsystems and health checks.

pub mod collectors;
pub mod health;

use prometheus::{Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, Opts, Registry};
use std::sync::Arc;
use thiserror::Error;

pub use collectors::*;
pub use health::*;

#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Prometheus error: {0}")]
    PrometheusError(#[from] prometheus::Error),

    #[error("Metrics not initialized")]
    NotInitialized,

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
}

pub type Result<T> = std::result::Result<T, MetricsError>;

/// Global metrics registry
pub struct MetricsRegistry {
    registry: Arc<Registry>,
    config_metrics: ConfigMetrics,
    cache_metrics: CacheMetrics,
    rbac_metrics: RbacMetrics,
    audit_metrics: AuditMetrics,
    storage_metrics: StorageMetrics,
    crypto_metrics: CryptoMetrics,
    system_metrics: SystemMetrics,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Result<Self> {
        let registry = Arc::new(Registry::new());

        Ok(Self {
            config_metrics: ConfigMetrics::new(Arc::clone(&registry))?,
            cache_metrics: CacheMetrics::new(Arc::clone(&registry))?,
            rbac_metrics: RbacMetrics::new(Arc::clone(&registry))?,
            audit_metrics: AuditMetrics::new(Arc::clone(&registry))?,
            storage_metrics: StorageMetrics::new(Arc::clone(&registry))?,
            crypto_metrics: CryptoMetrics::new(Arc::clone(&registry))?,
            system_metrics: SystemMetrics::new(Arc::clone(&registry))?,
            registry,
        })
    }

    /// Get the Prometheus registry
    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }

    /// Get configuration metrics
    pub fn config(&self) -> &ConfigMetrics {
        &self.config_metrics
    }

    /// Get cache metrics
    pub fn cache(&self) -> &CacheMetrics {
        &self.cache_metrics
    }

    /// Get RBAC metrics
    pub fn rbac(&self) -> &RbacMetrics {
        &self.rbac_metrics
    }

    /// Get audit metrics
    pub fn audit(&self) -> &AuditMetrics {
        &self.audit_metrics
    }

    /// Get storage metrics
    pub fn storage(&self) -> &StorageMetrics {
        &self.storage_metrics
    }

    /// Get crypto metrics
    pub fn crypto(&self) -> &CryptoMetrics {
        &self.crypto_metrics
    }

    /// Get system metrics
    pub fn system(&self) -> &SystemMetrics {
        &self.system_metrics
    }

    /// Gather all metrics in Prometheus format
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics registry")
    }
}

/// Configuration operation metrics
pub struct ConfigMetrics {
    operations_total: CounterVec,
    operation_duration: HistogramVec,
    active_configs: GaugeVec,
    errors_total: CounterVec,
}

impl ConfigMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let operations_total = CounterVec::new(
            Opts::new(
                "config_operations_total",
                "Total number of configuration operations",
            ),
            &["operation", "environment"],
        )?;

        let operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "config_operation_duration_seconds",
                "Configuration operation duration in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["operation"],
        )?;

        let active_configs = GaugeVec::new(
            Opts::new("config_active_total", "Number of active configurations"),
            &["namespace", "environment"],
        )?;

        let errors_total = CounterVec::new(
            Opts::new(
                "config_errors_total",
                "Total number of configuration errors",
            ),
            &["error_type", "operation"],
        )?;

        registry.register(Box::new(operations_total.clone()))?;
        registry.register(Box::new(operation_duration.clone()))?;
        registry.register(Box::new(active_configs.clone()))?;
        registry.register(Box::new(errors_total.clone()))?;

        Ok(Self {
            operations_total,
            operation_duration,
            active_configs,
            errors_total,
        })
    }

    pub fn record_operation(&self, operation: &str, environment: &str) {
        self.operations_total
            .with_label_values(&[operation, environment])
            .inc();
    }

    pub fn observe_duration(&self, operation: &str, duration: f64) {
        self.operation_duration
            .with_label_values(&[operation])
            .observe(duration);
    }

    pub fn set_active_configs(&self, namespace: &str, environment: &str, count: i64) {
        self.active_configs
            .with_label_values(&[namespace, environment])
            .set(count as f64);
    }

    pub fn record_error(&self, error_type: &str, operation: &str) {
        self.errors_total
            .with_label_values(&[error_type, operation])
            .inc();
    }
}

/// Cache metrics
pub struct CacheMetrics {
    hits_total: CounterVec,
    misses_total: CounterVec,
    evictions_total: CounterVec,
    size: GaugeVec,
    operation_duration: HistogramVec,
}

impl CacheMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let hits_total =
            CounterVec::new(Opts::new("cache_hits_total", "Total cache hits"), &["tier"])?;

        let misses_total = CounterVec::new(
            Opts::new("cache_misses_total", "Total cache misses"),
            &["tier"],
        )?;

        let evictions_total = CounterVec::new(
            Opts::new("cache_evictions_total", "Total cache evictions"),
            &["tier"],
        )?;

        let size = GaugeVec::new(
            Opts::new("cache_size_entries", "Current cache size in entries"),
            &["tier"],
        )?;

        let operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "cache_operation_duration_seconds",
                "Cache operation duration in seconds",
            )
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["operation", "tier"],
        )?;

        registry.register(Box::new(hits_total.clone()))?;
        registry.register(Box::new(misses_total.clone()))?;
        registry.register(Box::new(evictions_total.clone()))?;
        registry.register(Box::new(size.clone()))?;
        registry.register(Box::new(operation_duration.clone()))?;

        Ok(Self {
            hits_total,
            misses_total,
            evictions_total,
            size,
            operation_duration,
        })
    }

    pub fn record_hit(&self, tier: &str) {
        self.hits_total.with_label_values(&[tier]).inc();
    }

    pub fn record_miss(&self, tier: &str) {
        self.misses_total.with_label_values(&[tier]).inc();
    }

    pub fn record_eviction(&self, tier: &str) {
        self.evictions_total.with_label_values(&[tier]).inc();
    }

    pub fn set_size(&self, tier: &str, size: usize) {
        self.size.with_label_values(&[tier]).set(size as f64);
    }

    pub fn observe_duration(&self, operation: &str, tier: &str, duration: f64) {
        self.operation_duration
            .with_label_values(&[operation, tier])
            .observe(duration);
    }

    pub fn hit_rate(&self, tier: &str) -> f64 {
        let hits = self.hits_total.with_label_values(&[tier]).get();
        let misses = self.misses_total.with_label_values(&[tier]).get();
        if hits + misses == 0.0 {
            0.0
        } else {
            hits / (hits + misses)
        }
    }
}

/// RBAC metrics
pub struct RbacMetrics {
    permission_checks_total: CounterVec,
    permission_denials_total: CounterVec,
    check_duration: HistogramVec,
    active_roles: GaugeVec,
}

impl RbacMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let permission_checks_total = CounterVec::new(
            Opts::new("rbac_permission_checks_total", "Total permission checks"),
            &["resource", "action", "result"],
        )?;

        let permission_denials_total = CounterVec::new(
            Opts::new("rbac_permission_denials_total", "Total permission denials"),
            &["resource", "action"],
        )?;

        let check_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "rbac_check_duration_seconds",
                "Permission check duration in seconds",
            )
            .buckets(vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005]),
            &["resource"],
        )?;

        let active_roles = GaugeVec::new(
            Opts::new(
                "rbac_active_roles_total",
                "Number of active role assignments",
            ),
            &["role"],
        )?;

        registry.register(Box::new(permission_checks_total.clone()))?;
        registry.register(Box::new(permission_denials_total.clone()))?;
        registry.register(Box::new(check_duration.clone()))?;
        registry.register(Box::new(active_roles.clone()))?;

        Ok(Self {
            permission_checks_total,
            permission_denials_total,
            check_duration,
            active_roles,
        })
    }

    pub fn record_permission_check(&self, resource: &str, action: &str, allowed: bool) {
        let result = if allowed { "allowed" } else { "denied" };
        self.permission_checks_total
            .with_label_values(&[resource, action, result])
            .inc();

        if !allowed {
            self.permission_denials_total
                .with_label_values(&[resource, action])
                .inc();
        }
    }

    pub fn observe_check_duration(&self, resource: &str, duration: f64) {
        self.check_duration
            .with_label_values(&[resource])
            .observe(duration);
    }

    pub fn set_active_roles(&self, role: &str, count: usize) {
        self.active_roles
            .with_label_values(&[role])
            .set(count as f64);
    }
}

/// Audit log metrics
pub struct AuditMetrics {
    events_total: CounterVec,
    events_by_user: CounterVec,
    event_processing_duration: Histogram,
    queue_size: Gauge,
}

impl AuditMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let events_total = CounterVec::new(
            Opts::new("audit_events_total", "Total audit events"),
            &["event_type"],
        )?;

        let events_by_user = CounterVec::new(
            Opts::new("audit_events_by_user_total", "Audit events by user"),
            &["user"],
        )?;

        let event_processing_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "audit_event_processing_duration_seconds",
                "Audit event processing duration",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5]),
        )?;

        let queue_size = Gauge::new("audit_queue_size", "Current audit event queue size")?;

        registry.register(Box::new(events_total.clone()))?;
        registry.register(Box::new(events_by_user.clone()))?;
        registry.register(Box::new(event_processing_duration.clone()))?;
        registry.register(Box::new(queue_size.clone()))?;

        Ok(Self {
            events_total,
            events_by_user,
            event_processing_duration,
            queue_size,
        })
    }

    pub fn record_event(&self, event_type: &str, user: &str) {
        self.events_total.with_label_values(&[event_type]).inc();
        self.events_by_user.with_label_values(&[user]).inc();
    }

    pub fn observe_processing_duration(&self, duration: f64) {
        self.event_processing_duration.observe(duration);
    }

    pub fn set_queue_size(&self, size: usize) {
        self.queue_size.set(size as f64);
    }
}

/// Storage metrics
pub struct StorageMetrics {
    operations_total: CounterVec,
    operation_duration: HistogramVec,
    storage_size_bytes: Gauge,
    errors_total: CounterVec,
}

impl StorageMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let operations_total = CounterVec::new(
            Opts::new("storage_operations_total", "Total storage operations"),
            &["operation"],
        )?;

        let operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "storage_operation_duration_seconds",
                "Storage operation duration",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
            &["operation"],
        )?;

        let storage_size_bytes = Gauge::new("storage_size_bytes", "Total storage size in bytes")?;

        let errors_total = CounterVec::new(
            Opts::new("storage_errors_total", "Total storage errors"),
            &["error_type"],
        )?;

        registry.register(Box::new(operations_total.clone()))?;
        registry.register(Box::new(operation_duration.clone()))?;
        registry.register(Box::new(storage_size_bytes.clone()))?;
        registry.register(Box::new(errors_total.clone()))?;

        Ok(Self {
            operations_total,
            operation_duration,
            storage_size_bytes,
            errors_total,
        })
    }

    pub fn record_operation(&self, operation: &str) {
        self.operations_total.with_label_values(&[operation]).inc();
    }

    pub fn observe_duration(&self, operation: &str, duration: f64) {
        self.operation_duration
            .with_label_values(&[operation])
            .observe(duration);
    }

    pub fn set_size(&self, size_bytes: u64) {
        self.storage_size_bytes.set(size_bytes as f64);
    }

    pub fn record_error(&self, error_type: &str) {
        self.errors_total.with_label_values(&[error_type]).inc();
    }
}

/// Cryptography metrics
pub struct CryptoMetrics {
    operations_total: CounterVec,
    operation_duration: HistogramVec,
    key_rotations_total: Counter,
    encryption_errors_total: Counter,
}

impl CryptoMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let operations_total = CounterVec::new(
            Opts::new("crypto_operations_total", "Total crypto operations"),
            &["operation", "algorithm"],
        )?;

        let operation_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "crypto_operation_duration_seconds",
                "Crypto operation duration",
            )
            .buckets(vec![0.00001, 0.00005, 0.0001, 0.0005, 0.001, 0.005, 0.01]),
            &["operation"],
        )?;

        let key_rotations_total =
            Counter::new("crypto_key_rotations_total", "Total key rotations")?;

        let encryption_errors_total =
            Counter::new("crypto_encryption_errors_total", "Total encryption errors")?;

        registry.register(Box::new(operations_total.clone()))?;
        registry.register(Box::new(operation_duration.clone()))?;
        registry.register(Box::new(key_rotations_total.clone()))?;
        registry.register(Box::new(encryption_errors_total.clone()))?;

        Ok(Self {
            operations_total,
            operation_duration,
            key_rotations_total,
            encryption_errors_total,
        })
    }

    pub fn record_operation(&self, operation: &str, algorithm: &str) {
        self.operations_total
            .with_label_values(&[operation, algorithm])
            .inc();
    }

    pub fn observe_duration(&self, operation: &str, duration: f64) {
        self.operation_duration
            .with_label_values(&[operation])
            .observe(duration);
    }

    pub fn record_key_rotation(&self) {
        self.key_rotations_total.inc();
    }

    pub fn record_encryption_error(&self) {
        self.encryption_errors_total.inc();
    }
}

/// System-wide metrics
pub struct SystemMetrics {
    uptime_seconds: Gauge,
    memory_usage_bytes: Gauge,
    goroutines: Gauge,
    http_requests_total: CounterVec,
    http_request_duration: HistogramVec,
}

impl SystemMetrics {
    fn new(registry: Arc<Registry>) -> Result<Self> {
        let uptime_seconds = Gauge::new("system_uptime_seconds", "System uptime in seconds")?;

        let memory_usage_bytes =
            Gauge::new("system_memory_usage_bytes", "Current memory usage in bytes")?;

        let goroutines = Gauge::new("system_goroutines", "Number of goroutines")?;

        let http_requests_total = CounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        )?;

        let http_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["method", "path"],
        )?;

        registry.register(Box::new(uptime_seconds.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(goroutines.clone()))?;
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;

        Ok(Self {
            uptime_seconds,
            memory_usage_bytes,
            goroutines,
            http_requests_total,
            http_request_duration,
        })
    }

    pub fn set_uptime(&self, seconds: f64) {
        self.uptime_seconds.set(seconds);
    }

    pub fn set_memory_usage(&self, bytes: u64) {
        self.memory_usage_bytes.set(bytes as f64);
    }

    pub fn set_goroutines(&self, count: usize) {
        self.goroutines.set(count as f64);
    }

    pub fn record_http_request(&self, method: &str, path: &str, status: u16) {
        self.http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
    }

    pub fn observe_http_duration(&self, method: &str, path: &str, duration: f64) {
        self.http_request_duration
            .with_label_values(&[method, path])
            .observe(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry_creation() {
        let registry = MetricsRegistry::new().unwrap();
        assert!(!registry.gather().is_empty());
    }

    #[test]
    fn test_config_metrics() {
        let registry = MetricsRegistry::new().unwrap();

        registry.config().record_operation("set", "production");
        registry.config().observe_duration("set", 0.005);
        registry
            .config()
            .set_active_configs("test/ns", "production", 42);
        registry.config().record_error("validation", "set");

        let metrics = registry.gather();
        assert!(!metrics.is_empty());
    }

    #[test]
    fn test_cache_metrics() {
        let registry = MetricsRegistry::new().unwrap();

        registry.cache().record_hit("l1");
        registry.cache().record_miss("l1");
        registry.cache().set_size("l1", 100);

        let hit_rate = registry.cache().hit_rate("l1");
        assert!((hit_rate - 0.5).abs() < 0.01); // 1 hit, 1 miss = 50%
    }

    #[test]
    fn test_rbac_metrics() {
        let registry = MetricsRegistry::new().unwrap();

        registry
            .rbac()
            .record_permission_check("config", "read", true);
        registry
            .rbac()
            .record_permission_check("config", "write", false);
        registry.rbac().observe_check_duration("config", 0.0001);
        registry.rbac().set_active_roles("admin", 5);

        let metrics = registry.gather();
        assert!(!metrics.is_empty());
    }
}
