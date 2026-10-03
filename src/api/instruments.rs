use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use uuid::Uuid;

use crate::api::{admin::instruments::InstrumentResponse, error::ApiError, state::AppState};

/// Builds read-only instrument routes.
///
/// Authentication is applied to this entire router in `api/mod.rs`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_instruments))
        .route("/{instrument_id}", get(get_instrument))
}

/// Lists every instrument, ordered by symbol.
pub async fn list_instruments(
    State(state): State<AppState>,
) -> Result<Json<Vec<InstrumentResponse>>, ApiError> {
    let instruments = sqlx::query_as::<_, InstrumentResponse>(
        r#"
        SELECT
            id,
            symbol,
            asset_class,
            base_asset,
            quote_asset,
            price_scale,
            quantity_scale,
            tick_size,
            lot_size,
            status,
            created_at
        FROM instruments
        ORDER BY symbol
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to list instruments");

        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to list instruments",
        )
    })?;

    Ok(Json(instruments))
}

/// Returns one instrument by id.
pub async fn get_instrument(
    State(state): State<AppState>,
    Path(instrument_id): Path<Uuid>,
) -> Result<Json<InstrumentResponse>, ApiError> {
    let instrument = sqlx::query_as::<_, InstrumentResponse>(
        r#"
        SELECT
            id,
            symbol,
            asset_class,
            base_asset,
            quote_asset,
            price_scale,
            quantity_scale,
            tick_size,
            lot_size,
            status,
            created_at
        FROM instruments
        WHERE id = $1
        "#,
    )
    .bind(instrument_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, %instrument_id, "failed to load instrument");

        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to load instrument",
        )
    })?
    .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "instrument not found"))?;

    Ok(Json(instrument))
}
