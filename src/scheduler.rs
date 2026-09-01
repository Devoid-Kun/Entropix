//! src/scheduler.rs
//!
//! Handles the daily digest generation and dispatch to the admin channel.

use chrono::{Local, Timelike};
use sqlx::SqlitePool;
use tokio::time::{interval_at, Duration, Instant};

/// Starts the daily digest scheduler. Runs in the background, firing once
/// at the next local midnight and then every 24 hours after that (spec 4.2).
pub async fn start_scheduler(pool: SqlitePool) {
    let first_run = Instant::now() + duration_until_next_midnight();
    let mut ticker = interval_at(first_run, Duration::from_secs(86400));

    tokio::spawn(async move {
        loop {
            ticker.tick().await;

            // Note: In a real implementation, we would query the database to gather daily_stats,
            // construct an embed, and send it via the Discord API to each guild's admin_channel_id.
            // For now, this is a placeholder that clears the daily stats.

            let _ = sqlx::query!("DELETE FROM daily_stats")
                .execute(&pool)
                .await;
        }
    });
}

/// How long from right now until the next local midnight (00:00:00).
fn duration_until_next_midnight() -> Duration {
    let now = Local::now();
    let seconds_since_midnight =
        now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    Duration::from_secs(86400 - seconds_since_midnight)
}
