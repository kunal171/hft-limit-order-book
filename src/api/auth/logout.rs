use axum::{
    extract::{Request, State},
    http::StatusCode,
};

use crate::api::{error::ApiError, state::AppState};

use super::sessions::{bearer_token, hash_token};

/// Revokes the session represented by the supplied bearer token.
pub async fn logout(
    State(state): State<AppState>,
    request: Request,
) -> Result<StatusCode, ApiError> {
    // Own the token before awaiting the database operation.
    let raw_token = bearer_token(&request)?.to_owned();
    let token_hash = hash_token(&raw_token);

    sqlx::query(
        r#"
        UPDATE sessions
        SET revoked_at = now()
        WHERE token_hash = $1
          AND revoked_at IS NULL
        "#,
    )
    .bind(token_hash)
    .execute(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to revoke session");

        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to log out")
    })?;

    // Return the same response for valid, already-revoked and unknown tokens.
    // This makes repeated logout requests idempotent.
    Ok(StatusCode::NO_CONTENT)
}
