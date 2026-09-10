use axum::{Router, routing::post};

use crate::api::state::AppState;

pub mod signup;

/// Routes that do not require an existing authenticated session.
pub fn router() -> Router<AppState> {
    Router::new().route("/signup", post(signup::signup))
}
