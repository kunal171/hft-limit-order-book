use axum::{Router, routing::get};

use self::state::AppState;

mod admin;
mod health;
pub mod state;

/// Builds the complete HTTP router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .nest("/admin", admin::router())
        .with_state(state)
}
