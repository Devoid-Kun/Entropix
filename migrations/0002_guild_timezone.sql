ALTER TABLE guild_settings ADD COLUMN utc_offset_minutes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE guild_settings ADD COLUMN last_digest_at INTEGER NOT NULL DEFAULT 0;
