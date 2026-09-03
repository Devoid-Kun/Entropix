//! src/scheduler.rs
//!
//! Handles daily digest generation and dispatch to each guild's admin channel.

use crate::localization::Localization;
use chrono::{DateTime, Local, Timelike, Utc};
use poise::serenity_prelude as serenity;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{Duration, Instant, interval_at};

/// Starts the daily digest scheduler. Runs in the background, firing once
/// at the next local midnight and then every 24 hours after that.
pub async fn start_scheduler(
    pool: SqlitePool,
    http: Arc<serenity::Http>,
    locales: Arc<Localization>,
) {
    let first_run = Instant::now() + duration_until_next_midnight();
    let mut ticker = interval_at(first_run, Duration::from_secs(86400));

    tokio::spawn(async move {
        loop {
            ticker.tick().await;
            if let Err(e) = run_digest_for_all_guilds(&pool, &http, &locales).await {
                tracing::error!("digest run failed: {e}");
            }
        }
    });
}

async fn run_digest_for_all_guilds(
    pool: &SqlitePool,
    http: &serenity::Http,
    locales: &Localization,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let guilds = crate::config::list_configured_guilds(pool).await?;

    for guild in guilds {
        let message_count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM daily_stats WHERE guild_id = ?",
            guild.guild_id
        )
        .fetch_one(pool)
        .await?;

        if message_count == 0 {
            continue;
        }

        let embed = build_digest_embed(
            pool,
            guild.guild_id,
            &guild.language,
            guild.current_stage as i64,
            locales,
        )
        .await?;

        let channel_id = serenity::ChannelId::new(guild.admin_channel_id as u64);
        channel_id
            .send_message(http, serenity::CreateMessage::new().embed(embed))
            .await?;
    }

    sqlx::query!("DELETE FROM daily_stats")
        .execute(pool)
        .await?;
    Ok(())
}

async fn build_digest_embed(
    pool: &SqlitePool,
    guild_id: i64,
    language: &str,
    current_stage: i64,
    locales: &Localization,
) -> Result<serenity::CreateEmbed, sqlx::Error> {
    let top_chatters = sqlx::query!(
        r#"SELECT user_id, COUNT(*) as "count!: i64" FROM daily_stats
           WHERE guild_id = ? GROUP BY user_id ORDER BY COUNT(*) DESC LIMIT 3"#,
        guild_id
    )
    .fetch_all(pool)
    .await?;

    let top_chatters_text = if top_chatters.is_empty() {
        "—".to_string()
    } else {
        top_chatters
            .iter()
            .map(|r| format!("<@{}> — {}", r.user_id, r.count))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let all_times: Vec<i64> = sqlx::query_scalar!(
        "SELECT message_time FROM daily_stats WHERE guild_id = ?",
        guild_id
    )
    .fetch_all(pool)
    .await?;
    let peak_hour = peak_activity_hour(&all_times);

    // Overall day status is based on the last known chaos stage — we only
    // keep a live snapshot, not a full intraday history, so "how the day
    // went" is approximated by "how it currently stands".
    let status_key = match current_stage {
        1 => "digest_calm",
        2 => "digest_active",
        _ => "digest_chaotic",
    };

    Ok(serenity::CreateEmbed::new()
        .title(locales.get(language, "digest_title"))
        .description(locales.get(language, status_key))
        .field(
            locales.get(language, "top_chatters"),
            top_chatters_text,
            false,
        )
        .field(
            locales.get(language, "peak_activity"),
            format!("{peak_hour}:00"),
            false,
        ))
}

/// Which local hour (0-23) had the most messages, given raw unix timestamps.
fn peak_activity_hour(timestamps: &[i64]) -> u32 {
    let mut counts = [0u32; 24];
    for &ts in timestamps {
        if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
            let hour = dt.with_timezone(&Local).hour();
            counts[hour as usize] += 1;
        }
    }
    counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(hour, _)| hour as u32)
        .unwrap_or(0)
}

fn duration_until_next_midnight() -> Duration {
    let now = Local::now();
    let seconds_since_midnight =
        now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    Duration::from_secs(86400 - seconds_since_midnight)
}
