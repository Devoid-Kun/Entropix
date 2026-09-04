//! src/commands/status.rs
//!
//! Command to check current chaos index.

use crate::SCORING_WINDOW_SECS;
use crate::chaos;
use crate::config;
use crate::{Context, Error};

/// Check the current Chaos Index right now.
#[poise::command(slash_command, ephemeral, guild_only)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| Error::from("Guild only command"))?
        .get() as i64;
    let guild_config = config::get_or_create(&ctx.data().db, guild_id).await?;

    // TODO: compute the real index from the in-memory message-signal buffer
    // once main.rs wires up the message event handler. Mocked for now.
    let index = {
        let buffers = ctx.data().message_buffers.lock().await;
        if let Some(buffer) = buffers.get(&guild_id) {
            let signals: Vec<_> = buffer.iter().map(|(_, s)| *s).collect();
            let velocity = signals.len() as f32 / (SCORING_WINDOW_SECS as f32 / 60.0);
            chaos::score(&signals, velocity)
        } else {
            0
        }
    };

    let msg_template = ctx
        .data()
        .locales
        .get(&guild_config.language, "status_message");
    let msg = msg_template.replace("{index}", &index.to_string());

    ctx.say(msg).await?;
    Ok(())
}
