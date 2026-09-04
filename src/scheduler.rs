//! src/scheduler.rs
//!
//! Handles daily digest generation and dispatch to each guild's admin channel.

use crate::localization::Localization;
use chrono::{DateTime, Local, Timelike, Utc};
use poise::serenity_prelude as serenity;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::Duration;

/// Starts the daily digest scheduler. Runs in the background, firing once
/// at the next local midnight and then every 24 hours after that.
pub async fn start_scheduler(
    pool: SqlitePool,
    http: Arc<serenity::Http>,
    locales: Arc<Localization>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));

    tokio::spawn(async move {
        loop {
            ticker.tick().await;
            if let Err(e) = check_and_run_digests(&pool, &http, &locales).await {
                tracing::error!("digest check failed: {e}");
            }
        }
    });
}

async fn check_and_run_digests(
    pool: &SqlitePool,
    http: &serenity::Http,
    locales: &Localization,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = chrono::Utc::now();
    let guilds = crate::config::list_configured_guilds(pool).await?;

    for guild in guilds {
        let local_now = now + chrono::Duration::minutes(guild.utc_offset_minutes as i64);
        let hours_since_last_digest = (now.timestamp() - guild.last_digest_at) as f64 / 3600.0;

        // Local midnight, and guard against firing twice for the same day
        // if this tick happens to land on minute 0 more than once.
        if local_now.hour() == 0 && local_now.minute() == 0 && hours_since_last_digest >= 23.0 {
            if let Err(e) = run_digest_for_guild(pool, http, locales, &guild, now.timestamp()).await
            {
                tracing::error!(guild_id = guild.guild_id, "digest failed: {e}");
            }
        }
    }

    Ok(())
}

async fn run_digest_for_guild(
    pool: &SqlitePool,
    http: &serenity::Http,
    locales: &Localization,
    guild: &crate::config::ConfiguredGuild,
    now: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let message_count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM daily_stats WHERE guild_id = ?",
        guild.guild_id
    )
    .fetch_one(pool)
    .await?;

    if message_count > 0 {
        let embed = build_digest_embed(
            pool,
            http,
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

    sqlx::query!("DELETE FROM daily_stats WHERE guild_id = ?", guild.guild_id)
        .execute(pool)
        .await?;
    crate::config::record_digest_sent(pool, guild.guild_id, now).await?;

    Ok(())
}

async fn build_digest_embed(
    pool: &SqlitePool,
    http: &serenity::Http,
    guild_id: i64,
    language: &str,
    current_stage: i64,
    locales: &Localization,
) -> Result<serenity::CreateEmbed, Box<dyn std::error::Error + Send + Sync>> {
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

    let active_ids = crate::config::active_user_ids_today(pool, guild_id).await?;

    // Fetching members can fail (e.g. rate limits) — degrade gracefully
    // rather than losing the whole digest over one missing field.
    let members = serenity::GuildId::new(guild_id as u64)
        .members(http, None, None)
        .await
        .unwrap_or_default();

    let lurker_mentions: Vec<String> = members
        .iter()
        .filter(|m| !m.user.bot && !active_ids.contains(&(m.user.id.get() as i64)))
        .map(|m| format!("<@{}>", m.user.id))
        .collect();

    let lurkers_text = if lurker_mentions.is_empty() {
        "—".to_string()
    } else if lurker_mentions.len() > 20 {
        format!(
            "{}\n…and {} more",
            lurker_mentions[..20].join(", "),
            lurker_mentions.len() - 20
        )
    } else {
        lurker_mentions.join(", ")
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
        )
        .field(locales.get(language, "lurkers"), lurkers_text, false))
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
