use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::api::{rate_limit, state::AppState};

pub mod login;
pub mod logout;
pub mod role;
pub mod sessions;
pub mod signup;
pub mod status;

/// Builds public authentication routes and protected session routes.
pub fn router(state: AppState) -> Router<AppState> {
    // Both routes run Argon2, so each client gets a limited number of attempts.
    let limited_routes = Router::new()
        .route("/signup", post(signup::signup))
        .route("/login", post(login::login))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::limit_auth_requests,
        ));

    let protected_routes = Router::new()
        .route("/me", get(sessions::current_user))
        // Only routes in this router pass through session authentication.
        .route_layer(middleware::from_fn_with_state(
            state,
            sessions::require_authenticated,
        ));

    Router::new()
        .route("/logout", post(logout::logout))
        .merge(limited_routes)
        .merge(protected_routes)
}
