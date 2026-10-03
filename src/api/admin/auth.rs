use crate::api::{
    auth::{role::UserRole, sessions::AuthenticatedUser},
    error::ApiError,
};

use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};

/// Allows only authenticated administrators to continue.
pub async fn require_admin(request: Request, next: Next) -> Result<Response, ApiError> {
    // Authentication middleware inserts this trusted identity.
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "authentication required"))?;

    if user.role != UserRole::Admin {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "administrator access required",
        ));
    }

    Ok(next.run(request).await)
}
