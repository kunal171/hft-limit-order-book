use crate::api::{error::ApiError, state::AppState};
use axum::{
    extract::{Request, State}, http::{StatusCode, request}, middleware::Next, response::Response
};

/// Allows requests carrying the configured development admin key.
pub async fn require_admin_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    //Header Values may be absent or contain invalid text
    let supplied_key= request
        .headers()
        .get("x-admin-api-key")
        .and_then(|value| value.to_str().ok());

    if supplied_key != Some(state.admin_api_key.as_ref()) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid admin credentials"
        ));
    }
    
    Ok(next.run(request).await)
}