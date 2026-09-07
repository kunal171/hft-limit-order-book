use sqlx::PgPool;

/// Shared dependencies available to every API handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}
