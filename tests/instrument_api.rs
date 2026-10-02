mod common;

use axum::http::{Method, StatusCode};
use common::{admin_token, json_request, read_json, send};
use limit_order_book::api::{router, state::AppState};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::{login, signup};

#[derive(Debug, Deserialize)]
struct CreatedInstrument {
    id: Uuid,
    symbol: String,
    asset_class: String,
    base_asset: String,
    quote_asset: String,
    status: String,
}

/// Builds one valid instrument request for integration tests.
fn instrument_request(symbol: &str, base_asset: &str, quote_asset: &str) -> Value {
    json!({
        "symbol": symbol,
        "asset_class": "crypto",
        "base_asset": base_asset,
        "quote_asset": quote_asset,
        "price_scale": 2,
        "quantity_scale": 6,
        "tick_size": 1,
        "lot_size": 1000
    })
}

#[sqlx::test]
async fn admin_creates_multiple_instruments_atomically(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    // Create a login-capable administrator for this isolated test database.
    let token = admin_token(&app, &pool, "instrument-admin@example.com").await;

    let response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            json!(
                [
                    {
                         "symbol": " btc-usdt ",
                        "asset_class": "crypto",
                        "base_asset": " btc ",
                        "quote_asset": " usdt ",
                        "price_scale": 2,
                        "quantity_scale": 6,
                        "tick_size": 1,
                        "lot_size": 1000
                    },
                    {
                        "symbol": "eth-usdt",
                        "asset_class": "crypto",
                        "base_asset": "eth",
                        "quote_asset": "usdt",
                        "price_scale": 2,
                        "quantity_scale": 6,
                        "tick_size": 1,
                        "lot_size": 1000
                    }
                ]
            ),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let instruments: Vec<CreatedInstrument> = read_json(response).await;
    assert_eq!(instruments.len(), 2);

    // Input should be normalized before storage and response.
    assert_eq!(instruments[0].symbol, "BTC-USDT");
    assert_eq!(instruments[0].base_asset, "BTC");
    assert_eq!(instruments[0].quote_asset, "USDT");
    assert_eq!(instruments[0].asset_class, "crypto");
    assert_eq!(instruments[0].status, "active");
    assert!(!instruments[0].id.is_nil());

    // Both inserts must have been committed.
    let instrument_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instruments")
        .fetch_one(&pool)
        .await
        .expect("instrument count should be readable");

    assert_eq!(instrument_count, 2);
}

#[sqlx::test]
async fn trader_cannot_create_instruments(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));

    signup(&app, "instrument-trader@example.com").await;
    let token = login(&app, "instrument-trader@example.com").await;

    let response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            Value::Array(vec![instrument_request("BTC-USDT", "BTC", "USDT")]),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Authorization must fail before anything is inserted.
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instruments")
        .fetch_one(&pool)
        .await
        .expect("instrument count should be readable");

    assert_eq!(count, 0);
}

#[sqlx::test]
async fn duplicate_symbol_rolls_back_entire_batch(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "rollback-admin@example.com").await;

    // Create the symbol that will cause the later conflict.
    let initial_response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            Value::Array(vec![instrument_request("BTC-USDT", "BTC", "USDT")]),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(initial_response.status(), StatusCode::CREATED);

    // ETH is inserted first inside the transaction, then BTC conflicts.
    let conflict_response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            Value::Array(vec![
                instrument_request("ETH-USDT", "ETH", "USDT"),
                instrument_request("BTC-USDT", "BTC", "USDT"),
            ]),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(conflict_response.status(), StatusCode::CONFLICT);

    // ETH must have been rolled back; only the original BTC remains.
    let symbols = sqlx::query_scalar::<_, String>("SELECT symbol FROM instruments ORDER BY symbol")
        .fetch_all(&pool)
        .await
        .expect("instrument symbols should be readable");

    assert_eq!(symbols, vec!["BTC-USDT"]);
}

#[sqlx::test]
async fn invalid_instrument_batches_are_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "validation-admin@example.com").await;

    let invalid_symbol = instrument_request("BTC/USDT", "BTC", "USDT");

    let same_assets = instrument_request("USD-USD", "USD", "USD");

    let mut invalid_scale = instrument_request("ETH-USDT", "ETH", "USDT");
    invalid_scale["price_scale"] = json!(19);

    let mut invalid_quantity_scale = instrument_request("XRP-USDT", "XRP", "USDT");
    invalid_quantity_scale["quantity_scale"] = json!(-1);

    let mut invalid_tick = instrument_request("SOL-USDT", "SOL", "USDT");
    invalid_tick["tick_size"] = json!(0);

    let mut invalid_lot = instrument_request("AAPL-USD", "AAPL", "USD");
    invalid_lot["lot_size"] = json!(0);

    let cases = vec![
        ("empty batch", Value::Array(vec![])),
        ("invalid symbol", Value::Array(vec![invalid_symbol])),
        ("same assets", Value::Array(vec![same_assets])),
        ("invalid scale", Value::Array(vec![invalid_scale])),
        (
            "invalid quantity scale",
            Value::Array(vec![invalid_quantity_scale]),
        ),
        ("invalid tick", Value::Array(vec![invalid_tick])),
        ("invalid lot", Value::Array(vec![invalid_lot])),
    ];

    for (case, body) in cases {
        let response = send(
            &app,
            json_request(Method::POST, "/admin/instruments", body, Some(&token)),
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "failed validation case: {case}",
        );
    }

    // Invalid requests must never create partial data.
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instruments")
        .fetch_one(&pool)
        .await
        .expect("instrument count should be readable");

    assert_eq!(count, 0);
}

