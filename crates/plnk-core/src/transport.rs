use std::sync::Arc;
use std::time::SystemTime;

use reqwest::StatusCode;
use reqwest::header::{HeaderMap, RETRY_AFTER};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::{Duration, Instant, sleep};
use tracing::{debug, trace};

use crate::error::PlankaError;

const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Shared HTTP transport policy for the entire SDK/CLI stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportPolicy {
    /// Maximum number of in-flight HTTP requests allowed per client instance.
    pub max_in_flight: usize,
    /// Optional sustained request rate limit in requests per second.
    pub rate_limit_per_second: Option<u32>,
    /// Optional burst size used with the rate limiter.
    pub burst_size: Option<u32>,
    /// Number of retry attempts after the initial request.
    pub retry_attempts: u32,
    /// Base retry delay in milliseconds.
    pub retry_base_delay_ms: u64,
    /// Maximum retry delay in milliseconds.
    pub retry_max_delay_ms: u64,
    /// Whether retry delays should include jitter.
    pub retry_jitter: bool,
    /// Restrict automatic retries to safe/idempotent methods.
    pub retry_safe_methods_only: bool,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            max_in_flight: 8,
            rate_limit_per_second: Some(10),
            burst_size: Some(10),
            retry_attempts: 2,
            retry_base_delay_ms: 250,
            retry_max_delay_ms: 2_000,
            retry_jitter: true,
            retry_safe_methods_only: true,
        }
    }
}

impl TransportPolicy {
    /// Validate internal policy consistency before building runtime state.
    ///
    /// # Errors
    /// Returns `PlankaError::InvalidOptionValue` when a field is out of range
    /// or conflicts with another transport policy field.
    pub fn validate(&self) -> Result<(), PlankaError> {
        if self.max_in_flight == 0 {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.max_in_flight".to_string(),
                message: "must be at least 1".to_string(),
            });
        }

        if let Some(rate) = self.rate_limit_per_second
            && rate == 0
        {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.rate_limit_per_second".to_string(),
                message: "must be at least 1 when set".to_string(),
            });
        }

        if let Some(burst) = self.burst_size
            && burst == 0
        {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.burst_size".to_string(),
                message: "must be at least 1 when set".to_string(),
            });
        }

        if self.burst_size.is_some() && self.rate_limit_per_second.is_none() {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.burst_size".to_string(),
                message: "requires rate_limit_per_second to also be set".to_string(),
            });
        }

        if self.retry_base_delay_ms == 0 {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.retry_base_delay_ms".to_string(),
                message: "must be at least 1".to_string(),
            });
        }

        if self.retry_max_delay_ms < self.retry_base_delay_ms {
            return Err(PlankaError::InvalidOptionValue {
                field: "transport.retry_max_delay_ms".to_string(),
                message: "must be greater than or equal to retry_base_delay_ms".to_string(),
            });
        }

        Ok(())
    }

    fn retries_allowed_for_method(&self, method: &str) -> bool {
        if self.retry_attempts == 0 {
            return false;
        }

        if !self.retry_safe_methods_only {
            return true;
        }

        matches!(
            method.to_ascii_uppercase().as_str(),
            "GET" | "HEAD" | "OPTIONS"
        )
    }

    fn retry_delay(&self, retry_number: u32) -> Duration {
        let exponential_ms = self
            .retry_base_delay_ms
            .saturating_mul(2_u64.saturating_pow(retry_number.saturating_sub(1)));
        let capped_ms = exponential_ms.min(self.retry_max_delay_ms);

        if self.retry_jitter {
            let lower_bound = capped_ms / 2;
            let spread = capped_ms.saturating_sub(lower_bound);
            let nanos = u64::from(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos(),
            );
            Duration::from_millis(lower_bound + (nanos % (spread.saturating_add(1))))
        } else {
            Duration::from_millis(capped_ms)
        }
    }

    fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
        let raw = headers.get(RETRY_AFTER)?.to_str().ok()?;

        let parsed = raw
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
            .or_else(|| {
                httpdate::parse_http_date(raw)
                    .ok()
                    .and_then(|deadline| deadline.duration_since(SystemTime::now()).ok())
            })?;

        Some(parsed.min(MAX_RETRY_AFTER))
    }

    fn should_retry_status(&self, method: &str, status: StatusCode) -> bool {
        self.retries_allowed_for_method(method)
            && matches!(
                status,
                StatusCode::TOO_MANY_REQUESTS
                    | StatusCode::BAD_GATEWAY
                    | StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::GATEWAY_TIMEOUT
            )
    }

    fn should_retry_error(&self, method: &str, error: &reqwest::Error) -> bool {
        self.retries_allowed_for_method(method) && (error.is_timeout() || error.is_connect())
    }
}

