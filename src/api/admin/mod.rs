use axum::{Router, routing::post};

use crate::api::state::AppState;

pub mod users;
pub mod auth;

pub fn router() -> Router<AppState> {
    Router::new().route("/users", post(users::create_user))
}
