//! Per-client request limiting for expensive public routes.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::api::{error::ApiError, state::AppState};

/// Above this many tracked clients, idle buckets are dropped.
const MAX_TRACKED_CLIENTS: usize = 10_000;

/// How many requests a client may make, and how fast the allowance returns.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Requests allowed in one burst.
    pub burst: u32,
    /// Tokens added back per second.
    pub refill_per_second: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        // Ten quick attempts, then one more every five seconds.
        Self {
            burst: 10,
            refill_per_second: 0.2,
        }
    }
}

/// One client's remaining allowance.
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// Token-bucket limiter keyed by client IP address.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<Mutex<HashMap<IpAddr, Bucket>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Takes one token for `client`. Returns false when none are left.
    ///
    /// `now` is a parameter so tests can move time forward without sleeping.
    pub fn try_acquire(&self, client: IpAddr, now: Instant) -> bool {
        let burst = f64::from(self.config.burst);
        let refill_per_second = self.config.refill_per_second;

        // The lock is held only for this short calculation, never across an await.
        let mut buckets = self
            .buckets
            .lock()
            .expect("rate limiter lock should not be poisoned");

        if buckets.len() >= MAX_TRACKED_CLIENTS {
            // A bucket that has refilled completely carries no information.
            buckets.retain(|_, bucket| {
                let idle = now.saturating_duration_since(bucket.last_refill);
                bucket.tokens + idle.as_secs_f64() * refill_per_second < burst
            });
        }

        let bucket = buckets.entry(client).or_insert(Bucket {
            tokens: burst,
            last_refill: now,
        });

        let elapsed = now.saturating_duration_since(bucket.last_refill);
        bucket.tokens = (bucket.tokens + elapsed.as_secs_f64() * refill_per_second).min(burst);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Rejects a client that has used up its allowance on the auth routes.
pub async fn limit_auth_requests(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // The server records each connection's peer address. Requests sent straight
    // into the router, as integration tests do, have none and share one bucket.
    let client = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    if !state.auth_rate_limiter.try_acquire(client, Instant::now()) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "too many requests, try again later",
        ));
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    const OTHER_CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    fn limiter() -> RateLimiter {
        RateLimiter::new(RateLimitConfig {
            burst: 3,
            refill_per_second: 1.0,
        })
    }

    #[test]
    fn allows_a_burst_then_rejects() {
        let limiter = limiter();
        let now = Instant::now();

        assert!(limiter.try_acquire(CLIENT, now));
        assert!(limiter.try_acquire(CLIENT, now));
        assert!(limiter.try_acquire(CLIENT, now));
        assert!(!limiter.try_acquire(CLIENT, now));
    }

    #[test]
    fn allowance_returns_over_time() {
        let limiter = limiter();
        let start = Instant::now();
        for _ in 0..3 {
            limiter.try_acquire(CLIENT, start);
        }

        // One second at one token per second buys exactly one request.
        let later = start + Duration::from_secs(1);
        assert!(limiter.try_acquire(CLIENT, later));
        assert!(!limiter.try_acquire(CLIENT, later));
    }

    #[test]
    fn allowance_never_exceeds_the_burst() {
        let limiter = limiter();
        let start = Instant::now();
        limiter.try_acquire(CLIENT, start);

        // A long idle period must not bank more than one burst.
        let much_later = start + Duration::from_secs(3_600);
        for _ in 0..3 {
            assert!(limiter.try_acquire(CLIENT, much_later));
        }
        assert!(!limiter.try_acquire(CLIENT, much_later));
    }

    #[test]
    fn clients_are_limited_independently() {
        let limiter = limiter();
        let now = Instant::now();
        for _ in 0..3 {
            limiter.try_acquire(CLIENT, now);
        }

        assert!(!limiter.try_acquire(CLIENT, now));
        assert!(limiter.try_acquire(OTHER_CLIENT, now));
    }
}
