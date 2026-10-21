//! Translating selected text with the AI providers of the chat. Pure helpers; the app runs the call.

use crate::models::{AiProvider, Settings};

/// Languages on the wheel until the user chooses their own.
pub const DEFAULT_LANGUAGES: [&str; 8] = ["English", "Spanish", "French", "German", "Portuguese", "Italian", "Japanese", "Chinese"];

/// Most segments the wheel shows.
pub const MAX_LANGUAGES: usize = 12;

/// Longest selection sent to a model, in characters.
pub const MAX_CHARS: usize = 20_000;

/// System prompt of a translation request.
pub const SYSTEM: &str = "You are a translation engine. Output only the translation of the user's text.";

/// The wheel's languages from the setting: trimmed, without blanks or duplicates, at most 12;
/// the defaults when nothing is left.
pub fn languages(setting: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for l in setting {
        let l = l.trim();
        if !l.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(l)) {
            out.push(l.to_string());
        }
    }
    out.truncate(MAX_LANGUAGES);
    if out.is_empty() {
        out = DEFAULT_LANGUAGES.iter().map(|s| s.to_string()).collect();
    }
    out
}

/// A comma- or newline-separated list typed in Settings.
pub fn parse_list(text: &str) -> Vec<String> {
    text.split([',', '\n']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// The user turn asking for a translation; the text goes last and untouched.
pub fn prompt(text: &str, target: &str) -> String {
    format!(
        "Translate the following text to {target}. Reply with the translation only, no quotes, no notes, keep line breaks and formatting.\n\n{text}"
    )
}

/// The model's reply without the wrappers models like to add: a code fence, one pair of quotes, a label.
pub fn clean_reply(s: &str) -> String {
    let mut t = s.trim();
    if let Some(inner) = t.strip_prefix("```").and_then(|r| r.strip_suffix("```")) {
        // drop the optional language tag on the opening fence line
        t = inner.split_once('\n').map(|(_, rest)| rest).unwrap_or(inner).trim();
    }
    for label in ["Translation:", "Translated text:"] {
        if let Some(r) = t.strip_prefix(label) {
            t = r.trim();
        }
    }
    let unquoted = t.strip_prefix('"').and_then(|r| r.strip_suffix('"'));
    match unquoted {
        Some(u) if !u.contains('"') => u.trim().to_string(),
        _ => t.to_string(),
    }
}

/// Hub lines of the wheel: a word count and a one-line preview of 24 characters.
pub fn hub_text(text: &str) -> (String, String) {
    let words = text.split_whitespace().count();
    let count = format!("{words} word{}", if words == 1 { "" } else { "s" });
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let preview =
        if one_line.chars().count() > 24 { format!("{}\u{2026}", one_line.chars().take(23).collect::<String>()) } else { one_line };
    (count, preview)
}

/// The settings a translation runs with: the chat's, with the provider replaced when the user chose one.
pub fn engine_settings(s: &Settings) -> Settings {
    match s.translate_provider {
        Some(p) => Settings { ai_provider: p, ..s.clone() },
        None => s.clone(),
    }
}

/// Provider name as shown to the user.
pub fn provider_name(p: AiProvider) -> &'static str {
    match p {
        AiProvider::OpenAiCompatible => "OpenAI API",
        AiProvider::Anthropic => "Anthropic",
        AiProvider::ClaudeCode => "Claude Code",
        AiProvider::ChatGpt => "ChatGPT",
    }
}

/// Which engine translates, for the toast: "Same as chat (ChatGPT)" or the chosen provider.
pub fn engine_label(s: &Settings) -> String {
    match s.translate_provider {
        Some(p) => provider_name(p).to_string(),
        None => format!("Same as chat ({})", provider_name(s.ai_provider)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AiProvider, Settings};

    #[test]
    fn defaults_are_eight_languages() {
        assert_eq!(DEFAULT_LANGUAGES.len(), 8);
        assert_eq!(languages(&[]), DEFAULT_LANGUAGES.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn languages_trims_dedupes_and_caps_at_twelve() {
        let l = |v: &[&str]| languages(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(l(&[" Spanish ", "", "spanish", "French"]), vec!["Spanish", "French"]);
        assert_eq!(l(&["", "  "]).len(), 8, "blank list falls back to the defaults");
        let many: Vec<String> = (0..15).map(|i| format!("Lang{i}")).collect();
        assert_eq!(languages(&many).len(), 12);
    }

    #[test]
    fn parse_list_splits_on_commas_and_newlines() {
        assert_eq!(parse_list("Spanish, French\nGerman,,"), vec!["Spanish", "French", "German"]);
    }

    #[test]
    fn prompt_names_the_target_and_keeps_the_text_verbatim() {
        let p = prompt("Hello\n  world", "Spanish");
        assert!(p.contains("Spanish"));
        assert!(p.ends_with("Hello\n  world"));
    }

    #[test]
    fn clean_reply_strips_wrappers() {
        assert_eq!(clean_reply("  Hola mundo \n"), "Hola mundo");
        assert_eq!(clean_reply("\"Hola\""), "Hola");
        assert_eq!(clean_reply("```\nHola\nmundo\n```"), "Hola\nmundo");
        assert_eq!(clean_reply("Translation: Hola"), "Hola");
        assert_eq!(clean_reply("\"a\" y \"b\""), "\"a\" y \"b\"", "inner quotes stay");
    }

    #[test]
    fn hub_text_counts_words_and_previews_one_line() {
        assert_eq!(hub_text("one two  three"), ("3 words".to_string(), "one two three".to_string()));
        assert_eq!(hub_text("solo").0, "1 word");
        let (_, long) = hub_text("abcdefghijklmnopqrstuvwxyz0123456789");
        assert_eq!(long.chars().count(), 24);
        assert!(long.ends_with('\u{2026}'));
        assert_eq!(hub_text("a\n\nb").1, "a b");
    }

    #[test]
    fn engine_settings_follows_chat_unless_a_provider_is_set() {
        let chat = Settings { ai_provider: AiProvider::OpenAiCompatible, ai_model: "m".into(), ..Default::default() };
        assert_eq!(engine_settings(&chat), chat);
        let t = Settings { translate_provider: Some(AiProvider::Anthropic), ..chat.clone() };
        let e = engine_settings(&t);
        assert_eq!(e.ai_provider, AiProvider::Anthropic);
        assert_eq!(Settings { ai_provider: chat.ai_provider, ..e }, t, "only the provider changes");
    }

    #[test]
    fn engine_label_names_the_provider() {
        let mut s = Settings::default();
        assert!(engine_label(&s).starts_with("Same as chat"));
        s.translate_provider = Some(AiProvider::ClaudeCode);
        assert_eq!(engine_label(&s), "Claude Code");
        s.translate_provider = Some(AiProvider::ChatGpt);
        assert_eq!(engine_label(&s), "ChatGPT");
    }

    #[test]
    fn old_settings_files_load_with_translate_defaults() {
        let s: Settings = serde_json::from_str("{\"theme\":\"Dark\"}").unwrap();
        assert_eq!(s.translate_languages.len(), 8);
        assert_eq!(s.translate_provider, None);
    }
}
