//! Rate limiting and throttling

use crate::errors::{SecurityError, SecurityResult};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per second for authenticated users
    pub authenticated_rps: u32,
    /// Requests per second for unauthenticated users
    pub unauthenticated_rps: u32,
    /// Burst size
    pub burst_size: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Ban duration for abusers (seconds)
    pub ban_duration_seconds: u64,
    /// Threshold for banning (violations)
    pub ban_threshold: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            authenticated_rps: 100,
            unauthenticated_rps: 10,
            burst_size: 50,
            window_seconds: 60,
            ban_duration_seconds: 3600, // 1 hour
            ban_threshold: 10,
        }
    }
}

/// Rate limiter for API endpoints
pub struct RateLimiter {
    config: RateLimitConfig,
    authenticated_limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    unauthenticated_limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    per_ip_limiters: Arc<RwLock<HashMap<IpAddr, IpLimiter>>>,
    banned_ips: Arc<RwLock<HashMap<IpAddr, BanInfo>>>,
}

#[derive(Debug, Clone)]
struct IpLimiter {
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    violations: usize,
    last_violation: std::time::Instant,
}

/// Information about a banned IP address
#[derive(Debug, Clone)]
pub struct BanInfo {
    /// When the IP was banned
    pub banned_at: std::time::Instant,
    /// Reason for the ban
    pub reason: String,
    /// Number of violations that led to the ban
    pub violations: usize,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        let authenticated_quota = Quota::per_second(
            NonZeroU32::new(config.authenticated_rps).unwrap_or(NonZeroU32::new(100).unwrap())
        ).allow_burst(
            NonZeroU32::new(config.burst_size).unwrap_or(NonZeroU32::new(50).unwrap())
        );

        let unauthenticated_quota = Quota::per_second(
            NonZeroU32::new(config.unauthenticated_rps).unwrap_or(NonZeroU32::new(10).unwrap())
        ).allow_burst(
            NonZeroU32::new(config.burst_size / 5).unwrap_or(NonZeroU32::new(10).unwrap())
        );

