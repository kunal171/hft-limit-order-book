use crate::api::state::AppState;
use axum::{
    Json,
    extract::State,
    http::{StatusCode},
};
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
pub struct CreateUserResponse {
    pub id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    pub status: String,
    pub role: String,
}

/// JSON returned when an API operation fails.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: &'static str,
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), (StatusCode, Json<ErrorResponse>)> {
    let display_name = request.display_name.trim();
    // Reject empty names before querying PostgreSQL.
    if display_name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "display_name cannot be empty",
            }),
        ));
    }

    // Normalize email casing so A@x.com and a@x.com are treated consistently.
    let email = request
        .email
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    let user_id = Uuid::now_v7();

    let result = sqlx::query_as::<_, (Uuid, String, Option<String>, String, String)>(
        r#"
        INSERT INTO users (id, display_name, email, status, role)
        VALUES($1, $2, $3, 'active', $4)
        RETURNING id, display_name, email, status, role
        "#,
    )
    .bind(user_id)
    .bind(display_name)
    .bind(email)
    .bind(request.role.as_str())
    .fetch_one(&state.db)
    .await;

    match result {
        Ok((id, display_name, email, status, role)) => Ok((
            StatusCode::CREATED,
            Json(CreateUserResponse {
                id,
                display_name,
                email,
                status,
                role,
            }),
        )),
        Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "email already exists",
            }),
        )),
        Err(error) => {
            tracing::error!(%error, "failed to create user");

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "failed to create user",
                }),
            ))
        }
    }
}
