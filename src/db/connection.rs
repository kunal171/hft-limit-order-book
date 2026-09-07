use dotenvy::dotenv;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

pub async fn connect_db() -> Result<Pool<Postgres>, sqlx::Error> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    Ok(pool)
}