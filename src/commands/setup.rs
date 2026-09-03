//! src/commands/setup.rs
//!
//! Setup commands for admin and target channels.

use crate::config;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// Set the channel for the bot to monitor.
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn setup_target(
    ctx: Context<'_>,
    #[description = "The channel to monitor"] channel: serenity::Channel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;
    let channel_id = channel.id().get() as i64;

    config::set_target_channel(&ctx.data().db, guild_id, channel_id).await?;

    let guild_config = config::get_or_create(&ctx.data().db, guild_id).await?;
    let msg = ctx.data().locales.get(&guild_config.language, "setup_target_success");
    ctx.say(msg).await?;
    Ok(())
}

/// Set the channel where the bot will send the nightly digest.
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn setup_admin(
    ctx: Context<'_>,
    #[description = "The channel for daily digests"] channel: serenity::Channel,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap().get() as i64;
    let channel_id = channel.id().get() as i64;

    config::set_admin_channel(&ctx.data().db, guild_id, channel_id).await?;

    let guild_config = config::get_or_create(&ctx.data().db, guild_id).await?;
    let msg = ctx.data().locales.get(&guild_config.language, "setup_admin_success");
    ctx.say(msg).await?;
    Ok(())
}
