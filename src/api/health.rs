use super::state::AppState;
use axum::{extract::State, http::StatusCode};
use sqlx::Postgres;

pub async fn health(State(state): State<AppState>) -> StatusCode {
    let database_is_available = sqlx::query_scalar::<Postgres, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    if database_is_available {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
