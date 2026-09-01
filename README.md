<p align="center">
  <img src="assets/logo.png" style="max-width: 100%; height: auto;" alt="Entropix logo">
</p>

<h1 align="center">Entropix</h1>
<p align="center">Real-time chaos tracking for your Discord server 🌀</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-2024-orange.svg">
  <img alt="Status" src="https://img.shields.io/badge/status-in%20development-yellow.svg">
</p>

---

**Entropix** watches a text channel, computes a live "chaos index" from message
velocity, punctuation spam, and caps usage, and renames the channel to reflect
the current mood — then wraps it up with a daily digest of who talked the
most, when things peaked, and who stayed silent.

## 📖 Table of Contents

- [How it works](#-how-it-works)
- [Features](#-features)
- [Tech stack](#-tech-stack)
- [Getting started](#-getting-started)
- [Commands](#-commands)
- [Database schema](#-database-schema)
- [Project status](#-project-status)
- [Contributing](#-contributing)
- [License](#-license)

## ⚙️ How it works

Every message in the target channel is scored on three signals — caps ratio,
punctuation-spam density, and messages-per-minute velocity — and combined
into a 0–100 chaos index. Crossing a stage boundary (calm / active /
chaotic) triggers a channel rename, throttled by a 5-minute cooldown to stay
well under Discord's own rate limit of two renames per ten minutes.

<details>
<summary>Chaos stages (default thresholds)</summary>

| Stage | Range | Default name |
|---|---|---|
| 1 — Calm | 0–30 | `#штиль` |
| 2 — Active | 31–70 | `#активное-общение` |
| 3 — Chaotic | 71–100 | `#сущий-кошмар` |

Names are fully customizable per server via `/set_names`.
</details>

## ✨ Features

- 🌀 Live chaos index, recalculated as messages arrive
- 🔁 Automatic channel renaming with built-in cooldown protection
- 📊 Daily digest — top chatters, peak activity hour, and a list of lurkers
- 🌍 Localization — English and Russian out of the box
- 🎛️ Per-server configuration, fully ephemeral slash commands (no chat clutter)

## 🧱 Tech stack

| Layer | Choice |
|---|---|
| Language | Rust (2024 edition) |
| Discord API | [`serenity`](https://github.com/serenity-rs/serenity) + [`poise`](https://github.com/serenity-rs/poise) |
| Async runtime | `tokio` |
| Database | SQLite via `sqlx` |
| Config / locales | `serde`, `serde_json` |

## 🚀 Getting started

```bash
git clone https://github.com/Devoid-Kun/Entropix.git
cd Entropix

cp .env.example .env
# then edit .env and paste your DISCORD_TOKEN

cargo install sqlx-cli --no-default-features --features sqlite
export DATABASE_URL="sqlite://bot.db"
sqlx database create
sqlx migrate run

cargo run
```

Run the test suite and linter before pushing:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

## 🎮 Commands

| Command | Description |
|---|---|
| `/setup_target <channel>` | Set the channel Entropix monitors and renames |
| `/setup_admin <channel>` | Set the channel that receives the daily digest |
| `/set_language <en\|ru>` | Switch the bot's response language |
| `/set_names <level> <name>` | Customize the channel name for a chaos stage (1–3) |
| `/status` | Check the current chaos index |

All commands reply **ephemerally** — only the person who ran them sees the response.

## 🗄️ Database schema

<details>
<summary>guild_settings</summary>

| Column | Type | Notes |
|---|---|---|
| `guild_id` | INTEGER PK | Discord snowflake |
| `target_channel_id` | INTEGER | Monitored channel |
| `admin_channel_id` | INTEGER | Digest destination |
| `language` | TEXT | `en` / `ru` |
| `custom_names_json` | TEXT | Per-stage name overrides |
| `current_stage` | INTEGER | Last known chaos stage |
| `last_renamed_at` | INTEGER | Unix timestamp, backs the rename cooldown |
</details>

<details>
<summary>daily_stats</summary>

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Autoincrement |
| `guild_id` | INTEGER | FK → guild_settings |
| `user_id` | INTEGER | Message author |
| `message_time` | INTEGER | Unix timestamp |

Purged automatically after each daily digest — Entropix never stores message
content, only aggregate activity metadata.
</details>

## 🛣️ Project status

- [x] Database schema & connection pool
- [x] Guild configuration layer
- [x] Localization (en/ru)
- [x] Chaos index algorithm + unit tests
- [x] Slash command scaffolding
- [ ] Wire commands + data into `main.rs`
- [ ] In-memory message buffer for live scoring
- [ ] Daily digest embed generation
- [ ] Deployment

## 🤝 Contributing

This project is built collaboratively — see commit history for
`Co-authored-by` credits. Pull requests target the `dev` branch.

## 📄 License

Licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
