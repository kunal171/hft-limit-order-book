use sqlx::PgPool;
use std::sync::Arc;

/// Dependencies shared by API handlers and middleware.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,

    // Arc allows cheap cloning when Axum clones application state.
    pub admin_api_key: Arc<str>,
}

impl AppState {
    pub fn new(db: PgPool, admin_api_key: String) -> Self {
        Self {
            db,
            admin_api_key: Arc::from(admin_api_key),
        }
    }
}
