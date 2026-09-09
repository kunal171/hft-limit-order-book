use axum::{Router, middleware, routing::get};

use self::state::AppState;

mod admin;
pub mod error;
mod health;
pub mod state;

/// Builds the complete HTTP router.
pub fn router(state: AppState) -> Router {
    // Every route nested under /admin passes through this middleware.
    let admin_routes = admin::router().route_layer(middleware::from_fn_with_state(
        state.clone(),
        admin::auth::require_admin_key,
    ));

    Router::new()
        .route("/health", get(health::health))
        .nest("/admin", admin_routes)
        .with_state(state)
}
