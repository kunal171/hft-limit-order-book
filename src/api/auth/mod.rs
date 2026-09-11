use axum::{Router, routing::post};

use crate::api::state::AppState;

pub mod login;
pub mod signup;
pub mod sessions;

/// Routes that do not require an existing authenticated session.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup::signup))
        .route("/login", post(login::login))
}
