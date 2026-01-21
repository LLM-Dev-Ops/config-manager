//! Prometheus metrics for Config Validation Agent
//!
//! Provides comprehensive metrics collection for validation operations:
//! - `validation_requests_total` (counter) - Total validation requests by result
//! - `validation_duration_seconds` (histogram) - Validation duration distribution
//! - `validation_findings_total` (counter) - Findings by severity
//! - `validation_confidence` (gauge) - Current confidence scores
//!
//! # Example
//!
//! ```rust,no_run
//! use config_validation::telemetry::ValidationMetricsRegistry;
//!
//! let registry = ValidationMetricsRegistry::new().unwrap();
//! let metrics = registry.validation();
//!
//! // Record a validation request
//! metrics.record_request("production", "app-config", true);
//!
//! // Record validation duration
//! metrics.observe_duration("production", "1.0.0", 0.045);
//!
//! // Record findings
//! metrics.record_finding("warning", "WARN001", "production");
//!
//! // Set confidence gauge
//! metrics.set_confidence("production", "app-config", 0.95);
//! ```

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, Opts, Registry,
};
use std::sync::Arc;
use std::time::Instant;

use super::{Result, TelemetryError};
use crate::contracts::IssueSeverity;

/// Validation metrics for Prometheus
pub struct ValidationMetrics {
    /// Total number of validation requests (by environment, namespace, result)
    requests_total: CounterVec,

    /// Validation duration in seconds (by environment, schema_version)
    duration_seconds: HistogramVec,

    /// Total findings by severity, code, and environment
    findings_total: CounterVec,

    /// Current confidence score (by environment, namespace)
    confidence: GaugeVec,

    /// Active validations in progress
    active_validations: Gauge,

    /// Validation errors total (by error_type, environment)
    errors_total: CounterVec,

    /// Schema version usage counter
    schema_versions: CounterVec,

    /// Rules evaluated total (by rule, result)
    rules_evaluated_total: CounterVec,

    /// Decision events emitted total
    events_emitted_total: Counter,

    /// Decision event emission failures
    events_failed_total: Counter,

    /// Event queue depth
    event_queue_depth: Gauge,
}