#[derive(Debug)]
struct RateLimiter {
    rate_per_second: f64,
    burst_size: f64,
    state: Mutex<RateLimiterState>,
}

#[derive(Debug)]
struct RateLimiterState {
    available_tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(rate_limit_per_second: u32, burst_size: u32) -> Self {
        Self {
            rate_per_second: f64::from(rate_limit_per_second),
            burst_size: f64::from(burst_size),
            state: Mutex::new(RateLimiterState {
                available_tokens: f64::from(burst_size),
                last_refill: Instant::now(),
            }),
        }
    }

    async fn acquire(&self) {
        loop {
            let maybe_wait = {
                let mut state = self.state.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(state.last_refill).as_secs_f64();
                if elapsed > 0.0 {
                    state.available_tokens = (state.available_tokens
                        + (elapsed * self.rate_per_second))
                        .min(self.burst_size);
                    state.last_refill = now;
                }

                if state.available_tokens >= 1.0 {
                    state.available_tokens -= 1.0;
                    None
                } else {
                    let seconds_until_token = (1.0 - state.available_tokens) / self.rate_per_second;
                    Some(Duration::from_secs_f64(seconds_until_token))
                }
            };

            match maybe_wait {
                None => return,
                Some(wait) => {
                    trace!(
                        delay_ms = wait.as_millis(),
                        "waiting for HTTP rate-limit token"
                    );
                    sleep(wait).await;
                }
            }
        }
    }
}

/// Runtime state shared by every request issued through a single `HttpClient`.
#[derive(Debug, Clone)]
pub struct TransportRuntime {
    policy: TransportPolicy,
    concurrency: Arc<Semaphore>,
    rate_limiter: Option<Arc<RateLimiter>>,
}

impl TransportRuntime {
    /// Build transport runtime state from a validated policy.
    ///
    /// # Errors
    /// Returns `PlankaError` if the supplied policy is invalid.
    pub fn new(policy: TransportPolicy) -> Result<Self, PlankaError> {
        policy.validate()?;

        let burst_size = policy
            .burst_size
            .unwrap_or_else(|| policy.rate_limit_per_second.unwrap_or(1));
        let rate_limiter = policy
            .rate_limit_per_second
            .map(|rate| Arc::new(RateLimiter::new(rate, burst_size)));

        Ok(Self {
            concurrency: Arc::new(Semaphore::new(policy.max_in_flight)),
            rate_limiter,
            policy,
        })
    }

    /// Access the policy used to build this runtime.
    #[must_use]
    pub fn policy(&self) -> &TransportPolicy {
        &self.policy
    }

    /// Acquire the shared transport guard for a single request.
    ///
    /// This enforces shared request concurrency and optional rate limiting for
    /// every outbound HTTP request issued by the client instance.
    ///
    /// # Errors
    /// Returns `PlankaError::ApiError` if the semaphore is unexpectedly closed.
    pub async fn acquire(&self) -> Result<TransportGuard, PlankaError> {
        if self.concurrency.available_permits() == 0 {
            trace!(
                max_in_flight = self.policy.max_in_flight,
                "waiting for HTTP concurrency permit"
            );
        }

        let permit = self
            .concurrency
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| PlankaError::ApiError {
                status: 0,
                message: format!("HTTP transport semaphore closed unexpectedly: {e}"),
            })?;

