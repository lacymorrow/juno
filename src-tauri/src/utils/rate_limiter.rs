/// Rate limiting utilities for Tauri commands
/// Prevents abuse and ensures system stability

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{warn, debug};

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed in the time window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Whether to allow bursts (all requests at once)
    pub allow_burst: bool,
}

impl RateLimitConfig {
    /// Create a simple rate limit (e.g., 10 requests per second)
    pub fn per_second(max_requests: u32) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(1),
            allow_burst: true,
        }
    }

    /// Create a rate limit per minute
    pub fn per_minute(max_requests: u32) -> Self {
        Self {
            max_requests,
            window: Duration::from_secs(60),
            allow_burst: true,
        }
    }

    /// Create a strict rate limit with evenly spaced requests
    pub fn strict(max_requests: u32, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            allow_burst: false,
        }
    }
}

/// Token bucket implementation for rate limiting
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let max_tokens = config.max_requests as f64;
        let refill_rate = max_tokens / config.window.as_secs_f64();
        
        Self {
            tokens: if config.allow_burst { max_tokens } else { 1.0 },
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        // Refill tokens based on time elapsed
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * self.refill_rate;
        
        self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
        self.last_refill = now;

        // Try to consume tokens
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn time_until_available(&self, tokens: f64) -> Option<Duration> {
        if self.tokens >= tokens {
            None
        } else {
            let tokens_needed = tokens - self.tokens;
            let seconds_needed = tokens_needed / self.refill_rate;
            Some(Duration::from_secs_f64(seconds_needed))
        }
    }
}

/// Rate limiter for tracking request rates per key
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Check if a request is allowed for the given key
    pub async fn check(&self, key: &str) -> Result<(), RateLimitError> {
        self.check_with_cost(key, 1.0).await
    }

    /// Check if a request with a specific cost is allowed
    pub async fn check_with_cost(&self, key: &str, cost: f64) -> Result<(), RateLimitError> {
        let mut buckets = self.buckets.lock().await;
        
        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(&self.config));

        if bucket.try_consume(cost) {
            debug!("Rate limit check passed for key: {}", key);
            Ok(())
        } else {
            let retry_after = bucket.time_until_available(cost);
            warn!(
                "Rate limit exceeded for key: {} (retry after: {:?})",
                key, retry_after
            );
            Err(RateLimitError::RateLimitExceeded { retry_after })
        }
    }

    /// Clean up old buckets that haven't been used recently
    pub async fn cleanup(&self, max_age: Duration) {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        
        buckets.retain(|key, bucket| {
            let age = now.duration_since(bucket.last_refill);
            if age > max_age {
                debug!("Removing stale rate limit bucket for key: {}", key);
                false
            } else {
                true
            }
        });
    }
}

/// Rate limit error types
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded {
        retry_after: Option<Duration>,
    },
}

impl RateLimitError {
    /// Convert to a user-friendly error message
    pub fn to_user_message(&self) -> String {
        match self {
            RateLimitError::RateLimitExceeded { retry_after } => {
                if let Some(duration) = retry_after {
                    format!(
                        "Too many requests. Please try again in {} seconds.",
                        duration.as_secs()
                    )
                } else {
                    "Too many requests. Please try again later.".to_string()
                }
            }
        }
    }
}

/// Global rate limiters for different command categories
pub struct GlobalRateLimiters {
    /// Rate limiter for AI/LLM operations
    pub ai_operations: RateLimiter,
    /// Rate limiter for file system operations
    pub file_operations: RateLimiter,
    /// Rate limiter for shell commands
    pub shell_commands: RateLimiter,
    /// Rate limiter for screenshot operations
    pub screenshots: RateLimiter,
    /// Rate limiter for browser operations
    pub browser_operations: RateLimiter,
}

impl GlobalRateLimiters {
    pub fn new() -> Self {
        Self {
            // AI operations: 20 per minute (expensive API calls)
            ai_operations: RateLimiter::new(RateLimitConfig::per_minute(20)),
            
            // File operations: 100 per second
            file_operations: RateLimiter::new(RateLimitConfig::per_second(100)),
            
            // Shell commands: 10 per second (security concern)
            shell_commands: RateLimiter::new(RateLimitConfig::per_second(10)),
            
            // Screenshots: 5 per second (resource intensive)
            screenshots: RateLimiter::new(RateLimitConfig::per_second(5)),
            
            // Browser operations: 30 per minute
            browser_operations: RateLimiter::new(RateLimitConfig::per_minute(30)),
        }
    }

    /// Start periodic cleanup of stale buckets
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                // Clean up buckets older than 1 hour
                let max_age = Duration::from_secs(3600);
                
                self.ai_operations.cleanup(max_age).await;
                self.file_operations.cleanup(max_age).await;
                self.shell_commands.cleanup(max_age).await;
                self.screenshots.cleanup(max_age).await;
                self.browser_operations.cleanup(max_age).await;
                
                debug!("Completed rate limiter cleanup");
            }
        });
    }
}

/// Macro for easy rate limit checking in commands
#[macro_export]
macro_rules! rate_limit_check {
    ($limiter:expr, $key:expr) => {
        match $limiter.check($key).await {
            Ok(()) => {},
            Err(e) => return Err(e.to_user_message()),
        }
    };
    ($limiter:expr, $key:expr, $cost:expr) => {
        match $limiter.check_with_cost($key, $cost).await {
            Ok(()) => {},
            Err(e) => return Err(e.to_user_message()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(2));
        
        // First two requests should succeed
        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());
        
        // Third request should fail
        assert!(limiter.check("test").await.is_err());
        
        // Different key should work
        assert!(limiter.check("other").await.is_ok());
        
        // After 1 second, should work again
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(limiter.check("test").await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_with_cost() {
        let limiter = RateLimiter::new(RateLimitConfig::per_second(10));
        
        // Request with cost 5 should succeed
        assert!(limiter.check_with_cost("test", 5.0).await.is_ok());
        
        // Another request with cost 5 should succeed (total 10)
        assert!(limiter.check_with_cost("test", 5.0).await.is_ok());
        
        // Request with cost 1 should fail (would exceed 10)
        assert!(limiter.check_with_cost("test", 1.0).await.is_err());
    }
}