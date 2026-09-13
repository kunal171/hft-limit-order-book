use axum::{Extension, Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use tracing::instrument;
use uuid::Uuid;

use crate::api::{auth::sessions::AuthenticatedUser, error::ApiError, state::AppState};

/// Instrument categories supported by the PostgreSQL asset_class enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "asset_class", rename_all = "lowercase")]
pub enum AssetClass {
    Crypto,
    Stock,
    Futures,
    Commodities,
}

/// Trading states supported by the PostgreSQL market_status enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "market_status", rename_all = "lowercase")]
pub enum MarketStatus {
    Active,
    Paused,
    Delisted,
}

/// Values an administrator supplies when creating a market.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateInstrumentRequest {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_scale: i32,
    pub quantity_scale: i32,
    pub tick_size: i64,
    pub lot_size: i64,
}

/// Instrument representation returned by the API.
#[derive(Debug, Serialize, FromRow)]
pub struct InstrumentResponse {
    pub id: Uuid,
    pub symbol: String,
    pub asset_class: AssetClass,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_scale: i32,
    pub quantity_scale: i32,
    pub tick_size: i64,
    pub lot_size: i64,
    pub status: MarketStatus,
    pub created_at: DateTime<Utc>,
}

pub async fn create_asset(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(request): Json<CreateInstrumentRequest>,
) -> Result<(StatusCode, Json<InstrumentResponse>), ApiError> {
    let request = match validate_instrument(request) {
        Ok(request) => request,
        Err(error) => return Err(error),
    };

    let asset_id = Uuid::now_v7();
    let market_status = MarketStatus::Active;

    let result = sqlx::query_as::<_, InstrumentResponse>(
        r#"
        INSERT INTO instruments(
            id, 
            symbol, 
            asset_class, 
            base_asset, 
            quote_asset, 
            price_scale, 
            quantity_scale,
            tick_size,
            lot_size,
            status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(asset_id)
    .bind(request.symbol)
    .bind(request.asset_class)
    .bind(request.base_asset)
    .bind(request.quote_asset)
    .bind(request.price_scale)
    .bind(request.quantity_scale)
    .bind(request.tick_size)
    .bind(request.lot_size)
    .bind(market_status)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(instrument) => Ok((StatusCode::CREATED, Json(instrument))),

        // UNIQUE(Asset Symbol) prevents duplicate Asset Symbols.
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            Err(ApiError::new(StatusCode::CONFLICT, "Asset already exists"))
        }

        Err(error) => {
            tracing::error!(
                %error,
                user_id = %user.user_id,
                "failed to create Asset"
            );

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create Asset",
            ))
        }
    }
}

/// Normalizes and validates administrator-supplied instrument data.
fn validate_instrument(
    mut request: CreateInstrumentRequest,
) -> Result<CreateInstrumentRequest, ApiError> {
    // Market identifiers use one canonical uppercase representation.
    request.symbol = request.symbol.trim().to_ascii_uppercase();
    request.base_asset = request.base_asset.trim().to_ascii_uppercase();
    request.quote_asset = request.quote_asset.trim().to_ascii_uppercase();

    if request.symbol.is_empty() || request.base_asset.is_empty() || request.quote_asset.is_empty()
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "symbol and assets cannot be empty",
        ));
    }

    // An instrument such as USD-USD would not represent a useful market.
    if request.base_asset == request.quote_asset {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "base_asset and quote_asset must differ",
        ));
    }

    // Scales represent decimal places used by fixed-point integers.
    if !(0..=18).contains(&request.price_scale) || !(0..=18).contains(&request.quantity_scale) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "scales must be between 0 and 18",
        ));
    }

    // Zero or negative increments would make order validation meaningless.
    if request.tick_size <= 0 || request.lot_size <= 0 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "tick_size and lot_size must be positive",
        ));
    }

    // Keep symbols predictable for logs, URLs, and engine routing.
    let valid_symbol = request
        .symbol
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-');

    if !valid_symbol {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "symbol may contain only letters, numbers, and hyphens",
        ));
    }

    Ok(request)
}