#[sqlx::test]
async fn instrument_creation_without_token_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));

    let response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            Value::Array(vec![instrument_request("BTC-USDT", "BTC", "USDT")]),
            None,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Authentication must fail before the handler inserts anything.
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instruments")
        .fetch_one(&pool)
        .await
        .expect("instrument count should be readable");

    assert_eq!(count, 0);
}

#[sqlx::test]
async fn instrument_batch_larger_than_limit_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "batch-limit-admin@example.com").await;

    // Generate one more request than the endpoint's configured maximum.
    let instruments = (0..101)
        .map(|index| {
            instrument_request(
                &format!("ASSET{index}-USD"),
                &format!("ASSET{index}"),
                "USD",
            )
        })
        .collect();

    let response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            Value::Array(instruments),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instruments")
        .fetch_one(&pool)
        .await
        .expect("instrument count should be readable");

    assert_eq!(count, 0);
}

#[sqlx::test]
async fn admin_can_pause_unpause_and_delist_instrument(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "status-admin@example.com").await;

    let create_response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            json!([instrument_request("BTC-USDT", "BTC", "USDT")]),
            Some(&token),
        ),
    )
    .await;

    let created: Vec<CreatedInstrument> = read_json(create_response).await;
    let instrument_id = created[0].id;
    let status_uri = format!("/admin/instruments/{instrument_id}/status");

    for (status, expected) in [
        ("paused", "paused"),
        ("active", "active"),
        ("delisted", "delisted"),
    ] {
        let response = send(
            &app,
            json_request(
                Method::PATCH,
                &status_uri,
                json!({ "status": status }),
                Some(&token),
            ),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let instrument: CreatedInstrument = read_json(response).await;
        assert_eq!(instrument.status, expected);
    }
}

#[sqlx::test]
async fn delisted_instrument_cannot_be_reactivated(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "delisted-admin@example.com").await;

    let create_response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            json!([instrument_request("ETH-USDT", "ETH", "USDT")]),
            Some(&token),
        ),
    )
    .await;

    let created: Vec<CreatedInstrument> = read_json(create_response).await;
    let status_uri = format!("/admin/instruments/{}/status", created[0].id);

    let delist_response = send(
        &app,
        json_request(
            Method::PATCH,
            &status_uri,
            json!({ "status": "delisted" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(delist_response.status(), StatusCode::OK);

    let reactivate_response = send(
        &app,
        json_request(
            Method::PATCH,
            &status_uri,
            json!({ "status": "active" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(reactivate_response.status(), StatusCode::CONFLICT);
}

#[sqlx::test]
async fn updating_missing_instrument_status_returns_not_found(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "missing-status-admin@example.com").await;

    let instrument_id = Uuid::now_v7();
    let response = send(
        &app,
        json_request(
            Method::PATCH,
            &format!("/admin/instruments/{instrument_id}/status"),
            json!({ "status": "paused" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn invalid_instrument_status_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "invalid-status-admin@example.com").await;

    // Serde rejects values outside the MarketStatus enum before SQL is executed.
    let instrument_id = Uuid::now_v7();
    let response = send(
        &app,
        json_request(
            Method::PATCH,
            &format!("/admin/instruments/{instrument_id}/status"),
            json!({ "status": "closed" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn setting_same_instrument_status_is_idempotent(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let token = admin_token(&app, &pool, "idempotent-status-admin@example.com").await;

    let create_response = send(
        &app,
        json_request(
            Method::POST,
            "/admin/instruments",
            json!([instrument_request("SOL-USDT", "SOL", "USDT")]),
            Some(&token),
        ),
    )
    .await;

    let created: Vec<CreatedInstrument> = read_json(create_response).await;
    let instrument_id = created[0].id;

    // Newly created instruments are active, so this repeats the current state.
    let response = send(
        &app,
        json_request(
            Method::PATCH,
            &format!("/admin/instruments/{instrument_id}/status"),
            json!({ "status": "active" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let instrument: CreatedInstrument = read_json(response).await;
    assert_eq!(instrument.status, "active");
}

#[sqlx::test]
async fn updating_instrument_status_without_token_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool));

    // The instrument does not need to exist because authentication runs
    // before the status handler reaches the database.
    let instrument_id = Uuid::now_v7();

    let response = send(
        &app,
        json_request(
            Method::PATCH,
            &format!("/admin/instruments/{instrument_id}/status"),
            json!({ "status": "paused" }),
            None,
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn trader_cannot_update_instrument_status(pool: PgPool) {
    let app = router(AppState::new(pool));

    // Public signup always creates a normal trader, never an administrator.
    signup(&app, "status-trader@example.com").await;
    let token = login(&app, "status-trader@example.com").await;

    // The instrument does not need to exist because authorization rejects
    // the request before the status handler runs.
    let instrument_id = Uuid::now_v7();

    let response = send(
        &app,
        json_request(
            Method::PATCH,
            &format!("/admin/instruments/{instrument_id}/status"),
            json!({ "status": "paused" }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn database_rejects_non_positive_tick_size(pool: PgPool) {
    // Insert directly, skipping handler validation.
    let error = sqlx::query(
        r#"
        INSERT INTO instruments (
            id, symbol, asset_class, base_asset, quote_asset,
            price_scale, quantity_scale, tick_size, lot_size, status
        )
        VALUES ($1, 'BAD-USD', 'crypto', 'BAD', 'USD', 2, 6, 0, 1000, 'active')
        "#,
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .expect_err("zero tick size should violate a check constraint");

    let code = error
        .as_database_error()
        .and_then(|error| error.code().map(|code| code.to_string()));

    // 23514 is PostgreSQL's check_violation.
    assert_eq!(code.as_deref(), Some("23514"));
}
