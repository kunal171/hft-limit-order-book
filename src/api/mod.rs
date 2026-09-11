use axum::{Router, middleware, routing::get};

use self::state::AppState;

mod admin;
pub mod auth;
pub mod error;
mod health;
pub mod state;

/// Builds the complete HTTP router.
pub fn router(state: AppState) -> Router {
    // Every route nested under /admin passes through this middleware.
    let admin_routes = admin::router()
        // This layer runs second and checks the authenticated user's role.
        .route_layer(middleware::from_fn(admin::auth::require_admin))
        // The last-added layer runs first and validates the bearer session.
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::sessions::require_authenticated,
        ));

    Router::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::router(state.clone()))
        .nest("/admin", admin_routes)
        .with_state(state)
}
