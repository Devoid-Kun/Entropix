//! src/db.rs
//!
//! Owns the SQLite connection pool and applies embedded migrations on
//! startup. Higher-level queries (reading/writing guild settings, logging
//! messages, running the digest) live in their own modules — this file only
//! knows how to open the database and keep its schema current.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Opens (creating if missing) the SQLite database at `path` and runs any
/// pending migrations from the `migrations/` directory.
pub async fn init_pool(path: &str) -> Result<SqlitePool, sqlx::Error> {
    // `create_if_missing` means we never have to manually `touch bot.db` —
    // the file appears automatically on first run, both locally and on
    // whatever server this ends up deployed to.
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Migrations are embedded into the binary at compile time via this
    // macro, so the deployed bot doesn't need the `migrations/` folder to
    // exist on disk at runtime — only during `cargo build`.
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
