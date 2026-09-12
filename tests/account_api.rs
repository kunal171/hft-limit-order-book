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
                "name": "Primary Trading Account"
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
