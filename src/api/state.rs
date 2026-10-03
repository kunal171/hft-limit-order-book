use sqlx::PgPool;

use super::rate_limit::{RateLimitConfig, RateLimiter};

/// Dependencies shared by API handlers and middleware.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Limits signup and login attempts per client.
    pub auth_rate_limiter: RateLimiter,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self::with_auth_rate_limit(db, RateLimitConfig::default())
    }

    /// Builds state with a custom allowance for the auth routes.
    pub fn with_auth_rate_limit(db: PgPool, config: RateLimitConfig) -> Self {
        Self {
            db,
            auth_rate_limiter: RateLimiter::new(config),
        }
    }
}
