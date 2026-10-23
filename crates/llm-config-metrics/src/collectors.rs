//! Metric collectors for system and process information

use prometheus::{Gauge, Opts};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Process metrics collector
pub struct ProcessCollector {
    start_time: Instant,
    cpu_usage: Gauge,
    memory_rss: Gauge,
    open_fds: Gauge,
}

impl ProcessCollector {
    /// Create a new process collector
    pub fn new(registry: Arc<prometheus::Registry>) -> crate::Result<Self> {
        let cpu_usage = Gauge::with_opts(Opts::new(
            "process_cpu_usage_percent",
            "Process CPU usage percentage",
        ))?;

        let memory_rss = Gauge::with_opts(Opts::new(
            "process_memory_rss_bytes",
            "Process resident set size in bytes",
        ))?;

        let open_fds = Gauge::with_opts(Opts::new(
            "process_open_fds",
            "Number of open file descriptors",
        ))?;

        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(memory_rss.clone()))?;
        registry.register(Box::new(open_fds.clone()))?;

        Ok(Self {
            start_time: Instant::now(),
            cpu_usage,
            memory_rss,
            open_fds,
        })
    }

    /// Update process metrics
    pub fn update(&self) {
        // In a real implementation, these would collect actual process stats
        // For now, we'll use placeholder values
        self.cpu_usage.set(0.0);
        self.memory_rss.set(0.0);
        self.open_fds.set(0.0);
    }

    /// Get process uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Custom collector for application-specific metrics
pub struct CustomCollector {
    /// Application version
    pub version: String,
    /// Build timestamp
    pub build_time: String,
}

impl CustomCollector {
    /// Create a new custom collector
    pub fn new(version: impl Into<String>, build_time: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            build_time: build_time.into(),
        }
    }

    /// Get application info
    pub fn info(&self) -> ApplicationInfo {
        ApplicationInfo {
            version: self.version.clone(),
            build_time: self.build_time.clone(),
        }
    }
}

/// Application information
#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    pub version: String,
    pub build_time: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Registry;

    #[test]
    fn test_process_collector() {
        let registry = Arc::new(Registry::new());
        let collector = ProcessCollector::new(registry).unwrap();

        collector.update();
        let first_uptime = collector.uptime();
        let second_uptime = collector.uptime();
        assert!(second_uptime >= first_uptime);
    }

    #[test]
    fn test_custom_collector() {
        let collector = CustomCollector::new("1.0.0", "2024-01-01");
        let info = collector.info();

        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.build_time, "2024-01-01");
    }
}
