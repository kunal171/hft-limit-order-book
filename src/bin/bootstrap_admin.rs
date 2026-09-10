use argon2::{Argon2, password_hash::PasswordHasher};

use limit_order_book::db::connection::connect_db;
use std::{
    env,
    error::Error,
    io::{Error as IoError, ErrorKind},
};
use uuid::Uuid;

/// Create a normal error that can be returned from main.
fn invalid_input(message: &'static str) -> IoError {
    IoError::new(ErrorKind::InvalidInput, message)
}

/// Hash a password using Argon2id and an automatically generated random salt.
fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes())?
        .to_string();

    // The PHC string contains the algorithm, parameters, salt, and hash.
    Ok(password_hash)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Usage:
    // cargo run --bin bootstrap_admin -- admin@example.com "Kunal Agarawal"
    let mut args = env::args().skip(1);

    let email = args
        .next()
        .ok_or_else(|| invalid_input("missing admin email"))?
        .trim()
        .to_lowercase();

    let display_name = args.next().unwrap_or_else(|| "Administrator".to_string());

    if email.is_empty() || !email.contains('@') {
        return Err(invalid_input("invalid admin email").into());
    }

    // rpassword reads from the terminal without displaying the password.
    let password = rpassword::prompt_password("Admin password: ")?;
    let confirmation = rpassword::prompt_password("Confirm password: ")?;

    if password != confirmation {
        return Err(invalid_input("passwords do not match").into());
    }

    if password.chars().count() < 15 {
        return Err(invalid_input("password must contain at least 15 characters").into());
    }

    // Hash before opening the transaction because Argon2 is intentionally
    // expensive and should not hold a database lock while running.
    let password_hash = hash_password(&password)?;

    let db = connect_db().await?;
    let mut tx = db.begin().await?;

    // Serialize bootstrap attempts so two simultaneous commands cannot both
    // observe that no administrator exists.
    sqlx::query("LOCK TABLE users IN EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;

    let admin_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE role = 'admin')")
            .fetch_one(&mut *tx)
            .await?;

    if admin_exists {
        return Err(invalid_input("an administrator already exists").into());
    }

    let user_id = Uuid::now_v7();

    // User and credentials are inserted in one transaction. Either both
    // records are committed or neither record is stored.
    sqlx::query(
        r#"
        INSERT INTO users (id, display_name, email, status, role)
        VALUES ($1, $2, $3, 'active', 'admin')
        "#,
    )
    .bind(user_id)
    .bind(display_name.trim())
    .bind(&email)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO user_credentials (user_id, password_hash)
        VALUES ($1, $2)
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    println!("Created administrator {email} with id {user_id}");

    Ok(())
}
