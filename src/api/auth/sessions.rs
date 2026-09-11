use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
};
use uuid::Uuid;

use crate::api::error::ApiError;

/// Trusted identity produced after validating a database session.
///
/// Handlers may trust this value because middleware creates it.
/// Never construct it from JSON supplied by the client.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub role: String,
}

/// Extracts the raw token from `Authorization: Bearer <token>`.
fn bearer_token(request: &Request) -> Result<&str, ApiError> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "missing authorization token")
        })?;

    let (scheme, token) = header.split_once(' ').ok_or_else(|| {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid authorization token")
    })?;

    // Authentication schemes are case-insensitive.
    if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid authorization token",
        ));
    }

    Ok(token)
}