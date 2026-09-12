mod common;

use axum::http::{Method, StatusCode};
use common::{empty_request, json_request, login, send, signup};
use limit_order_book::api::{router, state::AppState};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn signup_creates_trader(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));

    signup(&app, "trader@example.com").await;

    // Verify durable authorization state, not only the HTTP response.
    let role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE email = $1")
        .bind("trader@example.com")
        .fetch_one(&pool)
        .await
        .expect("created user should exist");

    assert_eq!(role, "trader");
}

#[sqlx::test]
async fn login_authenticate_and_logout_lifecycle(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "lifecycle@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    let before_logout = send(&app, empty_request(Method::GET, "/auth/me", Some(&token))).await;
    assert_eq!(before_logout.status(), StatusCode::OK);

    let first_logout = send(
        &app,
        empty_request(Method::POST, "/auth/logout", Some(&token)),
    )
    .await;
    assert_eq!(first_logout.status(), StatusCode::NO_CONTENT);

    let after_logout = send(&app, empty_request(Method::GET, "/auth/me", Some(&token))).await;
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);

    // Repeating logout must not fail or change the original outcome.
    let repeated_logout = send(
        &app,
        empty_request(Method::POST, "/auth/logout", Some(&token)),
    )
    .await;
    assert_eq!(repeated_logout.status(), StatusCode::NO_CONTENT);

    let revoked =
        sqlx::query_scalar::<_, bool>("SELECT revoked_at IS NOT NULL FROM sessions LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("session should exist");
    assert!(revoked);
}

#[sqlx::test]
async fn expired_session_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "expired@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    // Move both timestamps into the past while preserving the schema check
    // that expiry must occur after creation.
    sqlx::query(
        r#"
        UPDATE sessions
        SET created_at = now() - interval '2 hours',
            expires_at = now() - interval '1 hour'
        "#,
    )
    .execute(&pool)
    .await
    .expect("session should be made expired");

    let response = send(&app, empty_request(Method::GET, "/auth/me", Some(&token))).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn suspended_user_is_rejected(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "suspended@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    sqlx::query("UPDATE users SET status = 'suspended' WHERE email = $1")
        .bind(email)
        .execute(&pool)
        .await
        .expect("user should be suspended");

    let response = send(&app, empty_request(Method::GET, "/auth/me", Some(&token))).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn admin_route_requires_admin_role(pool: PgPool) {
    let app = router(AppState::new(pool.clone()));
    let email = "role-check@example.com";

    signup(&app, email).await;
    let token = login(&app, email).await;

    let create_system_user = || {
        json_request(
            Method::POST,
            "/admin/users",
            json!({
                "display_name": "Market Service",
                "email": "market-service@example.com",
                "role": "system",
            }),
            Some(&token),
        )
    };

    let trader_response = send(&app, create_system_user()).await;
    assert_eq!(trader_response.status(), StatusCode::FORBIDDEN);

    // The session contains no role claim. Middleware reloads the current role
    // from PostgreSQL on every request.
    sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1")
        .bind(email)
        .execute(&pool)
        .await
        .expect("test user should be promoted");

    let admin_response = send(&app, create_system_user()).await;
    assert_eq!(admin_response.status(), StatusCode::CREATED);

    let created_role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE email = $1")
        .bind("market-service@example.com")
        .fetch_one(&pool)
        .await
        .expect("admin-created user should exist");

    assert_eq!(created_role, "system");
}
