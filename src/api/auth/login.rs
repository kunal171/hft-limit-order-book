use argon2::{
    Argon2,
    password_hash::{PasswordVerifier, phc::PasswordHash},
};
use axum::{Json, extract::State, http::StatusCode};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use tokio::task;
use uuid::Uuid;

use crate::api::{error::ApiError, state::AppState};

// A fixed duration is sufficient initially; configuration can come later.
const SESSION_LIFETIME_HOURS: i64 = 12;

/// Credentials accepted by POST /auth/login.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Holds the client token and the hash stored in PostgreSQL.
struct GeneratedToken {
    raw: String,
    hash: Vec<u8>,
}

/// Session information returned after successful authentication.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user_id: Uuid,
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

/// Internal database result used while verifying credentials.
///
/// This is never returned directly because it contains the password hash.
#[derive(Debug, FromRow)]
struct LoginUserRow {
    pub id: Uuid,
    pub status: String,
    pub password_hash: String,
}

/// Loads the authentication information associated with an email.
async fn find_login_user(state: &AppState, email: &str) -> Result<Option<LoginUserRow>, ApiError> {
    sqlx::query_as::<_, LoginUserRow>(
        r#"
        SELECT
            users.id,
            users.status,
            user_credentials.password_hash
        FROM users
        INNER JOIN user_credentials
            ON user_credentials.user_id = users.id
        WHERE users.email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to load login credentials");

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to process login")
    })
}

/// Verifies a password without blocking a Tokio async worker.
async fn verify_password(password: String, stored_hash: String) -> Result<bool, ApiError> {
    let result = task::spawn_blocking(move || {
        let parsed_hash = PasswordHash::new(&stored_hash)?;

        Ok::<bool, argon2::password_hash::Error>(
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok(),
        )
    })
    .await;

    match result {
        Ok(Ok(matches)) => Ok(matches),

        Ok(Err(error)) => {
            // A stored hash that cannot be parsed indicates corrupted data.
            tracing::error!(%error, "stored password hash is invalid");

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to process login",
            ))
        }

        Err(error) => {
            tracing::error!(%error, "password verification task failed");

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to process login",
            ))
        }
    }
}

/// Generates a cryptographically secure 256-bit bearer token.
fn generate_session_token() -> Result<GeneratedToken, ApiError> {
    let mut random_bytes = [0_u8; 32];

    getrandom::fill(&mut random_bytes).map_err(|error| {
        tracing::error!(%error, "failed to generate secure session token");

        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create session",
        )
    })?;

    // URL-safe Base64 can be transported in an Authorization header without
    // requiring escaping. Padding is unnecessary for a random opaque token.
    let raw = URL_SAFE_NO_PAD.encode(random_bytes);

    // Store only the SHA-256 digest. A leaked database cannot directly reveal
    // usable bearer tokens.
    let hash = Sha256::digest(raw.as_bytes()).to_vec();

    Ok(GeneratedToken { raw, hash })
}

/// Verifies credentials and creates a new database-backed session.
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<(StatusCode, Json<LoginResponse>), ApiError> {
    let email = request.email.trim().to_lowercase();

    if email.is_empty() || request.password.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "email and password are required",
        ));
    }

    let user = find_login_user(&state, &email).await?.ok_or_else(|| {
        // Do not reveal whether the email address exists.
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid email or password")
    })?;

    let password_matches = verify_password(request.password, user.password_hash).await?;

    if !password_matches {
        // Use the same error as an unknown email to prevent account discovery.
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid email or password",
        ));
    }

    // Only reveal account status after valid credentials were supplied.
    if user.status != "active" {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "account is not active",
        ));
    }

    let token = generate_session_token()?;
    let session_id = Uuid::now_v7();
    let expires_at = Utc::now() + Duration::hours(SESSION_LIFETIME_HOURS);

    let GeneratedToken { raw, hash } = token;

    // Only the token hash is stored. The raw bearer token is returned once.
    sqlx::query(
        r#"
        INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(session_id)
    .bind(user.id)
    .bind(hash)
    .bind(&expires_at)
    .execute(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, user_id = %user.id, "failed to create session");

        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create session",
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            user_id: user.id,
            access_token: raw,
            expires_at,
        }),
    ))
}
