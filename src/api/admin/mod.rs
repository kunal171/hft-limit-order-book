use axum::{Router, routing::{patch, post}};

use crate::api::state::AppState;

pub mod auth;
pub mod instruments;
pub mod users;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", post(users::create_user))
        .route("/instruments", post(instruments::create_instruments))
        .route(
            "/instruments/{instrument_id}",
            patch(instruments::update_instrument),
        )
        .route(
            "/instruments/{instrument_id}/status",
            patch(instruments::update_instrument_status),
        )
}
