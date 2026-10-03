use std::time::Instant;

use crate::observability::metrics::{
    DATABASE_HEALTH_CHECK_DURATION_SECONDS, DATABASE_HEALTH_CHECKS_TOTAL,
};
use ::metrics::{counter, histogram};
use axum::{extract::State, http::StatusCode};
use sqlx::Postgres;

use super::state::AppState;

pub async fn health(State(state): State<AppState>) -> StatusCode {
    // Start measuring how long the database health query takes.
    let started_at = Instant::now();

    let database_is_available = sqlx::query_scalar::<Postgres, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    // Use a bounded label with only two possible values.
    let outcome = if database_is_available {
        "success"
    } else {
        "failure"
    };

    // Counter: records how many database health checks finished.
    counter!(
        DATABASE_HEALTH_CHECKS_TOTAL,
        "outcome" => outcome
    )
    .increment(1);

    // Histogram: records the duration of every database health check.
    histogram!(DATABASE_HEALTH_CHECK_DURATION_SECONDS).record(started_at.elapsed());

    if database_is_available {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
