use axum::{Extension, Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::{auth::sessions::AuthenticatedUser, error::ApiError, state::AppState};

/// Client input for creating an account.
///
/// `user_id`, `status`, and `id` are controlled by the server.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAccountRequest {
    pub name: String,
}

/// Public account representation returned by the API.
#[derive(Debug, Serialize, FromRow)]
pub struct AccountResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Creates an account owned by the authenticated user.
pub async fn create_account(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(request): Json<CreateAccountRequest>,
) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
    let name = request.name.trim();

    if name.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "account name cannot be empty",
        ));
    }

    let account_id = Uuid::now_v7();

    let result = sqlx::query_as::<_, AccountResponse>(
        r#"
        INSERT INTO accounts (id, user_id, name, status)
        VALUES ($1, $2, $3, 'active')
        RETURNING id, user_id, name, status, created_at
        "#,
    )
    .bind(account_id)
    // Ownership comes from authenticated middleware, never client JSON.
    .bind(user.user_id)
    .bind(name)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(account) => Ok((StatusCode::CREATED, Json(account))),

        // UNIQUE(user_id, name) prevents duplicate account names per user.
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => Err(
            ApiError::new(StatusCode::CONFLICT, "account name already exists"),
        ),

        Err(error) => {
            tracing::error!(
                %error,
                user_id = %user.user_id,
                "failed to create account"
            );

            Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create account",
            ))
        }
    }
}
