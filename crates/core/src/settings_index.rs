//! Static index of every row of the Settings window, for the Settings search field.
//!
//! `ENTRIES` lists the rows by tab (0 General, 1 Clipboard, 2 Snippets, 3 AI, 4 Translate,
//! 5 Scripts, 6 Advanced) in the order they are drawn. The title is the exact title of the row;
//! a test in the app walks every tab and fails when a drawn row is missing here or an entry is
//! never drawn.

/// One Settings row.
pub struct Entry {
    /// Tab the row lives on (0..=6).
    pub tab: usize,
    /// Exact title of the row.
    pub title: &'static str,
    /// Extra words that find the row.
    pub keywords: &'static [&'static str],
}

const fn e(tab: usize, title: &'static str, keywords: &'static [&'static str]) -> Entry {
    Entry { tab, title, keywords }
}

/// Most results `search` returns.
pub const MAX_RESULTS: usize = 8;

pub const ENTRIES: &[Entry] = &[
    // 0 General
    e(0, "Launcher hotkey", &["shortcut", "keybinding", "alt space", "record", "open", "global"]),
    e(0, "Appearance", &["theme", "dark", "light", "system", "mode", "color", "colour"]),
    e(0, "Compact mode", &["minimal", "search bar", "small", "collapse"]),
    e(0, "Launch at login", &["startup", "start", "boot", "login item", "autostart"]),
    e(0, "Drop wheel", &["drag", "files", "wheel", "convert", "radial"]),
    e(0, "Convert wheel key", &["modifier", "shift", "drag", "files", "conversion"]),
    e(0, "Tools wheel key", &["modifier", "control", "drag", "compress", "crop", "redact"]),
    e(0, "Actions wheel key", &["modifier", "option", "drag", "copy", "move", "airdrop"]),
    e(0, "Enter on Finder selection", &["return", "rename", "finder", "convert", "accessibility"]),
    e(0, "Menu bar icon", &["tray", "status bar", "restart", "mouse"]),
    e(0, "Check for updates automatically", &["update", "github", "upgrade", "version", "privacy", "network"]),
    e(0, "Install updates", &["update", "automatic", "ask first", "release notes", "upgrade"]),
    e(0, "Version", &["update", "check now", "release", "about", "install", "upgrade"]),
    e(0, "Sensitivity", &["fuzzy", "matching", "search", "low", "high", "typo"]),
    e(0, "Include files", &["spotlight", "file search", "results", "root search"]),
    e(0, "Web search fallback", &["url", "google", "browser", "query", "search the web", "engine"]),
    // 1 Clipboard
    e(1, "Record history", &["copy", "paste", "history", "password", "privacy", "enable"]),
    e(1, "Keep history for", &["retention", "days", "delete", "expire", "clean", "pinned"]),
    // 2 Snippets
    e(2, "Expand while typing", &["expansion", "keyword", "text", "abbreviation", "accessibility", "autocomplete"]),
    // 3 AI
    e(3, "Provider", &["openai", "anthropic", "claude", "chatgpt", "ollama", "engine", "chat"]),
    e(3, "Preset", &["ollama", "lm studio", "openai", "server", "local"]),
    e(3, "Server", &["url", "endpoint", "ollama", "base", "address", "host", "v1"]),
    e(3, "Model", &["llm", "gpt", "claude", "name", "custom", "refresh"]),
    e(3, "API key", &["token", "secret", "keychain", "password", "credential", "sk"]),
    e(3, "Account", &["sign in", "login", "chatgpt", "session", "sign out", "oauth"]),
    e(3, "Command-line tool", &["claude code", "cli", "install", "terminal", "binary"]),
    // 4 Translate
    e(4, "Translate wheel key", &["modifier", "control", "drag", "text", "wheel"]),
    e(4, "Languages", &["translate", "locale", "language", "target", "list", "spanish", "english"]),
    e(4, "After translating", &["output", "replace", "paste", "copy", "window", "auto", "javascript"]),
    e(4, "Provider", &["engine", "translation", "same as chat", "openai", "anthropic", "claude", "chatgpt"]),
    e(4, "Model", &["llm", "translation", "default", "custom", "gpt"]),
    // 5 Scripts
    e(5, "Default folder", &["scripts", "directory", "path", "raycast", "commands"]),
    e(5, "Add directory", &["scripts", "folder", "path", "extra", "home"]),
    // 6 Advanced
    e(6, "Accessibility", &["permission", "privacy", "grant", "window management", "security", "access"]),
    e(6, "1 rem", &["px", "pixels", "css", "calculator", "em", "units"]),
    e(6, "Window gap", &["spacing", "margin", "snap", "tiling", "edges", "pt"]),
    e(6, "Data folder", &["storage", "files", "open", "settings.json", "backup", "location", "path"]),
];

/// Lower-cases and strips the basic Latin accents, one char to one char (so a match position in
/// the result is a char position in the original).
pub fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| {
            let c = c.to_lowercase().next().unwrap_or(c);
            match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                'ç' => 'c',
                'è' | 'é' | 'ê' | 'ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' => 'i',
                'ñ' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
                'ù' | 'ú' | 'û' | 'ü' => 'u',
                'ý' | 'ÿ' => 'y',
                other => other,
            }
        })
        .collect()
}

/// Rank of `entry` for the normalized query `q` (terms are its whitespace-separated words):
/// 0 title starts with q, 1 title contains q, 2 a keyword starts with q, 3 every term matches the
/// title or a keyword. `None` when some term matches nothing.
fn rank(entry: &Entry, q: &str, terms: &[&str]) -> Option<u8> {
    let title = normalize(entry.title);
    let keywords: Vec<String> = entry.keywords.iter().map(|k| normalize(k)).collect();
    if !terms.iter().all(|t| title.contains(t) || keywords.iter().any(|k| k.contains(t))) {
        return None;
    }
    Some(if title.starts_with(q) {
        0
    } else if title.contains(q) {
        1
    } else if keywords.iter().any(|k| k.starts_with(q)) {
        2
    } else {
        3
    })
}

