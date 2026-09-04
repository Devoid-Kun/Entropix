//! src/commands/settings.rs
//!
//! Commands to configure bot language and custom stage names.

use crate::config;
use crate::{Context, Error};

/// Supported languages for the bot's responses.
const SUPPORTED_LANGUAGES: &[&str] = &["en", "ru"];

/// Set the bot's response language (en or ru).
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn set_language(
    ctx: Context<'_>,
    #[description = "Language (en/ru)"] language: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;
    let normalized = language.to_lowercase();
    if !SUPPORTED_LANGUAGES.contains(&&normalized.as_str()) {
        tracing::warn!(guild_id, attempted = %language, "Rejected unsupported language");
        ctx.say(format!(
            "Unsupported language: `{language}`. Supportet: en, ru",
        ))
        .await?;
        return Ok(());
    }

    config::set_language(&ctx.data().db, guild_id, &normalized).await?;

    // Reply in the NEW language, not the one that was set before this call.
    let msg = ctx.data().locales.get(&normalized, "set_language_success");
    ctx.say(msg).await?;
    Ok(())
}

/// Set a custom channel name for a specific chaos level (1-3).
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn set_names(
    ctx: Context<'_>,
    #[description = "Chaos level (1, 2, or 3)"] level: u8,
    #[description = "Custom name"] name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;
    let guild_config = config::get_or_create(&ctx.data().db, guild_id).await?;

    if (1..=3).contains(&level) {
        config::set_custom_name(&ctx.data().db, guild_id, level, &name).await?;
        let msg = ctx
            .data()
            .locales
            .get(&guild_config.language, "set_names_success");
        ctx.say(msg).await?;
    } else {
        let msg = ctx
            .data()
            .locales
            .get(&guild_config.language, "invalid_level");
        ctx.say(msg).await?;
    }

    Ok(())
}

/// Set the UTC offset for this server's daily digest (e.g. 9 for Tokyo, -5 for New York).
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn set_timezone(
    ctx: Context<'_>,
    #[description = "UTC offset in hours, e.g. 9 or -5"] offset_hours: i32,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;

    if !(-12..=14).contains(&offset_hours) {
        ctx.say("Invalid offset. Must be between -12 and 14.")
            .await?;
        return Ok(());
    }

    config::set_utc_offset(&ctx.data().db, guild_id, offset_hours * 60).await?;

    let guild_config = config::get_or_create(&ctx.data().db, guild_id).await?;
    let msg = ctx
        .data()
        .locales
        .get(&guild_config.language, "set_timezone_success");
    ctx.say(msg).await?;
    Ok(())
}
