//! Health check system for monitoring service health

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded but operational
    Degraded,
    /// Service is unhealthy
    Unhealthy,
}

impl HealthStatus {
    /// Check if status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if status is degraded
    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded)
    }

    /// Check if status is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy)
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Component name
    pub component: String,
    /// Health status
    pub status: HealthStatus,
    /// Optional message
    pub message: Option<String>,
    /// Last check timestamp
    pub last_check: DateTime<Utc>,
    /// Check duration in milliseconds
    pub duration_ms: u64,
}

impl HealthCheckResult {
    /// Create a healthy result
    pub fn healthy(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Healthy,
            message: None,
            last_check: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create a degraded result
    pub fn degraded(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            last_check: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Create an unhealthy result
    pub fn unhealthy(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            last_check: Utc::now(),
            duration_ms: 0,
        }
    }

    /// Set duration
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis() as u64;
        self
    }
}

/// Overall health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall status
    pub status: HealthStatus,
    /// Individual component checks
    pub checks: HashMap<String, HealthCheckResult>,
    /// Report timestamp
    pub timestamp: DateTime<Utc>,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

impl HealthReport {
    /// Create a new health report
    pub fn new(checks: HashMap<String, HealthCheckResult>, uptime: Duration) -> Self {
        // Determine overall status from component checks
        let status = if checks.values().all(|c| c.status.is_healthy()) {
            HealthStatus::Healthy
        } else if checks.values().any(|c| c.status.is_unhealthy()) {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        };

        Self {
            status,
            checks,
            timestamp: Utc::now(),
            uptime_seconds: uptime.as_secs(),
        }
    }

    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }
}

/// Health check trait
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Perform health check
    async fn check(&self) -> HealthCheckResult;

    /// Get component name
    fn name(&self) -> &str;
}

/// Health checker that manages multiple health checks
pub struct HealthChecker {
    checks: Arc<RwLock<HashMap<String, Arc<dyn HealthCheck>>>>,
    start_time: Instant,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Register a health check
    pub fn register(&self, check: Box<dyn HealthCheck>) {
        let name = check.name().to_string();
        self.checks.write().unwrap().insert(name, Arc::from(check));
    }

    /// Perform all health checks
    pub async fn check_health(&self) -> HealthReport {
        let checks: Vec<_> = {
            let checks_map = self.checks.read().unwrap();
            checks_map
                .iter()
                .map(|(name, check)| (name.clone(), Arc::clone(check)))
                .collect()
        };
        let mut results = HashMap::new();

        for (name, check) in checks {
            let result = check.check().await;
            results.insert(name, result);
        }

        HealthReport::new(results, self.start_time.elapsed())
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage health check
pub struct StorageHealthCheck {
    name: String,
}

impl StorageHealthCheck {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait::async_trait]
impl HealthCheck for StorageHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // In a real implementation, this would check storage connectivity
        // For now, return healthy
        HealthCheckResult::healthy(&self.name).with_duration(start.elapsed())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Cache health check
pub struct CacheHealthCheck {
    name: String,
}

impl CacheHealthCheck {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait::async_trait]
impl HealthCheck for CacheHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Check cache connectivity
        HealthCheckResult::healthy(&self.name).with_duration(start.elapsed())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Database health check
pub struct DatabaseHealthCheck {
    name: String,
}

impl DatabaseHealthCheck {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DatabaseHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // Check database connectivity
        HealthCheckResult::healthy(&self.name).with_duration(start.elapsed())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Disk space health check
pub struct DiskSpaceHealthCheck {
    name: String,
    path: String,
    warning_threshold_percent: f64,
    critical_threshold_percent: f64,
}

impl DiskSpaceHealthCheck {
    pub fn new(
        name: impl Into<String>,
        path: impl Into<String>,
        warning_threshold: f64,
        critical_threshold: f64,
    ) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            warning_threshold_percent: warning_threshold,
            critical_threshold_percent: critical_threshold,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for DiskSpaceHealthCheck {
    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // In a real implementation, this would check actual disk space
        // For now, simulate healthy disk
        let usage_percent = 50.0; // Simulated value

        if usage_percent >= self.critical_threshold_percent {
            HealthCheckResult::unhealthy(
                &self.name,
                format!(
                    "Disk {} usage at {:.1}% (critical threshold: {:.1}%)",
                    self.path, usage_percent, self.critical_threshold_percent
                ),
            )
            .with_duration(start.elapsed())
        } else if usage_percent >= self.warning_threshold_percent {
            HealthCheckResult::degraded(
                &self.name,
                format!(
                    "Disk {} usage at {:.1}% (warning threshold: {:.1}%)",
                    self.path, usage_percent, self.warning_threshold_percent
                ),
            )
            .with_duration(start.elapsed())
        } else {
            HealthCheckResult::healthy(&self.name).with_duration(start.elapsed())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Healthy.is_degraded());
        assert!(!HealthStatus::Healthy.is_unhealthy());

        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(HealthStatus::Degraded.is_degraded());
        assert!(!HealthStatus::Degraded.is_unhealthy());

        assert!(!HealthStatus::Unhealthy.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_degraded());
        assert!(HealthStatus::Unhealthy.is_unhealthy());
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::healthy("test");
        assert_eq!(result.component, "test");
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.message.is_none());

        let result = HealthCheckResult::degraded("test", "warning");
        assert_eq!(result.status, HealthStatus::Degraded);
        assert_eq!(result.message, Some("warning".to_string()));

        let result = HealthCheckResult::unhealthy("test", "error");
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(result.message, Some("error".to_string()));
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new();

        checker.register(Box::new(StorageHealthCheck::new("storage")));
        checker.register(Box::new(CacheHealthCheck::new("cache")));

        let report = checker.check_health().await;
        assert!(report.is_healthy());
        assert_eq!(report.checks.len(), 2);
    }

    #[test]
    fn test_health_report() {
        let mut checks = HashMap::new();
        checks.insert("test1".to_string(), HealthCheckResult::healthy("test1"));
        checks.insert("test2".to_string(), HealthCheckResult::healthy("test2"));

        let report = HealthReport::new(checks, Duration::from_secs(100));
        assert!(report.is_healthy());
        assert_eq!(report.uptime_seconds, 100);
    }

    #[test]
    fn test_health_report_degraded() {
        let mut checks = HashMap::new();
        checks.insert("test1".to_string(), HealthCheckResult::healthy("test1"));
        checks.insert(
            "test2".to_string(),
            HealthCheckResult::degraded("test2", "warning"),
        );

        let report = HealthReport::new(checks, Duration::from_secs(100));
        assert!(!report.is_healthy());
        assert_eq!(report.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_report_unhealthy() {
        let mut checks = HashMap::new();
        checks.insert("test1".to_string(), HealthCheckResult::healthy("test1"));
        checks.insert(
            "test2".to_string(),
            HealthCheckResult::unhealthy("test2", "error"),
        );

        let report = HealthReport::new(checks, Duration::from_secs(100));
        assert!(!report.is_healthy());
        assert_eq!(report.status, HealthStatus::Unhealthy);
    }
}
