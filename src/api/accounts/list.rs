use axum::{Extension, Json, extract::State, http::StatusCode};

use super::model::AccountResponse;
use crate::api::{auth::sessions::AuthenticatedUser, error::ApiError, state::AppState};

/// Lists every account owned by the authenticated user.
pub async fn list_accounts(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<Vec<AccountResponse>>, ApiError> {
    let accounts = sqlx::query_as::<_, AccountResponse>(
        r#"
        SELECT id, user_id, name, status, created_at
        FROM accounts
        WHERE user_id = $1
        ORDER BY created_at, id
        "#,
    )
    // Never accept user_id from the client for this query.
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(
            %error,
            user_id = %user.user_id,
            "failed to list accounts"
        );

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list accounts")
    })?;

    Ok(Json(accounts))
}
