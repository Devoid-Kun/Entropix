//! src/main.rs
//!
//! Wires every module together: opens the database, loads locale strings,
//! registers slash commands, starts the daily-digest scheduler, and handles
//! incoming messages — scoring them, updating the live chaos buffer, and
//! renaming the target channel when a stage boundary is crossed.

use poise::serenity_prelude as serenity;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

mod chaos;
mod commands;
mod config;
mod db;
mod localization;
mod scheduler;

use localization::Localization;

/// Shared bot state, reachable from every command and from the event
/// handler via `ctx.data()`.
pub struct Data {
    pub db: sqlx::SqlitePool,
    pub locales: Arc<Localization>,
    /// guild_id -> recent (unix_timestamp, MessageSignal) pairs, used to
    /// compute the live chaos score. Entries older than `SCORING_WINDOW_SECS`
    /// are pruned every time a new message arrives.
    pub message_buffers: Mutex<HashMap<i64, VecDeque<(i64, chaos::MessageSignal)>>>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// How far back we look when computing the live chaos score (
/// suggests 3-5 minutes; 4 splits the difference).
pub const SCORING_WINDOW_SECS: i64 = 4 * 60;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN in .env");
    let pool = db::init_pool("bot.db")
        .await
        .expect("failed to open bot.db");

    let intents = serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MEMBERS
        | serenity::GatewayIntents::GUILDS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::setup_target(),
                commands::setup_admin(),
                commands::set_language(),
                commands::set_names(),
                commands::status(),
                commands::set_timezone(),
            ],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let pool = pool.clone();
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let locales = Arc::new(Localization::load());
                scheduler::start_scheduler(pool.clone(), ctx.http.clone(), locales.clone()).await;

                Ok(Data {
                    db: pool,
                    locales,
                    message_buffers: Mutex::new(HashMap::new()),
                })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("failed to build client");

    client.start().await.expect("client error");
}

/// Resolves the channel name to use for a given stage: a guild's custom
/// override if one was set via /set_names, otherwise the built-in default
fn resolve_channel_name(stage: chaos::Stage, guild_config: &config::GuildConfig) -> String {
    guild_config
        .custom_names
        .get(&stage.as_u8())
        .cloned()
        .unwrap_or_else(|| stage.default_channel_name().to_string())
}

/// Raw Discord gateway event handler. Only "Message" is used: it logs
/// activity for the daily digest, feeds the live scoring buffer, and
/// triggers a channel rename when warranted.
async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    let serenity::FullEvent::Message { new_message } = event else {
        return Ok(());
    };

    if new_message.author.bot {
        return Ok(()); // never react to the bot's own messages, or other bots
    }
    let Some(guild_id) = new_message.guild_id else {
        return Ok(()); // ignore DMs
    };

    let guild_config = config::get_or_create(&data.db, guild_id.get() as i64).await?;

    let Some(target_channel_id) = guild_config.target_channel_id else {
        return Ok(()); // /setup_target hasn't been run yet for this guild
    };
    if new_message.channel_id.get() as i64 != target_channel_id {
        return Ok(()); // message wasn't in the monitored channel
    }

    let now = chrono::Utc::now().timestamp();

    // Log for the daily digest (spec 4.2) — metadata only, message content
    // is never persisted.
    let user_id = new_message.author.id.get() as i64;
    sqlx::query!(
        "INSERT INTO daily_stats (guild_id, user_id, message_time) VALUES (?, ?, ?)",
        guild_config.guild_id,
        user_id,
        now
    )
    .execute(&data.db)
    .await?;

    // Update the live scoring buffer for this guild.
    let signal = chaos::MessageSignal::from_text(&new_message.content);
    let signals = {
        let mut buffers = data.message_buffers.lock().await;
        let buffer = buffers.entry(guild_config.guild_id).or_default();

        buffer.push_back((now, signal));
        while let Some((ts, _)) = buffer.front() {
            if now - ts > SCORING_WINDOW_SECS {
                buffer.pop_front();
            } else {
                break;
            }
        }

        buffer.iter().map(|(_, s)| *s).collect::<Vec<_>>()
        // lock released here, before the .await calls below
    };

    let velocity = signals.len() as f32 / (SCORING_WINDOW_SECS as f32 / 60.0);
    let score = chaos::score(&signals, velocity);

    let stored_stage = chaos::Stage::from_stage_number(guild_config.current_stage);
    let seconds_since_last_rename = now - guild_config.last_renamed_at;

    if chaos::should_rename(score, stored_stage, seconds_since_last_rename) {
        let new_stage = chaos::Stage::from_score(score);
        let new_name = resolve_channel_name(new_stage, &guild_config);

        let channel_id = serenity::ChannelId::new(target_channel_id as u64);
        channel_id
            .edit(&ctx.http, serenity::EditChannel::new().name(&new_name)) //  verify this line compiles
            .await?;

        config::record_rename(&data.db, guild_config.guild_id, new_stage.as_u8(), now).await?;
    }

    Ok(())
}
