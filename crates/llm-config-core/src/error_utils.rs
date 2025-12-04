//! Error handling utilities for production resilience
//!
//! This module provides utilities for handling errors in production:
//! - Retry logic with exponential backoff
//! - Circuit breaker pattern
//! - Error categorization
//! - Graceful degradation helpers

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, warn, error};

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial backoff duration in milliseconds
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds
    pub max_backoff_ms: u64,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 10_000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with custom values
    pub fn new(
        max_attempts: u32,
        initial_backoff_ms: u64,
        max_backoff_ms: u64,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_backoff_ms,
            max_backoff_ms,
            backoff_multiplier,
        }
    }

    /// Create a policy with aggressive retries (short backoff, many attempts)
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff_ms: 50,
            max_backoff_ms: 1_000,
            backoff_multiplier: 1.5,
        }
    }

    /// Create a policy with conservative retries (long backoff, few attempts)
    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            initial_backoff_ms: 500,
            max_backoff_ms: 30_000,
            backoff_multiplier: 3.0,
        }
    }

    /// Calculate backoff duration for a given attempt
    fn backoff_duration(&self, attempt: u32) -> Duration {
        let backoff_ms = (self.initial_backoff_ms as f64
            * self.backoff_multiplier.powi(attempt as i32))
        .min(self.max_backoff_ms as f64) as u64;
        Duration::from_millis(backoff_ms)
    }
}

/// Retry an async operation with exponential backoff
///
/// # Examples
///
/// ```no_run
/// use llm_config_core::error_utils::{retry_with_backoff, RetryPolicy};
///
/// async fn flaky_operation() -> Result<String, std::io::Error> {
///     // ... operation that might fail ...
///     Ok("success".to_string())
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let result = retry_with_backoff(
///         || flaky_operation(),
///         RetryPolicy::default(),
///         |e| e.kind() == std::io::ErrorKind::ConnectionRefused
///     ).await;
/// }
/// ```
pub async fn retry_with_backoff<F, Fut, T, E, R>(
    mut operation: F,
    policy: RetryPolicy,
    is_retriable: R,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
    R: Fn(&E) -> bool,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!("Operation succeeded after {} retries", attempt);
                }
                return Ok(result);
            }
            Err(e) if attempt < policy.max_attempts - 1 && is_retriable(&e) => {
                let backoff = policy.backoff_duration(attempt);
                warn!(
                    "Attempt {} failed: {}. Retrying in {:?}",
                    attempt + 1,
                    e,
                    backoff
                );
                sleep(backoff).await;
                attempt += 1;
            }
            Err(e) => {
                if attempt > 0 {
                    error!("Operation failed after {} retries: {}", attempt, e);
                }
                return Err(e);
            }
        }
    }
}

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed { failures: u32 },
    /// Circuit is open, requests are rejected
    Open { opened_at: Instant },
    /// Circuit is half-open, testing if service recovered
    HalfOpen { successes: u32 },
}

