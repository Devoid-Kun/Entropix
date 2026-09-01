//! src/localization.rs
//!
//! Handles loading and retrieving localized strings based on the guild's language.

use serde_json::Value;
use std::collections::HashMap;

// Embedded into the binary at compile time — no dependency on a `locales/`
// folder existing next to the executable at runtime (important once this
// ships to a server with a different working directory).
const EN_JSON: &str = include_str!("../locales/en.json");
const RU_JSON: &str = include_str!("../locales/ru.json");

/// Localization manager to hold loaded translations.
pub struct Localization {
    strings: HashMap<String, Value>,
}

impl Localization {
    /// Parses the embedded JSON localization strings.
    pub fn load() -> Self {
        let mut strings = HashMap::new();

        if let Ok(en_json) = serde_json::from_str(EN_JSON) {
            strings.insert("en".to_string(), en_json);
        }
        if let Ok(ru_json) = serde_json::from_str(RU_JSON) {
            strings.insert("ru".to_string(), ru_json);
        }

        Self { strings }
    }

    /// Gets a localized string by key and language. Falls back to English if not found.
    pub fn get(&self, lang: &str, key: &str) -> String {
        let lang_json = self.strings.get(lang).or_else(|| self.strings.get("en"));

        if let Some(json) = lang_json {
            if let Some(val) = json.get(key) {
                if let Some(s) = val.as_str() {
                    return s.to_string();
                }
            }
        }

        key.to_string()
    }
}