        Self {
            config,
            authenticated_limiter: Arc::new(GovernorRateLimiter::direct(authenticated_quota)),
            unauthenticated_limiter: Arc::new(GovernorRateLimiter::direct(
                unauthenticated_quota,
            )),
            per_ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            banned_ips: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request is allowed
    pub fn check_request(
        &self,
        ip: IpAddr,
        authenticated: bool,
    ) -> SecurityResult<()> {
        // Check if IP is banned
        if self.is_banned(ip) {
            return Err(SecurityError::RateLimitExceeded(
                "IP address is temporarily banned".to_string(),
            ));
        }

        // Check global rate limit
        let limiter = if authenticated {
            &self.authenticated_limiter
        } else {
            &self.unauthenticated_limiter
        };

        if limiter.check().is_err() {
            self.record_violation(ip, "Global rate limit exceeded");
            return Err(SecurityError::RateLimitExceeded(
                "Too many requests. Please try again later".to_string(),
            ));
        }

        // Check per-IP rate limit
        let mut limiters = self.per_ip_limiters.write().unwrap();
        let ip_limiter = limiters.entry(ip).or_insert_with(|| {
            let quota = if authenticated {
                Quota::per_second(
                    NonZeroU32::new(self.config.authenticated_rps / 10)
                        .unwrap_or(NonZeroU32::new(10).unwrap())
                )
            } else {
                Quota::per_second(
                    NonZeroU32::new(self.config.unauthenticated_rps)
                        .unwrap_or(NonZeroU32::new(10).unwrap())
                )
            }
            .allow_burst(NonZeroU32::new(10).unwrap());

            IpLimiter {
                limiter: Arc::new(GovernorRateLimiter::direct(quota)),
                violations: 0,
                last_violation: std::time::Instant::now(),
            }
        });

        if ip_limiter.limiter.check().is_err() {
            drop(limiters); // Release lock before recording violation
            self.record_violation(ip, "Per-IP rate limit exceeded");
            return Err(SecurityError::RateLimitExceeded(format!(
                "Too many requests from IP {}. Please try again later",
                ip
            )));
        }

        Ok(())
    }

    /// Check if an IP is banned
    fn is_banned(&self, ip: IpAddr) -> bool {
        let banned = self.banned_ips.read().unwrap();
        if let Some(ban_info) = banned.get(&ip) {
            let elapsed = ban_info.banned_at.elapsed();
            let ban_duration = Duration::from_secs(self.config.ban_duration_seconds);

            if elapsed < ban_duration {
                return true;
            }
        }
        false
    }

    /// Record a violation
    fn record_violation(&self, ip: IpAddr, reason: &str) {
        let mut limiters = self.per_ip_limiters.write().unwrap();
        if let Some(ip_limiter) = limiters.get_mut(&ip) {
            ip_limiter.violations += 1;
            ip_limiter.last_violation = std::time::Instant::now();

            // Ban if threshold exceeded
            if ip_limiter.violations >= self.config.ban_threshold {
                let violations = ip_limiter.violations; // Copy before dropping lock
                drop(limiters); // Release lock
                self.ban_ip(ip, reason.to_string(), violations);
            }
        }
    }

    /// Ban an IP address
    fn ban_ip(&self, ip: IpAddr, reason: String, violations: usize) {
        let mut banned = self.banned_ips.write().unwrap();
        banned.insert(
            ip,
            BanInfo {
                banned_at: std::time::Instant::now(),
                reason,
                violations,
            },
        );

        tracing::warn!(
            ip = %ip,
            violations = violations,
            "IP address banned due to rate limit violations"
        );
    }

    /// Manually ban an IP
    pub fn ban(&self, ip: IpAddr, reason: String) {
        self.ban_ip(ip, reason, 0);
    }

    /// Unban an IP
    pub fn unban(&self, ip: IpAddr) {
        let mut banned = self.banned_ips.write().unwrap();
        if banned.remove(&ip).is_some() {
            tracing::info!(ip = %ip, "IP address unbanned");
        }
    }

    /// Get banned IPs
    pub fn get_banned_ips(&self) -> Vec<(IpAddr, BanInfo)> {
        let banned = self.banned_ips.read().unwrap();
        banned
            .iter()
            .map(|(ip, info)| (*ip, info.clone()))
            .collect()
    }

    /// Clean up expired bans and old limiters
    pub fn cleanup(&self) {
        // Remove expired bans
        let mut banned = self.banned_ips.write().unwrap();
        let ban_duration = Duration::from_secs(self.config.ban_duration_seconds);
        banned.retain(|_, ban_info| ban_info.banned_at.elapsed() < ban_duration);

        // Remove old limiters (not accessed in last hour)
        let mut limiters = self.per_ip_limiters.write().unwrap();
        limiters.retain(|_, ip_limiter| {
            ip_limiter.last_violation.elapsed() < Duration::from_secs(3600)
        });
    }

    /// Get current statistics
    pub fn get_stats(&self) -> RateLimitStats {
        let banned = self.banned_ips.read().unwrap();
        let limiters = self.per_ip_limiters.read().unwrap();

        RateLimitStats {
            active_limiters: limiters.len(),
            banned_ips: banned.len(),
            total_violations: limiters.values().map(|l| l.violations).sum(),
        }
    }
}

/// Rate limit statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub active_limiters: usize,
    pub banned_ips: usize,
    pub total_violations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_basic() {
        let config = RateLimitConfig {
            authenticated_rps: 10,
            unauthenticated_rps: 5,
            burst_size: 10,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First request should succeed
        assert!(limiter.check_request(ip, true).is_ok());

        // Should allow burst
        for _ in 0..9 {
            assert!(limiter.check_request(ip, true).is_ok());
        }

        // Next request should fail
        assert!(limiter.check_request(ip, true).is_err());
    }

    #[test]
    fn test_per_ip_limiting() {
        let config = RateLimitConfig {
            authenticated_rps: 100,
            unauthenticated_rps: 10,
            burst_size: 20,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);
        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));

        // IP1 should be limited independently of IP2
        for _ in 0..10 {
            limiter.check_request(ip1, false).ok();
        }

        // IP2 should still be allowed
        assert!(limiter.check_request(ip2, false).is_ok());
    }

    #[test]
    fn test_banning() {
        let config = RateLimitConfig {
            authenticated_rps: 5,
            unauthenticated_rps: 5,
            burst_size: 10,
            ban_threshold: 3,
            ban_duration_seconds: 1,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Exceed rate limit multiple times to get banned
        for _ in 0..20 {
            limiter.check_request(ip, false).ok();
        }

        // Should be banned now
        assert!(limiter.is_banned(ip));

        // Wait for ban to expire
        thread::sleep(Duration::from_secs(2));

        // Clean up expired bans
        limiter.cleanup();

        // Should no longer be banned
        assert!(!limiter.is_banned(ip));
    }

    #[test]
    fn test_manual_ban() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Manually ban IP
        limiter.ban(ip, "Test ban".to_string());
        assert!(limiter.is_banned(ip));

        // Unban IP
        limiter.unban(ip);
        assert!(!limiter.is_banned(ip));
    }

    #[test]
    fn test_stats() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        limiter.check_request(ip, false).ok();

        let stats = limiter.get_stats();
        assert_eq!(stats.active_limiters, 1);
    }

    #[test]
    fn test_cleanup() {
        let limiter = RateLimiter::new(RateLimitConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        limiter.check_request(ip, false).ok();
        assert_eq!(limiter.get_stats().active_limiters, 1);

        limiter.cleanup();
        // Should still have the limiter as it was just used
        assert_eq!(limiter.get_stats().active_limiters, 1);
    }
}