        if let Some(rate_limiter) = &self.rate_limiter {
            rate_limiter.acquire().await;
        }

        Ok(TransportGuard { _permit: permit })
    }

    #[must_use]
    pub fn retries_allowed_for_method(&self, method: &str) -> bool {
        self.policy.retries_allowed_for_method(method)
    }

    #[must_use]
    pub fn should_retry_status(&self, method: &str, status: StatusCode) -> bool {
        self.policy.should_retry_status(method, status)
    }

    #[must_use]
    pub fn should_retry_error(&self, method: &str, error: &reqwest::Error) -> bool {
        self.policy.should_retry_error(method, error)
    }

    #[must_use]
    pub fn retry_delay_for_attempt(&self, retry_number: u32) -> Duration {
        self.policy.retry_delay(retry_number)
    }

    #[must_use]
    pub fn retry_delay_from_headers(&self, headers: &HeaderMap) -> Option<Duration> {
        TransportPolicy::parse_retry_after(headers)
    }

    pub async fn sleep_before_retry(
        &self,
        method: &str,
        path: &str,
        retry_number: u32,
        delay: Duration,
        source: &str,
    ) {
        debug!(
            retry = retry_number,
            max_retries = self.policy.retry_attempts,
            delay_ms = delay.as_millis(),
            source,
            "retrying {method} {path}"
        );
        sleep(delay).await;
    }
}

/// Per-request guard returned by `TransportRuntime::acquire`.
#[derive(Debug)]
pub struct TransportGuard {
    _permit: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_matches_transport_spec() {
        let policy = TransportPolicy::default();
        assert_eq!(policy.max_in_flight, 8);
        assert_eq!(policy.rate_limit_per_second, Some(10));
        assert_eq!(policy.burst_size, Some(10));
        assert_eq!(policy.retry_attempts, 2);
        assert_eq!(policy.retry_base_delay_ms, 250);
        assert_eq!(policy.retry_max_delay_ms, 2_000);
        assert!(policy.retry_jitter);
        assert!(policy.retry_safe_methods_only);
    }

    #[test]
    fn policy_rejects_zero_max_in_flight() {
        let policy = TransportPolicy {
            max_in_flight: 0,
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("max_in_flight"));
    }

    #[test]
    fn policy_rejects_zero_rate_limit() {
        let policy = TransportPolicy {
            rate_limit_per_second: Some(0),
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("rate_limit_per_second"));
    }

    #[test]
    fn policy_rejects_zero_burst_size() {
        let policy = TransportPolicy {
            burst_size: Some(0),
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("burst_size"));
    }

    #[test]
    fn policy_rejects_burst_without_rate_limit() {
        let policy = TransportPolicy {
            rate_limit_per_second: None,
            burst_size: Some(10),
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("burst_size"));
        assert!(err.to_string().contains("rate_limit_per_second"));
    }

    #[test]
    fn policy_rejects_zero_retry_base_delay() {
        let policy = TransportPolicy {
            retry_base_delay_ms: 0,
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("retry_base_delay_ms"));
    }

    #[test]
    fn policy_rejects_retry_delay_inversion() {
        let policy = TransportPolicy {
            retry_base_delay_ms: 1_000,
            retry_max_delay_ms: 500,
            ..TransportPolicy::default()
        };

        let err = policy.validate().unwrap_err();
        assert_eq!(err.error_type(), "InvalidOptionValue");
        assert!(err.to_string().contains("retry_max_delay_ms"));
    }

    #[test]
    fn runtime_stores_validated_policy_once() {
        let policy = TransportPolicy::default();
        let runtime = TransportRuntime::new(policy.clone()).unwrap();
        assert_eq!(runtime.policy(), &policy);
    }

