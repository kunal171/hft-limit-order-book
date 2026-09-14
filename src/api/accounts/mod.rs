use axum::{Router, routing::post};

use crate::api::state::AppState;

pub mod create;
pub mod list;
pub mod model;

/// Builds account routes.
///
/// Authentication is applied to this entire router in `api/mod.rs`.
pub fn router() -> Router<AppState> {
    Router::new().route("/", post(create::create_account).get(list::list_accounts))
}
