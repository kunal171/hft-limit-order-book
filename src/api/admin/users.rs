use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Allowed roles in the users table.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Trader,
    System,
}

impl UserRole {
    /// Converts the Rust enum into the value stored in PostgreSQL.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Trader => "trader",
            Self::System => "system",
        }
    }
}

/// JSON accepted by POST /admin/users.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub display_name: String,
    pub email: Option<String>,
    pub role: UserRole,
}

/// JSON returned after creating a user.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    pub status: String,
    pub role: String,
}
