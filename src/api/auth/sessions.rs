use axum::{Extension, Json};
use axum::{
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::{error::ApiError, state::AppState};

/// Trusted identity produced after validating a database session.
///
/// Handlers may trust this value because middleware creates it.
/// Never construct it from JSON supplied by the client.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub role: String,
}

/// Session identity loaded from PostgreSQL.
///
/// Expired and revoked sessions are filtered out by the SQL query.
#[derive(Debug, FromRow)]
struct SessionUserRow {
    user_id: Uuid,
    role: String,
    status: String,
}

/// Identity returned for the currently authenticated session.
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user_id: Uuid,
    pub role: String,
}

/// Extracts the raw token from `Authorization: Bearer <token>`.
fn bearer_token(request: &Request) -> Result<&str, ApiError> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing authorization token"))?;

    let (scheme, token) = header
        .split_once(' ')
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "invalid authorization token"))?;

    // Authentication schemes are case-insensitive.
    if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid authorization token",
        ));
    }

    Ok(token)
}

/// Hashes a raw bearer token and finds its valid database session.
async fn find_valid_session(
    state: &AppState,
    raw_token: &str,
) -> Result<Option<SessionUserRow>, ApiError> {
    // Login stores this same SHA-256 digest, never the raw token.
    let token_hash = Sha256::digest(raw_token.as_bytes()).to_vec();

    sqlx::query_as::<_, SessionUserRow>(
        r#"
        SELECT
            sessions.user_id,
            users.role,
            users.status
        FROM sessions
        INNER JOIN users
            ON users.id = sessions.user_id
        WHERE sessions.token_hash = $1
          AND sessions.revoked_at IS NULL
          AND sessions.expires_at > now()
        "#,
    )
    .bind(token_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| {
        tracing::error!(%error, "failed to validate session");

        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to validate session",
        )
    })
}

/// Authenticates a request using its bearer token.
///
/// Successful authentication inserts a trusted user identity into the
/// request extensions for downstream handlers and authorization middleware.
pub async fn require_authenticated(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Own the token so we do not keep a reference into `request` across await.
    let raw_token = bearer_token(&request)?.to_owned();

    let session = find_valid_session(&state, &raw_token)
        .await?
        .ok_or_else(|| {
            // Invalid, expired and revoked sessions intentionally return
            // the same response so callers cannot distinguish them.
            ApiError::new(StatusCode::UNAUTHORIZED, "invalid or expired session")
        })?;

    if session.status != "active" {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "account is not active",
        ));
    }

    // Extensions carry trusted request-scoped data to downstream handlers.
    request.extensions_mut().insert(AuthenticatedUser {
        user_id: session.user_id,
        role: session.role,
    });

    Ok(next.run(request).await)
}

/// Returns identity inserted by `require_authenticated`.
pub async fn current_user(
    Extension(user): Extension<AuthenticatedUser>,
) -> Json<CurrentUserResponse> {
    Json(CurrentUserResponse {
        user_id: user.user_id,
        role: user.role,
    })
}