/// Circuit breaker implementation to prevent cascading failures
///
/// The circuit breaker has three states:
/// - Closed: Normal operation, failures are counted
/// - Open: Fails fast without calling the operation
/// - Half-Open: Allows limited requests to test recovery
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Number of failures before opening circuit
    failure_threshold: u32,
    /// Number of successes to close circuit from half-open
    success_threshold: u32,
    /// How long to wait before transitioning from open to half-open
    timeout: Duration,
    /// Current state
    state: Arc<Mutex<CircuitState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(failure_threshold: u32, success_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            timeout,
            state: Arc::new(Mutex::new(CircuitState::Closed { failures: 0 })),
        }
    }

    /// Create a circuit breaker with default settings
    pub fn default_config() -> Self {
        Self::new(5, 2, Duration::from_secs(60))
    }

    /// Execute an operation through the circuit breaker
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        // Check current state
        let should_execute = {
            let mut state = self.state.lock().unwrap();

            match *state {
                CircuitState::Closed { .. } => true,
                CircuitState::Open { opened_at } => {
                    if opened_at.elapsed() >= self.timeout {
                        debug!("Circuit transitioning to half-open");
                        *state = CircuitState::HalfOpen { successes: 0 };
                        true
                    } else {
                        false
                    }
                }
                CircuitState::HalfOpen { .. } => true,
            }
        };

        if !should_execute {
            return Err(CircuitBreakerError::Open);
        }

        // Execute operation
        match operation().await {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(e) => {
                self.on_failure();
                Err(CircuitBreakerError::Inner(e))
            }
        }
    }

    /// Record a successful operation
    fn on_success(&self) {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed { .. } => {
                // Reset failures
                *state = CircuitState::Closed { failures: 0 };
            }
            CircuitState::HalfOpen { successes } => {
                let new_successes = successes + 1;
                if new_successes >= self.success_threshold {
                    debug!("Circuit closing after {} successes", new_successes);
                    *state = CircuitState::Closed { failures: 0 };
                } else {
                    *state = CircuitState::HalfOpen {
                        successes: new_successes,
                    };
                }
            }
            CircuitState::Open { .. } => {
                // Shouldn't happen, but handle gracefully
                warn!("Success recorded while circuit is open");
            }
        }
    }

    /// Record a failed operation
    fn on_failure(&self) {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed { failures } => {
                let new_failures = failures + 1;
                if new_failures >= self.failure_threshold {
                    warn!(
                        "Circuit opening after {} failures",
                        new_failures
                    );
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                } else {
                    *state = CircuitState::Closed {
                        failures: new_failures,
                    };
                }
            }
            CircuitState::HalfOpen { .. } => {
                warn!("Circuit reopening after failure in half-open state");
                *state = CircuitState::Open {
                    opened_at: Instant::now(),
                };
            }
            CircuitState::Open { .. } => {
                // Already open, do nothing
            }
        }
    }

    /// Check if circuit is open
    pub fn is_open(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            CircuitState::Open { .. }
        )
    }

    /// Get current state (for monitoring)
    pub fn current_state(&self) -> String {
        match *self.state.lock().unwrap() {
            CircuitState::Closed { failures } => format!("Closed (failures: {})", failures),
            CircuitState::Open { opened_at } => {
                format!("Open (elapsed: {:?})", opened_at.elapsed())
            }
            CircuitState::HalfOpen { successes } => {
                format!("HalfOpen (successes: {})", successes)
            }
        }
    }
}

/// Circuit breaker error
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open, operation not executed
    Open,
    /// Inner operation error
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Circuit breaker is open"),
            Self::Inner(e) => write!(f, "{}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open => None,
            Self::Inner(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_backoff() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.backoff_duration(0), Duration::from_millis(100));
        assert_eq!(policy.backoff_duration(1), Duration::from_millis(200));
        assert_eq!(policy.backoff_duration(2), Duration::from_millis(400));
        assert_eq!(policy.backoff_duration(10), Duration::from_millis(10_000)); // capped
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = AtomicU32::new(0);

        let result = retry_with_backoff(
            || {
                let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if count < 3 {
                        Err("temporary error")
                    } else {
                        Ok("success")
                    }
                }
            },
            RetryPolicy {
                max_attempts: 5,
                initial_backoff_ms: 1,
                max_backoff_ms: 10,
                backoff_multiplier: 2.0,
            },
            |_| true, // All errors are retriable
        )
        .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = AtomicU32::new(0);

        let result = retry_with_backoff(
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err::<(), _>("persistent error") }
            },
            RetryPolicy {
                max_attempts: 3,
                initial_backoff_ms: 1,
                max_backoff_ms: 10,
                backoff_multiplier: 2.0,
            },
            |_| true,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let breaker = CircuitBreaker::new(3, 2, Duration::from_millis(100));

        // Fail 3 times to open circuit
        for _ in 0..3 {
            let result = breaker
                .call(|| async { Err::<(), _>("error") })
                .await;
            assert!(matches!(result, Err(CircuitBreakerError::Inner(_))));
        }

        // Circuit should be open
        assert!(breaker.is_open());

        // Next call should fail fast
        let result = breaker
            .call(|| async { Ok::<(), &str>(()) })
            .await;
        assert!(matches!(result, Err(CircuitBreakerError::Open)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open() {
        let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(10));

        // Open the circuit
        for _ in 0..2 {
            let _ = breaker.call(|| async { Err::<(), _>("error") }).await;
        }

        assert!(breaker.is_open());

        // Wait for timeout
        sleep(Duration::from_millis(20)).await;

        // Should transition to half-open and allow request
        let result = breaker
            .call(|| async { Ok::<(), &str>(()) })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes() {
        let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(10));

        // Open the circuit
        for _ in 0..2 {
            let _ = breaker.call(|| async { Err::<(), _>("error") }).await;
        }

        // Wait and succeed twice
        sleep(Duration::from_millis(20)).await;

        for _ in 0..2 {
            breaker
                .call(|| async { Ok::<(), &str>(()) })
                .await
                .unwrap();
        }

        // Circuit should be closed
        assert!(!breaker.is_open());
    }
}
