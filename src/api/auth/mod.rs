use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::api::state::AppState;

pub mod login;
pub mod logout;
pub mod sessions;
pub mod signup;

/// Builds public authentication routes and protected session routes.
pub fn router(state: AppState) -> Router<AppState> {
    let protected_routes = Router::new()
        .route("/me", get(sessions::current_user))
        // Only routes in this router pass through session authentication.
        .route_layer(middleware::from_fn_with_state(
            state,
            sessions::require_authenticated,
        ));

    Router::new()
        .route("/signup", post(signup::signup))
        .route("/login", post(login::login))
        .route("/logout", post(logout::logout))
        .merge(protected_routes)
}