    #[test]
    fn safe_method_retry_classifier_matches_spec() {
        let runtime = TransportRuntime::new(TransportPolicy::default()).unwrap();
        assert!(runtime.should_retry_status("GET", StatusCode::SERVICE_UNAVAILABLE));
        assert!(runtime.should_retry_status("GET", StatusCode::TOO_MANY_REQUESTS));
        assert!(!runtime.should_retry_status("GET", StatusCode::NOT_FOUND));
        assert!(!runtime.should_retry_status("POST", StatusCode::SERVICE_UNAVAILABLE));
    }

    #[test]
    fn retry_delay_is_capped_by_max_delay() {
        let runtime = TransportRuntime::new(TransportPolicy {
            retry_base_delay_ms: 500,
            retry_max_delay_ms: 1_000,
            retry_jitter: false,
            ..TransportPolicy::default()
        })
        .unwrap();

        assert_eq!(
            runtime.retry_delay_for_attempt(1),
            Duration::from_millis(500)
        );
        assert_eq!(
            runtime.retry_delay_for_attempt(2),
            Duration::from_millis(1_000)
        );
        assert_eq!(
            runtime.retry_delay_for_attempt(3),
            Duration::from_millis(1_000)
        );
    }

    #[test]
    fn retry_delay_with_jitter_stays_within_bounds() {
        let runtime = TransportRuntime::new(TransportPolicy {
            retry_base_delay_ms: 800,
            retry_max_delay_ms: 800,
            retry_jitter: true,
            ..TransportPolicy::default()
        })
        .unwrap();

        let delay = runtime.retry_delay_for_attempt(1);
        assert!(delay >= Duration::from_millis(400));
        assert!(delay <= Duration::from_millis(800));
    }

    #[test]
    fn retry_after_seconds_header_is_parsed() {
        let runtime = TransportRuntime::new(TransportPolicy::default()).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, "3".parse().unwrap());

        assert_eq!(
            runtime.retry_delay_from_headers(&headers),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn retry_after_http_date_is_parsed() {
        let runtime = TransportRuntime::new(TransportPolicy::default()).unwrap();
        let deadline = SystemTime::now() + Duration::from_secs(1);
        let mut headers = HeaderMap::new();
        headers.insert(
            RETRY_AFTER,
            httpdate::fmt_http_date(deadline).parse().unwrap(),
        );

        let delay = runtime.retry_delay_from_headers(&headers).unwrap();
        assert!(delay <= Duration::from_secs(1));
    }

    #[test]
    fn retry_after_is_clamped_to_upper_bound() {
        let runtime = TransportRuntime::new(TransportPolicy::default()).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, "999".parse().unwrap());

        assert_eq!(
            runtime.retry_delay_from_headers(&headers),
            Some(MAX_RETRY_AFTER)
        );
    }

    #[tokio::test]
    async fn rate_limiter_waits_for_next_token_after_burst_is_exhausted() {
        let runtime = TransportRuntime::new(TransportPolicy {
            max_in_flight: 8,
            rate_limit_per_second: Some(5),
            burst_size: Some(1),
            ..TransportPolicy::default()
        })
        .unwrap();

        let _first = runtime.acquire().await.unwrap();
        let start = Instant::now();
        let _second = runtime.acquire().await.unwrap();

        assert!(
            start.elapsed() >= Duration::from_millis(180),
            "second token should have been delayed by the shared rate limiter"
        );
    }

    #[tokio::test]
    async fn concurrency_limit_blocks_until_permit_is_released() {
        let runtime = TransportRuntime::new(TransportPolicy {
            max_in_flight: 1,
            rate_limit_per_second: None,
            burst_size: None,
            ..TransportPolicy::default()
        })
        .unwrap();

        let first = runtime.acquire().await.unwrap();
        let cloned = runtime.clone();

        let waiter = tokio::spawn(async move {
            let start = Instant::now();
            let _second = cloned.acquire().await.unwrap();
            start.elapsed()
        });

        sleep(Duration::from_millis(50)).await;
        drop(first);

        let waited = waiter.await.unwrap();
        assert!(
            waited >= Duration::from_millis(45),
            "second request should wait until the first permit is dropped"
        );
    }
}
