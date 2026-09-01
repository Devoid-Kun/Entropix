//! src/commands/settings.rs
//!
//! Commands to configure bot language and custom stage names.

use crate::config;
use crate::{Context, Error};

/// Set the bot's response language (en or ru).
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn set_language(
    ctx: Context<'_>,
    #[description = "Language (en/ru)"] language: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;

    config::set_language(&ctx.data().db, guild_id, &language).await?;

    // Reply in the NEW language, not the one that was set before this call.
    let msg = ctx.data().locales.get(&language, "set_language_success");
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
        let msg = ctx.data().locales.get(&guild_config.language, "set_names_success");
        ctx.say(msg).await?;
    } else {
        let msg = ctx.data().locales.get(&guild_config.language, "invalid_level");
        ctx.say(msg).await?;
    }

    Ok(())
}