impl ValidationMetrics {
    /// Create a new ValidationMetrics instance and register with the provided registry
    pub fn new(registry: Arc<Registry>) -> Result<Self> {
        let requests_total = CounterVec::new(
            Opts::new(
                "validation_requests_total",
                "Total number of configuration validation requests",
            )
            .namespace("config_validation"),
            &["environment", "namespace", "result"],
        )?;

        let duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "validation_duration_seconds",
                "Configuration validation duration in seconds",
            )
            .namespace("config_validation")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["environment", "schema_version"],
        )?;

        let findings_total = CounterVec::new(
            Opts::new(
                "validation_findings_total",
                "Total number of validation findings by severity",
            )
            .namespace("config_validation"),
            &["severity", "code", "environment"],
        )?;

        let confidence = GaugeVec::new(
            Opts::new(
                "validation_confidence",
                "Current validation confidence score (0.0 - 1.0)",
            )
            .namespace("config_validation"),
            &["environment", "namespace"],
        )?;

        let active_validations = Gauge::new(
            "config_validation_active_validations",
            "Number of validations currently in progress",
        )?;

        let errors_total = CounterVec::new(
            Opts::new(
                "validation_errors_total",
                "Total number of validation errors",
            )
            .namespace("config_validation"),
            &["error_type", "environment"],
        )?;

        let schema_versions = CounterVec::new(
            Opts::new(
                "validation_schema_versions_total",
                "Schema versions used in validations",
            )
            .namespace("config_validation"),
            &["version"],
        )?;

        let rules_evaluated_total = CounterVec::new(
            Opts::new(
                "validation_rules_evaluated_total",
                "Total number of validation rules evaluated",
            )
            .namespace("config_validation"),
            &["rule", "result"],
        )?;

        let events_emitted_total = Counter::new(
            "config_validation_events_emitted_total",
            "Total number of decision events emitted to ruvector-service",
        )?;

        let events_failed_total = Counter::new(
            "config_validation_events_failed_total",
            "Total number of decision event emission failures",
        )?;

        let event_queue_depth = Gauge::new(
            "config_validation_event_queue_depth",
            "Current depth of the event emission queue",
        )?;

        // Register all metrics
        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(duration_seconds.clone()))?;
        registry.register(Box::new(findings_total.clone()))?;
        registry.register(Box::new(confidence.clone()))?;
        registry.register(Box::new(active_validations.clone()))?;
        registry.register(Box::new(errors_total.clone()))?;
        registry.register(Box::new(schema_versions.clone()))?;
        registry.register(Box::new(rules_evaluated_total.clone()))?;
        registry.register(Box::new(events_emitted_total.clone()))?;
        registry.register(Box::new(events_failed_total.clone()))?;
        registry.register(Box::new(event_queue_depth.clone()))?;

        Ok(Self {
            requests_total,
            duration_seconds,
            findings_total,
            confidence,
            active_validations,
            errors_total,
            schema_versions,
            rules_evaluated_total,
            events_emitted_total,
            events_failed_total,
            event_queue_depth,
        })
    }

    /// Record a validation request
    pub fn record_request(&self, environment: &str, namespace: &str, valid: bool) {
        let result = if valid { "valid" } else { "invalid" };
        self.requests_total
            .with_label_values(&[environment, namespace, result])
            .inc();
    }

    /// Observe validation duration
    pub fn observe_duration(&self, environment: &str, schema_version: &str, duration_secs: f64) {
        self.duration_seconds
            .with_label_values(&[environment, schema_version])
            .observe(duration_secs);
    }

    /// Record a validation finding
    pub fn record_finding(&self, severity: &str, code: &str, environment: &str) {
        self.findings_total
            .with_label_values(&[severity, code, environment])
            .inc();
    }

    /// Record a finding from IssueSeverity enum
    pub fn record_finding_severity(&self, severity: IssueSeverity, code: &str, environment: &str) {
        let severity_str = match severity {
            IssueSeverity::Error => "error",
            IssueSeverity::Warning => "warning",
            IssueSeverity::Info => "info",
        };
        self.record_finding(severity_str, code, environment);
    }

    /// Record multiple findings
    pub fn record_findings(&self, findings: &[(String, String)], environment: &str) {
        for (severity, code) in findings {
            self.record_finding(severity, code, environment);
        }
    }

    /// Set the current confidence score
    pub fn set_confidence(&self, environment: &str, namespace: &str, confidence: f64) {
        self.confidence
            .with_label_values(&[environment, namespace])
            .set(confidence);
    }

    /// Increment active validations
    pub fn inc_active(&self) {
        self.active_validations.inc();
    }

    /// Decrement active validations
    pub fn dec_active(&self) {
        self.active_validations.dec();
    }

    /// Record a validation error
    pub fn record_error(&self, error_type: &str, environment: &str) {
        self.errors_total
            .with_label_values(&[error_type, environment])
            .inc();
    }

    /// Record schema version usage
    pub fn record_schema_version(&self, version: &str) {
        self.schema_versions.with_label_values(&[version]).inc();
    }

    /// Record rule evaluation result
    pub fn record_rule_evaluation(&self, rule: &str, passed: bool) {
        let result = if passed { "passed" } else { "failed" };
        self.rules_evaluated_total
            .with_label_values(&[rule, result])
            .inc();
    }

    /// Record a successful event emission
    pub fn record_event_emitted(&self) {
        self.events_emitted_total.inc();
    }

    /// Record a failed event emission
    pub fn record_event_failed(&self) {
        self.events_failed_total.inc();
    }

    /// Set the current event queue depth
    pub fn set_queue_depth(&self, depth: usize) {
        self.event_queue_depth.set(depth as f64);
    }

    /// Start a validation timer (returns a guard that records duration on drop)
    pub fn start_timer(&self, environment: &str, schema_version: &str) -> ValidationTimer {
        self.inc_active();
        ValidationTimer {
            start: Instant::now(),
            environment: environment.to_string(),
            schema_version: schema_version.to_string(),
            metrics: self,
        }
    }
}

/// RAII guard for timing validations
pub struct ValidationTimer<'a> {
    start: Instant,
    environment: String,
    schema_version: String,
    metrics: &'a ValidationMetrics,
}

impl<'a> ValidationTimer<'a> {
    /// Get the elapsed time so far
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }

    /// Get elapsed time in seconds
    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl<'a> Drop for ValidationTimer<'a> {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.metrics
            .observe_duration(&self.environment, &self.schema_version, duration);
        self.metrics.dec_active();
    }
}

/// Registry for all validation metrics
pub struct ValidationMetricsRegistry {
    registry: Arc<Registry>,
    validation: ValidationMetrics,
}

impl ValidationMetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Result<Self> {
        let registry = Arc::new(Registry::new());
        let validation = ValidationMetrics::new(Arc::clone(&registry))?;

