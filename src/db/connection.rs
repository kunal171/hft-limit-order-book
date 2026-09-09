use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn connect_db() -> Result<Pool<Postgres>, sqlx::Error> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await?;

    Ok(pool)
}
