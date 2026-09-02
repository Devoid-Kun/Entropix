//! src/config.rs
//!
//! Reads and writes per-guild settings (target/admin channels, language,
//! custom chaos-stage names, rename cooldown state). This is the only module
//! that touches the `guild_settings` table directly — commands and the
//! scheduler go through the functions here rather than writing raw SQL
//! themselves, so the schema only needs to be known in one place.

use sqlx::SqlitePool;
use std::collections::HashMap;

/// A guild's full configuration, as stored in `guild_settings`.
#[derive(Debug, Clone)]
pub struct GuildConfig {
    pub guild_id: i64,
    pub target_channel_id: Option<i64>,
    pub admin_channel_id: Option<i64>,
    pub language: String,
    pub custom_names: HashMap<u8, String>, // chaos stage (1..3) -> channel name
    pub current_stage: u8,
    pub last_renamed_at: i64, // unix timestamp
}

impl Default for GuildConfig {
    fn default() -> Self {
        Self {
            guild_id: 0,
            target_channel_id: None,
            admin_channel_id: None,
            language: "en".to_string(),
            custom_names: HashMap::new(),
            current_stage: 1,
            last_renamed_at: 0,
        }
    }
}

// Internal row shape matching the SQLite table exactly, so sqlx's
// query_as! can map columns without guessing types (custom_names_json
// stays a raw String here; GuildConfig::custom_names is the parsed form).
#[derive(sqlx::FromRow)]
struct GuildRow {
    guild_id: i64,
    target_channel_id: Option<i64>,
    admin_channel_id: Option<i64>,
    language: String,
    custom_names_json: Option<String>,
    current_stage: i64,
    last_renamed_at: i64,
}

impl From<GuildRow> for GuildConfig {
    fn from(row: GuildRow) -> Self {
        let custom_names = row
            .custom_names_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<HashMap<u8, String>>(s).ok())
            .unwrap_or_default();

        Self {
            guild_id: row.guild_id,
            target_channel_id: row.target_channel_id,
            admin_channel_id: row.admin_channel_id,
            language: row.language,
            custom_names,
            current_stage: row.current_stage as u8,
            last_renamed_at: row.last_renamed_at,
        }
    }
}

/// Fetches a guild's config, creating a default row on first contact.
/// Every command handler starts by calling this — it guarantees a row
/// always exists, so nothing downstream has to special-case "unconfigured".
pub async fn get_or_create(pool: &SqlitePool, guild_id: i64) -> Result<GuildConfig, sqlx::Error> {
    if let Some(row) = sqlx::query_as!(
        GuildRow,
        r#"SELECT guild_id, target_channel_id, admin_channel_id, language,
                  custom_names_json, current_stage, last_renamed_at
           FROM guild_settings WHERE guild_id = ?"#,
        guild_id
    )
    .fetch_optional(pool)
    .await?
    {
        return Ok(row.into());
    }

    sqlx::query!("INSERT INTO guild_settings (guild_id) VALUES (?)", guild_id)
        .execute(pool)
        .await?;

    Ok(GuildConfig {
        guild_id,
        ..Default::default()
    })
}

/// Sets the channel the bot monitors and renames. Used by /setup_target.
pub async fn set_target_channel(
    pool: &SqlitePool,
    guild_id: i64,
    channel_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE guild_settings SET target_channel_id = ? WHERE guild_id = ?",
        channel_id,
        guild_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Sets the channel that receives the daily digest. Used by /setup_admin.
pub async fn set_admin_channel(
    pool: &SqlitePool,
    guild_id: i64,
    channel_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE guild_settings SET admin_channel_id = ? WHERE guild_id = ?",
        channel_id,
        guild_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Switches the bot's response language for this guild. Used by /set_language.
pub async fn set_language(
    pool: &SqlitePool,
    guild_id: i64,
    language: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE guild_settings SET language = ? WHERE guild_id = ?",
        language,
        guild_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Sets a custom channel name for one chaos stage (1, 2, or 3). Used by
/// /set_names. Reads the existing JSON blob, patches one key, writes it back —
/// simpler than a separate table for three rarely-changed values.
pub async fn set_custom_name(
    pool: &SqlitePool,
    guild_id: i64,
    stage: u8,
    name: &str,
) -> Result<(), sqlx::Error> {
    let config = get_or_create(pool, guild_id).await?;
    let mut names = config.custom_names;
    names.insert(stage, name.to_string());
    let json = serde_json::to_string(&names).unwrap_or_default();

    sqlx::query!(
        "UPDATE guild_settings SET custom_names_json = ? WHERE guild_id = ?",
        json,
        guild_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Records that a rename just happened, at the given stage. `chaos.rs`
/// reads `current_stage`/`last_renamed_at` back to enforce the 5-minute
/// cooldown before allowing another rename.
pub async fn record_rename(
    pool: &SqlitePool,
    guild_id: i64,
    stage: u8,
    timestamp: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE guild_settings SET current_stage = ?, last_renamed_at = ? WHERE guild_id = ?",
        stage,
        timestamp,
        guild_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// One row of guild data as needed by the daily digest — a narrower view
/// than `GuildConfig`, since the scheduler doesn't need custom_names or
/// target_channel_id, only guilds that are fully set up to receive a digest.
pub struct ConfiguredGuild {
    pub guild_id: i64,
    pub admin_channel_id: i64,
    pub language: String,
    pub current_stage: u8,
}

/// Every guild that has both a target and an admin channel configured —
/// i.e. guilds ready to receive a daily digest. Used by `scheduler.rs`.
pub async fn list_configured_guilds(
    pool: &SqlitePool,
) -> Result<Vec<ConfiguredGuild>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT guild_id, admin_channel_id, language, current_stage
           FROM guild_settings
           WHERE target_channel_id IS NOT NULL AND admin_channel_id IS NOT NULL"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ConfiguredGuild {
            guild_id: r.guild_id,
            admin_channel_id: r.admin_channel_id.unwrap(), // guaranteed non-null by the WHERE clause
            language: r.language,
            current_stage: r.current_stage as u8,
        })
        .collect())
}