/// Entries matching `query`, best first, at most `MAX_RESULTS`. Case and basic accents are
/// ignored; every whitespace-separated word must match the title or a keyword. An empty query
/// matches nothing.
pub fn search(query: &str) -> Vec<&'static Entry> {
    let terms_owned = normalize(query);
    let terms: Vec<&str> = terms_owned.split_whitespace().collect();
    if terms.is_empty() {
        return vec![];
    }
    let q = terms.join(" ");
    let mut hits: Vec<(u8, &'static Entry)> = ENTRIES.iter().filter_map(|en| rank(en, &q, &terms).map(|r| (r, en))).collect();
    // stable: inside a rank the entries keep their order (tab, then position in the tab)
    hits.sort_by_key(|(r, en)| (*r, en.tab));
    hits.into_iter().take(MAX_RESULTS).map(|(_, en)| en).collect()
}

/// Char range `(start, len)` of the first query word inside `title` (for highlighting), or `None`.
pub fn match_range(title: &str, query: &str) -> Option<(usize, usize)> {
    let t = normalize(title);
    let qn = normalize(query);
    let q = qn.split_whitespace().next()?;
    let byte = t.find(q)?;
    Some((t[..byte].chars().count(), q.chars().count()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(q: &str) -> Vec<(usize, &'static str)> {
        search(q).iter().map(|e| (e.tab, e.title)).collect()
    }

    #[test]
    fn empty_and_blank_queries_match_nothing() {
        assert!(search("").is_empty());
        assert!(search("   \t ").is_empty());
    }

    #[test]
    fn title_prefix_beats_title_contains_beats_keywords() {
        let r = titles("model");
        assert_eq!(r[..2], [(3, "Model"), (4, "Model")], "title starts with the query first");
        // Preset lists "server" as a keyword and comes first in the tab, Server owns it as title
        assert_eq!(titles("server")[..2], [(3, "Server"), (3, "Preset")]);
        // keyword-only hit comes after a title hit
        let r = titles("dark");
        assert_eq!(r[0], (0, "Appearance"), "keyword match");
        let r = titles("ollama");
        assert!(r.contains(&(3, "Server")));
    }

    #[test]
    fn ranks_are_ordered_title_start_title_contains_keyword_start_keyword_contains() {
        let launcher = ENTRIES.iter().find(|e| e.title == "Launcher hotkey").unwrap();
        let terms = |q: &'static str| q.split_whitespace().collect::<Vec<_>>();
        assert_eq!(rank(launcher, "launcher", &terms("launcher")), Some(0));
        assert_eq!(rank(launcher, "hotkey", &terms("hotkey")), Some(1));
        assert_eq!(rank(launcher, "short", &terms("short")), Some(2));
        assert_eq!(rank(launcher, "cut", &terms("cut")), Some(3));
        assert_eq!(rank(launcher, "zzz", &terms("zzz")), None);
    }

    #[test]
    fn every_term_must_match_title_or_keywords() {
        assert!(titles("launcher shortcut").contains(&(0, "Launcher hotkey")));
        assert!(titles("shortcut launcher").contains(&(0, "Launcher hotkey")));
        assert!(search("launcher zzzz").is_empty(), "one unmatched term rejects the entry");
        assert!(titles("api keychain").contains(&(3, "API key")));
        assert!(!titles("api shortcut").contains(&(3, "API key")));
    }

    #[test]
    fn case_and_accents_are_ignored() {
        assert_eq!(normalize("Café ÑANDÚ"), "cafe nandu");
        assert_eq!(titles("LAUNCHER"), titles("launcher"));
        assert_eq!(titles("  launcher  "), titles("launcher"));
        assert_eq!(titles("compáct"), titles("compact"));
    }

    #[test]
    fn results_are_capped_at_eight_and_stable() {
        let r = titles("e");
        assert_eq!(r.len(), MAX_RESULTS);
        assert_eq!(r, titles("e"), "deterministic");
        // inside a rank: by tab
        let tabs: Vec<usize> = search("wheel").iter().map(|e| e.tab).collect();
        let mut sorted = tabs.clone();
        sorted.sort();
        assert_eq!(tabs, sorted, "equal ranks keep tab order: {:?}", titles("wheel"));
    }

    #[test]
    fn match_range_points_at_the_first_word() {
        assert_eq!(match_range("Launcher hotkey", "hot"), Some((9, 3)));
        assert_eq!(match_range("Launcher hotkey", "zzz"), None);
        assert_eq!(match_range("Compact mode", "  MODE x"), Some((8, 4)));
    }

    #[test]
    fn every_entry_has_a_title_a_valid_tab_and_unique_title_within_its_tab() {
        let mut seen = std::collections::HashSet::new();
        for en in ENTRIES {
            assert!(!en.title.trim().is_empty());
            assert!(en.tab <= 6, "{} has tab {}", en.title, en.tab);
            assert!(seen.insert((en.tab, en.title)), "duplicate {} in tab {}", en.title, en.tab);
        }
        let tabs: Vec<usize> = ENTRIES.iter().map(|e| e.tab).collect();
        let mut sorted = tabs.clone();
        sorted.sort();
        assert_eq!(tabs, sorted, "entries are listed by tab");
        for t in 0..=6 {
            assert!(ENTRIES.iter().any(|e| e.tab == t), "tab {t} has entries");
        }
    }
}
