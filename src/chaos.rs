//! src/chaos.rs
//!
//! Pure scoring logic for Entropix's "chaos index": a 0-100 score derived
//! from how a channel's recent messages look and how fast they're arriving.
//! Nothing in this file touches the database or the Discord API directly —
//! callers (commands, the message-event handler) supply plain numbers and
//! get plain numbers back, which keeps the algorithm trivially unit-testable
//! and free to tune without touching any I/O code.

use std::time::Duration;

/// A chaos score is always clamped to this range.
pub const MIN_SCORE: u8 = 0;
pub const MAX_SCORE: u8 = 100;

/// Chaos stage boundaries, per. Only the *names* attached to each
/// stage are user-configurable (see `config::GuildConfig::custom_names`) —
/// the numeric ranges themselves are fixed.
pub const STAGE_1_MAX: u8 = 30; // 0-30   -> calm
pub const STAGE_2_MAX: u8 = 70; // 31-70  -> active
// 71-100 -> chaotic

/// Minimum time between two channel renames. Discord itself
/// allows two renames per 10 minutes; 5 minutes keeps a comfortable margin
/// so a burst of activity near a stage boundary can't trip the hard limit.
pub const RENAME_COOLDOWN: Duration = Duration::from_secs(5 * 60);

/// Which of the three named stages a 0-100 score falls into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Calm = 1,
    Active = 2,
    Chaotic = 3,
}

impl Stage {
    pub fn from_score(score: u8) -> Self {
        match score {
            0..=STAGE_1_MAX => Stage::Calm,
            s if s <= STAGE_2_MAX => Stage::Active,
            _ => Stage::Chaotic,
        }
    }
    /// Converts a raw stage number (1/2/3), as stored in `guild_settings.current_stage`,
    /// back into a `Stage`. Anything outside 1..=3 falls back to Chaotic defensively.
    pub fn from_stage_number(n: u8) -> Self {
        match n {
            1 => Stage::Calm,
            2 => Stage::Active,
            _ => Stage::Chaotic,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Default channel name for this stage. Edit these three
    /// lines to change the default names — no leading "#", Discord adds
    /// that in the UI itself.
    pub fn default_channel_name(self) -> &'static str {
        match self {
            Stage::Calm => "🍵-calm",
            Stage::Active => "⚡-active-discussion",
            Stage::Chaotic => "💥-absolute-chaos",
        }
    }
}

/// Per-message signal, extracted once per incoming message and fed into
/// `score`. Kept separate from the raw `&str` so the caller (the message
/// event handler in main.rs) only has to look at message content once.
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageSignal {
    pub caps_ratio: f32,       // 0.0..=1.0, share of uppercase letters among letters
    pub punctuation_spam: f32, // 0.0..=1.0, normalized density of ! ? ) spam
}

impl MessageSignal {
    /// Extracts caps ratio and punctuation-spam density from raw message text.
    pub fn from_text(text: &str) -> Self {
        Self {
            caps_ratio: caps_ratio(text),
            punctuation_spam: punctuation_spam(text),
        }
    }
}

/// Share of alphabetic characters that are uppercase. Non-letters (spaces,
/// digits, emoji, punctuation) don't count towards the denominator, so caps
/// and punctuation are tracked as independent signals on purpose — one
/// can't quietly compensate for the other.
fn caps_ratio(text: &str) -> f32 {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return 0.0;
    }
    let upper = letters.iter().filter(|c| c.is_uppercase()).count();
    upper as f32 / letters.len() as f32
}

/// Density of "spam" punctuation — repeated !, ?, or ) — normalized against
/// message length so a long calm message with one stray "?" doesn't score
/// the same as "?!?!?!". Capped at 1.0.
fn punctuation_spam(text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let spam_chars = text
        .chars()
        .filter(|c| matches!(c, '!' | '?' | ')'))
        .count();
    (spam_chars as f32 / text.chars().count() as f32 * 3.0).min(1.0)
}

/// Combines a batch of recent message signals and how fast they arrived into a
/// single 0-100 chaos score.
///
/// `signals` – every message seen in the current scoring window (the last 3-5
/// minutes; the exact window is the caller's choice, enforced wherever
/// messages are read out, not here).
/// `messages_per_minute` – velocity over that same window, already computed
/// by the caller so this function stays free of any notion of "now".
///
/// Weights: velocity 50%, caps 30%, punctuation 20% – sum to 100 so the raw
/// score lands in 0-100 without extra scaling.
///
/// Velocity dominates because sustained activity from multiple real users is
/// hard to fake with a single message. Caps outweigh punctuation because it's
/// a more consistent signal of excitement/frustration; punctuation spam is
/// easier to trigger accidentally. Velocity saturates at 20 msg/min — past
/// that, more messages don't make it feel "more chaotic" to a human reader,
/// so extra activity stops adding to the score.

pub fn score(signals: &[MessageSignal], messages_per_minute: f32) -> u8 {
    if signals.is_empty() {
        return MIN_SCORE;
    }

    let avg_caps: f32 = signals.iter().map(|s| s.caps_ratio).sum::<f32>() / signals.len() as f32;
    let avg_punct: f32 =
        signals.iter().map(|s| s.punctuation_spam).sum::<f32>() / signals.len() as f32;

    // Velocity saturates at 20 messages/minute — past that point the channel
    // is unambiguously chaotic regardless of what the text looks like.
    let velocity_component = (messages_per_minute / 20.0).min(1.0);

    // Weights are deliberately unequal: text signals are cheap to fake with
    // a single message, so velocity — which requires sustained activity
    // from real people — carries the most weight.
    let raw = avg_caps * 30.0 + avg_punct * 20.0 + velocity_component * 50.0;

    raw.round().clamp(MIN_SCORE as f32, MAX_SCORE as f32) as u8
}

/// Whether the bot should actually rename the channel right now.
/// only rename if the cooldown has elapsed *and* the stage actually
/// changed — a score wobbling near a boundary shouldn't rename every tick.
pub fn should_rename(new_score: u8, current_stage: Stage, seconds_since_last_rename: i64) -> bool {
    let new_stage = Stage::from_score(new_score);
    let cooldown_elapsed = seconds_since_last_rename >= RENAME_COOLDOWN.as_secs() as i64;
    cooldown_elapsed && new_stage != current_stage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_channel_is_calm() {
        assert_eq!(score(&[], 0.0), 0);
        assert_eq!(Stage::from_score(0), Stage::Calm);
    }

    #[test]
    fn full_caps_and_high_velocity_is_chaotic() {
        let signals = vec![MessageSignal::from_text("ЭТО ПРОСТО КРИК!!!"); 5];
        let s = score(&signals, 25.0);
        assert!(s > 70, "expected chaotic score, got {s}");
        assert_eq!(Stage::from_score(s), Stage::Chaotic);
    }

    #[test]
    fn calm_lowercase_low_velocity_stays_calm() {
        let signals = vec![MessageSignal::from_text("hey, how's it going today")];
        let s = score(&signals, 1.0);
        assert!(s <= 30, "expected calm score, got {s}");
    }

    #[test]
    fn cooldown_blocks_rename_even_on_stage_change() {
        assert!(!should_rename(90, Stage::Calm, 60)); // only 1 minute passed
        assert!(should_rename(90, Stage::Calm, 400)); // 5m40s passed, stage changed
    }

    #[test]
    fn same_stage_does_not_trigger_rename() {
        assert!(!should_rename(20, Stage::Calm, 1000)); // cooldown elapsed, stage unchanged
    }
}
