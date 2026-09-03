-- Migration 0001: initial schema for Entropix

-- guild_settings: one row per Discord server the bot is configured in
CREATE TABLE IF NOT EXISTS guild_settings (
    guild_id            INTEGER PRIMARY KEY,          -- Discord snowflake, fits in i64
    target_channel_id   INTEGER,                       -- channel the bot watches and renames
    admin_channel_id    INTEGER,                       -- channel that receives the daily digest
    language             TEXT NOT NULL DEFAULT 'en',    -- 'en' or 'ru'
    custom_names_json    TEXT,                          -- JSON-encoded per-stage channel name overrides
    current_stage         INTEGER NOT NULL DEFAULT 1,    -- last known chaos stage (1..3), used to detect transitions
    last_renamed_at        INTEGER NOT NULL DEFAULT 0     -- unix timestamp of the last rename, backs the 5-minute cooldown
);

-- daily_stats: one row per message seen in the target channel, purged after each daily digest
CREATE TABLE IF NOT EXISTS daily_stats (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id      INTEGER NOT NULL REFERENCES guild_settings(guild_id) ON DELETE CASCADE,
    user_id       INTEGER NOT NULL,
    message_time  INTEGER NOT NULL              -- unix timestamp, used to compute peak activity hour
);

-- speeds up the digest query (all of a guild's messages for "today") and the cooldown check
CREATE INDEX IF NOT EXISTS idx_daily_stats_guild_time ON daily_stats (guild_id, message_time);
