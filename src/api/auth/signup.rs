use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Information accepted from a new trader.
#[derive(Debug, Deserialize)]
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
