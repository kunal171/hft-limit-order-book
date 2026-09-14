use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Account data that may be returned to an authenticated client.
#[derive(Debug, Serialize, FromRow)]
pub struct AccountResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