        Ok(Self {
            registry,
            validation,
        })
    }

    /// Create with an existing Prometheus registry
    pub fn with_registry(registry: Arc<Registry>) -> Result<Self> {
        let validation = ValidationMetrics::new(Arc::clone(&registry))?;

        Ok(Self {
            registry,
            validation,
        })
    }

    /// Get the Prometheus registry
    pub fn registry(&self) -> Arc<Registry> {
        Arc::clone(&self.registry)
    }

    /// Get validation metrics
    pub fn validation(&self) -> &ValidationMetrics {
        &self.validation
    }

    /// Gather all metrics in Prometheus format
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// Encode metrics as text for scraping
    pub fn encode_text(&self) -> Result<String> {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| TelemetryError::MetricsError(prometheus::Error::Msg(e.to_string())))?;
        String::from_utf8(buffer)
            .map_err(|e| TelemetryError::MetricsError(prometheus::Error::Msg(e.to_string())))
    }
}

impl Default for ValidationMetricsRegistry {
    fn default() -> Self {
        Self::new().expect("Failed to create validation metrics registry")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metrics() -> ValidationMetrics {
        let registry = Arc::new(Registry::new());
        ValidationMetrics::new(registry).unwrap()
    }

    #[test]
    fn test_record_request() {
        let metrics = create_test_metrics();

        metrics.record_request("production", "app-config", true);
        metrics.record_request("production", "app-config", false);
        metrics.record_request("staging", "app-config", true);
    }

    #[test]
    fn test_observe_duration() {
        let metrics = create_test_metrics();

        metrics.observe_duration("production", "1.0.0", 0.05);
        metrics.observe_duration("production", "1.0.0", 0.10);
        metrics.observe_duration("staging", "2.0.0", 0.01);
    }

    #[test]
    fn test_record_findings() {
        let metrics = create_test_metrics();

        metrics.record_finding("warning", "WARN001", "production");
        metrics.record_finding("error", "ERR001", "production");
        metrics.record_finding("info", "INFO001", "staging");

        let findings = vec![
            ("warning".to_string(), "WARN002".to_string()),
            ("error".to_string(), "ERR002".to_string()),
        ];
        metrics.record_findings(&findings, "production");
    }

    #[test]
    fn test_record_finding_severity() {
        let metrics = create_test_metrics();

        metrics.record_finding_severity(IssueSeverity::Error, "ERR001", "production");
        metrics.record_finding_severity(IssueSeverity::Warning, "WARN001", "production");
        metrics.record_finding_severity(IssueSeverity::Info, "INFO001", "staging");
    }

    #[test]
    fn test_set_confidence() {
        let metrics = create_test_metrics();

        metrics.set_confidence("production", "app-config", 0.95);
        metrics.set_confidence("staging", "app-config", 0.87);
    }

    #[test]
    fn test_active_validations() {
        let metrics = create_test_metrics();

        metrics.inc_active();
        metrics.inc_active();
        metrics.dec_active();
        // Should have 1 active validation now
    }

    #[test]
    fn test_record_error() {
        let metrics = create_test_metrics();

        metrics.record_error("schema_not_found", "production");
        metrics.record_error("timeout", "staging");
    }

    #[test]
    fn test_schema_version_tracking() {
        let metrics = create_test_metrics();

        metrics.record_schema_version("1.0.0");
        metrics.record_schema_version("1.0.0");
        metrics.record_schema_version("2.0.0");
    }

    #[test]
    fn test_rule_evaluation() {
        let metrics = create_test_metrics();

        metrics.record_rule_evaluation("required-fields", true);
        metrics.record_rule_evaluation("type-check", true);
        metrics.record_rule_evaluation("range-check", false);
    }

    #[test]
    fn test_event_emission_tracking() {
        let metrics = create_test_metrics();

        metrics.record_event_emitted();
        metrics.record_event_emitted();
        metrics.record_event_failed();
        metrics.set_queue_depth(10);
    }

    #[test]
    fn test_validation_timer() {
        let metrics = create_test_metrics();

        {
            let timer = metrics.start_timer("production", "1.0.0");
            // Simulate some work
            std::thread::sleep(std::time::Duration::from_millis(10));
            assert!(timer.elapsed_secs() > 0.0);
        } // Timer drops here, recording duration
    }

    #[test]
    fn test_metrics_registry() {
        let registry = ValidationMetricsRegistry::new().unwrap();

        registry.validation().record_request("prod", "ns", true);
        registry.validation().observe_duration("prod", "1.0", 0.01);

        let families = registry.gather();
        assert!(!families.is_empty());
    }

    #[test]
    fn test_encode_text() {
        let registry = ValidationMetricsRegistry::new().unwrap();

        registry.validation().record_request("prod", "ns", true);

        let text = registry.encode_text().unwrap();
        assert!(text.contains("validation_requests_total"));
    }
}
