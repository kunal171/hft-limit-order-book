use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::api::{error::ApiError, state::AppState};
use argon2::{Argon2, password_hash::PasswordHasher};
use tokio::task;
use uuid::Uuid;

/// Information accepted from a new trader.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignupRequest {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

/// Public user information returned after registration.
#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
    pub status: String,
    pub role: String,
}

/// Normalizes and validates public signup data.
fn validate_signup(request: SignupRequest) -> Result<SignupRequest, ApiError> {
    let display_name = request.display_name.trim().to_string();
    let email = request.email.trim().to_lowercase();

    if display_name.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "display_name cannot be empty",
        ));
    }

    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "invalid email"));
    }

    if request.password.chars().count() < 15 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "password must contain at least 15 characters",
        ));
    }

    Ok(SignupRequest {
        display_name,
        email,
        password: request.password,
    })
}

/// Hashes a password on Tokio's blocking thread pool.
///
/// Argon2 is intentionally CPU-intensive, so running it directly inside an
/// async handler could delay unrelated API requests.
async fn hash_password(password: String) -> Result<String, ApiError> {
    let result = task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
    })
    .await;

    match result {
        Ok(Ok(password_hash)) => Ok(password_hash),

        Ok(Err(error)) => {
            tracing::error!(%error, "failed to hash password");

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to process credentials",
            ))
        }

        Err(error) => {
            tracing::error!(%error, "password hashing task failed");

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to process credentials",
            ))
        }
    }
}

/// Creates a trader and their password credentials atomically.
pub async fn signup(
    State(state): State<AppState>,
    Json(request): Json<SignupRequest>,
) -> Result<(StatusCode, Json<SignupResponse>), ApiError> {
    let request = validate_signup(request)?;

    // Destructuring gives ownership of each validated field.
    let SignupRequest {
        display_name,
        email,
        password,
    } = request;

    // Hash before opening the transaction because Argon2 is expensive.
    let password_hash = hash_password(password).await?;
    let user_id = Uuid::now_v7();

    let mut tx = state.db.begin().await.map_err(|error| {
        tracing::error!(%error, "failed to begin signup transaction");

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to create user")
    })?;

    // Role is hard-coded so public users cannot create admin accounts.
    let user_result = sqlx::query(
        r#"
        INSERT INTO users (id, display_name, email, status, role)
        VALUES ($1, $2, $3, 'active', 'trader')
        "#,
    )
    .bind(user_id)
    .bind(&display_name)
    .bind(&email)
    .execute(&mut *tx)
    .await;

    match user_result {
        Ok(_) => {}

        // PostgreSQL error 23505 means a UNIQUE constraint was violated.
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
            return Err(ApiError::new(StatusCode::CONFLICT, "email already exists"));
        }

        Err(error) => {
            tracing::error!(%error, "failed to insert signup user");

            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create user",
            ));
        }
    }

    // This uses the same transaction as the user insert. If it fails, dropping
    // the transaction rolls back the previously inserted user.
    sqlx::query(
        r#"
        INSERT INTO user_credentials (user_id, password_hash)
        VALUES ($1, $2)
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to insert user credentials");

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to create user")
    })?;

    tx.commit().await.map_err(|error| {
        tracing::error!(%error, "failed to commit signup transaction");

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to create user")
    })?;

    Ok((
        StatusCode::CREATED,
        Json(SignupResponse {
            id: user_id,
            display_name,
            email,
            status: "active".to_string(),
            role: "trader".to_string(),
        }),
    ))
}
