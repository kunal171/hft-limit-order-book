mod common;

use axum::http::{Method, StatusCode};
use common::{json_request, login, send, signup};
use limit_order_book::api::{router, state::AppState};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn authenticated_user_creates_own_account(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "account-owner@example.com";

    // Create and authenticate the user.
    signup(&app, email).await;
    let token = login(&app, email).await;

    // The client supplies only the account name.
    let response = send(
        &app,
        json_request(
            Method::POST,
            "/accounts",
            json!({
                "name": "  Primary Trading Account  "
            }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify that ownership came from the bearer token.
    let owner_email = sqlx::query_scalar::<_, String>(
        r#"
        SELECT users.email
        FROM accounts
        JOIN users ON users.id = accounts.user_id
        WHERE accounts.name = $1
        "#,
    )
    .bind("Primary Trading Account")
    .fetch_one(&pool)
    .await
    .expect("created account should exist");

    assert_eq!(owner_email, email);
}

#[sqlx::test]
async fn creating_account_without_token_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool));

    // No bearer token is supplied.
    let response = send(
        &app,
        json_request(
            Method::POST,
            "/accounts",
            json!({
                "name": "Unauthorized Account"
            }),
            None,
        ),
    )
    .await;

    // Authentication middleware must reject the request
    // before the account handler executes.
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn duplicate_account_name_for_same_user_returns_conflict(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "duplicate-account@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    // Build a fresh request each time because an HTTP request is consumed.
    let create_account = || {
        json_request(
            Method::POST,
            "/accounts",
            json!({
                "name": "Trading Account"
            }),
            Some(&token),
        )
    };

    let first_response = send(&app, create_account()).await;
    assert_eq!(first_response.status(), StatusCode::CREATED);

    // UNIQUE(user_id, name) rejects the second account.
    let duplicate_response = send(&app, create_account()).await;
    assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);

    // Only the original account should remain in PostgreSQL.
    let account_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .expect("account count should be readable");

    assert_eq!(account_count, 1);
}

#[sqlx::test]
async fn different_users_can_use_the_same_account_name(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let first_email = "first-owner@example.com";
    let second_email = "second-owner@example.com";

    signup(&app, first_email).await;
    signup(&app, second_email).await;

    let first_token = login(&app, first_email).await;
    let second_token = login(&app, second_email).await;

    // Account names are unique per user, not across the entire system.
    for token in [&first_token, &second_token] {
        let response = send(
            &app,
            json_request(
                Method::POST,
                "/accounts",
                json!({ "name": "Primary" }),
                Some(token),
            ),
        )
        .await;

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let account_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts WHERE name = $1")
            .bind("Primary")
            .fetch_one(&pool)
            .await
            .expect("account count should be readable");

    assert_eq!(account_count, 2);
}

#[sqlx::test]
async fn empty_account_name_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "empty-name@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    // Trimming turns whitespace-only input into an empty name.
    let response = send(
        &app,
        json_request(
            Method::POST,
            "/accounts",
            json!({ "name": "   " }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let account_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .expect("account count should be readable");

    assert_eq!(account_count, 0);
}

#[sqlx::test]
async fn client_cannot_set_account_owner_or_status(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "controlled-fields@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    // deny_unknown_fields prevents clients from supplying server-owned fields.
    let response = send(
        &app,
        json_request(
            Method::POST,
            "/accounts",
            json!({
                "name": "Injected Account",
                "user_id": "01994f09-6058-7f72-9408-b6d6329e8058",
                "status": "active"
            }),
            Some(&token),
        ),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let account_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .expect("account count should be readable");

    assert_eq!(account_count, 0);
}
