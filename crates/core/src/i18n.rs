//! Interface languages. English is the source: a string is its own key, so a missing translation
//! (or an unknown language) shows the English text and nothing breaks.
//!
//! All catalogs live in one embedded table, `i18n/strings.tsv`: a header row, then one row per
//! string with the English text first and one column per language, tab separated. `\n`, `\t`
//! and `\\` are escaped. Placeholders are `{name}` and are filled by [`trf`]. A test fails when a
//! cell is empty, when a row's placeholders differ from the English text, or when source code
//! asks for a key the table does not have.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// An interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    Es,
    De,
    Fr,
    PtBr,
    Ja,
    ZhHans,
}

impl Lang {
    pub const ALL: [Lang; 7] = [Lang::En, Lang::Es, Lang::De, Lang::Fr, Lang::PtBr, Lang::Ja, Lang::ZhHans];

    /// Code stored in settings and used as the column header of the table.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::PtBr => "pt-BR",
            Lang::Ja => "ja",
            Lang::ZhHans => "zh-Hans",
        }
    }

    /// The language's own name, as shown in the picker (never translated).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::PtBr => "Português (Brasil)",
            Lang::Ja => "日本語",
            Lang::ZhHans => "简体中文",
        }
    }

    /// Whether the language needs the wide fallback font (San Francisco has no CJK glyphs).
    pub fn needs_wide_font(self) -> bool {
        matches!(self, Lang::Ja | Lang::ZhHans)
    }

    fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(code))
    }

    fn from_u8(v: u8) -> Lang {
        Lang::ALL.get(v as usize).copied().unwrap_or(Lang::En)
    }
}

/// The language a macOS language tag ("es-ES", "pt-PT", "zh-Hans-CN", "en_GB") maps to. European
/// Portuguese uses the Brazilian text; Traditional Chinese has no translation (`None`: English).
pub fn match_tag(tag: &str) -> Option<Lang> {
    let tag = tag.replace('_', "-");
    let mut parts = tag.split('-');
    let lang = parts.next()?.to_ascii_lowercase();
    let rest: Vec<String> = parts.map(|p| p.to_ascii_lowercase()).collect();
    match lang.as_str() {
        "en" => Some(Lang::En),
        "es" => Some(Lang::Es),
        "de" => Some(Lang::De),
        "fr" => Some(Lang::Fr),
        "pt" => Some(Lang::PtBr),
        "ja" => Some(Lang::Ja),
        "zh" => {
            let traditional = rest.iter().any(|p| matches!(p.as_str(), "hant" | "tw" | "hk" | "mo"));
            if traditional {
                None
            } else {
                Some(Lang::ZhHans)
            }
        }
        _ => None,
    }
}

/// The first of the user's preferred languages (most preferred first) that has a translation;
/// English when none has. A preferred language without a translation is skipped, like macOS does
/// for apps, except that an explicit English preference ahead of the others wins.
pub fn from_preferred(preferred: &[String]) -> Lang {
    preferred.iter().find_map(|t| match_tag(t)).unwrap_or(Lang::En)
}

/// Language for the setting value: `"system"` (or anything unknown) follows `preferred`.
pub fn resolve(setting: &str, preferred: &[String]) -> Lang {
    Lang::from_code(setting).unwrap_or_else(|| from_preferred(preferred))
}

/// Settings value for a language picked by hand.
pub fn setting_value(l: Lang) -> &'static str {
    l.code()
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Switch the interface language (takes effect on the next frame / next string asked for). On a
/// thread running [`with_language`] it replaces that thread's language and leaves the global one alone.
pub fn set_language(l: Lang) {
    if OVERRIDE.with(|o| o.get()).is_some() {
        OVERRIDE.with(|o| o.set(Some(l)));
    } else {
        CURRENT.store(l as u8, Ordering::Relaxed);
    }
}

thread_local! {
    static OVERRIDE: std::cell::Cell<Option<Lang>> = const { std::cell::Cell::new(None) };
}

/// The current interface language (this thread's override, see [`with_language`], else the global one).
pub fn language() -> Lang {
    OVERRIDE.with(|o| o.get()).unwrap_or_else(|| Lang::from_u8(CURRENT.load(Ordering::Relaxed)))
}

/// Run `f` with `l` as this thread's language, whatever the global one is: tests run in parallel.
/// Misses are only recorded (see [`take_misses`]) while this is active.
pub fn with_language<R>(l: Lang, f: impl FnOnce() -> R) -> R {
    let prev = OVERRIDE.with(|o| o.replace(Some(l)));
    let r = f();
    OVERRIDE.with(|o| o.set(prev));
    r
}

const TABLE: &str = include_str!("i18n/strings.tsv");

type Catalog = HashMap<&'static str, Vec<&'static str>>;

fn unescape(s: &str) -> Cow<'_, str> {
    if !s.contains('\\') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// A leaked copy only when unescaping changed the text; the table lives for the whole process.
fn intern(s: &'static str) -> &'static str {
    match unescape(s) {
        Cow::Borrowed(b) => b,
        Cow::Owned(o) => Box::leak(o.into_boxed_str()),
    }
}

/// Parsed rows: key (English) → cells for `Lang::ALL[1..]`; an empty cell stays empty.
fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| {
        let mut m = Catalog::new();
        for line in TABLE.lines().skip(1).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut cols = line.split('\t');
            let Some(key) = cols.next() else { continue };
            m.insert(intern(key), cols.map(intern).collect());
        }
        m
    })
}

/// Every row of the table as (English key, cells per non-English language).
pub fn rows() -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
    catalog().iter().map(|(k, v)| (*k, v.as_slice()))
}

/// Text of `key` in `lang`; `None` for English or when the table has no (non-empty) translation.
pub fn lookup(key: &str, lang: Lang) -> Option<&'static str> {
    if lang == Lang::En {
        return None;
    }
    catalog().get(key).and_then(|c| c.get(lang as usize - 1)).copied().filter(|s| !s.is_empty())
}

thread_local! {
    /// Keys asked for inside [`with_language`] with a non-English language and not found in the table.
    static MISSES: std::cell::RefCell<std::collections::BTreeSet<String>> = const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
}

/// Translate `key` (English text) into the current language.
pub fn tr(key: &str) -> Cow<'_, str> {
    tr_in(key, language())
}

/// Translate `key` into `lang`; English text when it has no translation.
pub fn tr_in(key: &str, lang: Lang) -> Cow<'_, str> {
    match lookup(key, lang) {
        Some(t) => Cow::Borrowed(t),
        None => {
            if lang != Lang::En && !key.trim().is_empty() && OVERRIDE.with(|o| o.get()).is_some() {
                MISSES.with(|m| m.borrow_mut().insert(key.to_string()));
            }
            Cow::Borrowed(key)
        }
    }
}

/// Translate `text` as used in `context` (the table row is `context|text`), for words whose
/// translation depends on where they appear ("Command" the key, "Command" the launcher item).
/// Falls back to the plain translation of `text`, then to `text`.
pub fn trc(context: &str, text: &str) -> String {
    let lang = language();
    match lookup(&format!("{context}|{text}"), lang) {
        Some(t) => t.to_string(),
        None => tr_in(text, lang).into_owned(),
    }
}

/// Fill `{name}` placeholders of `text`. Unknown placeholders stay as written.
pub fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// Translate `key` and fill its placeholders: `trf("Delete {name}?", &[("name", &n)])`.
pub fn trf(key: &str, args: &[(&str, &str)]) -> String {
    fill(&tr(key), args)
}

/// Plural: `one` for a count of one (zero too in French), `other` otherwise; Japanese and Chinese
/// always use `other`. Both keys may hold `{n}`, filled with the count.
pub fn trn(one: &str, other: &str, n: u64) -> String {
    trn_in(one, other, n, language())
}

pub fn trn_in(one: &str, other: &str, n: u64, lang: Lang) -> String {
    let singular = match lang {
        Lang::Ja | Lang::ZhHans => false,
        Lang::Fr | Lang::PtBr => n < 2,
        _ => n == 1,
    };
    let key = if singular { one } else { other };
    fill(&tr_in(key, lang), &[("n", &n.to_string())])
}

/// Take the keys looked up and not found since the last call, for tests that draw a whole screen
/// inside [`with_language`] and assert that nothing on it is missing.
pub fn take_misses() -> Vec<String> {
    MISSES.with(|m| std::mem::take(&mut *m.borrow_mut())).into_iter().collect()
}

/// Placeholder names (`{name}`) in `s`, in order.
pub fn placeholders(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        let Some(j) = rest[i..].find('}') else { break };
        let name = &rest[i + 1..i + j];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            out.push(name.to_string());
        }
        rest = &rest[i + j + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn language_tags_match_the_supported_languages() {
        assert_eq!(match_tag("es-ES"), Some(Lang::Es));
        assert_eq!(match_tag("es-419"), Some(Lang::Es));
        assert_eq!(match_tag("de-AT"), Some(Lang::De));
        assert_eq!(match_tag("fr-CA"), Some(Lang::Fr));
        assert_eq!(match_tag("pt-BR"), Some(Lang::PtBr));
        assert_eq!(match_tag("pt-PT"), Some(Lang::PtBr), "European Portuguese uses the Brazilian text");
        assert_eq!(match_tag("ja-JP"), Some(Lang::Ja));
        assert_eq!(match_tag("zh-Hans-CN"), Some(Lang::ZhHans));
        assert_eq!(match_tag("zh-CN"), Some(Lang::ZhHans));
        assert_eq!(match_tag("en_GB"), Some(Lang::En));
    }

    #[test]
    fn traditional_chinese_and_unknown_languages_have_no_translation() {
        assert_eq!(match_tag("zh-Hant-TW"), None);
        assert_eq!(match_tag("zh-TW"), None);
        assert_eq!(match_tag("zh-HK"), None);
        assert_eq!(match_tag("ko-KR"), None);
        assert_eq!(match_tag(""), None);
    }

    #[test]
    fn the_first_preferred_language_with_a_translation_wins() {
        assert_eq!(from_preferred(&tags(["ko-KR", "de-DE", "fr-FR"].as_slice())), Lang::De);
        assert_eq!(from_preferred(&tags(["en-US", "es-ES"].as_slice())), Lang::En, "an explicit English preference wins");
        assert_eq!(from_preferred(&tags(["zh-Hant-TW", "ja-JP"].as_slice())), Lang::Ja);
        assert_eq!(from_preferred(&[]), Lang::En);
        assert_eq!(from_preferred(&tags(["ko-KR"].as_slice())), Lang::En);
    }

    #[test]
    fn a_manual_choice_beats_the_system_and_unknown_values_follow_it() {
        let sys = tags(["fr-FR"].as_slice());
        assert_eq!(resolve("system", &sys), Lang::Fr);
        assert_eq!(resolve("ja", &sys), Lang::Ja);
        assert_eq!(resolve("pt-BR", &sys), Lang::PtBr);
        assert_eq!(resolve("en", &sys), Lang::En);
        assert_eq!(resolve("klingon", &sys), Lang::Fr);
        assert_eq!(resolve("", &sys), Lang::Fr);
        for l in Lang::ALL {
            assert_eq!(resolve(setting_value(l), &sys), l, "{l:?} round-trips through the setting");
        }
    }

    #[test]
    fn a_known_key_is_translated_and_an_unknown_one_stays_english() {
        assert_eq!(tr_in("Settings", Lang::Es), "Ajustes");
        assert_eq!(tr_in("Settings", Lang::De), "Einstellungen");
        assert_eq!(tr_in("Settings", Lang::En), "Settings");
        assert_eq!(tr_in("No such string in the table", Lang::Ja), "No such string in the table");
    }

    #[test]
    fn placeholders_are_filled_and_unknown_ones_are_left_alone() {
        assert_eq!(fill("Delete {name}? ({n})", &[("name", "A"), ("n", "3")]), "Delete A? (3)");
        assert_eq!(fill("Hello {who}", &[("name", "A")]), "Hello {who}");
        assert_eq!(placeholders("{n} of {total} {not a name} {}"), ["n", "total"]);
    }

    #[test]
    fn plurals_follow_each_language() {
        // "{n} item" / "{n} items" only exist in English here: the fallback still fills the count
        assert_eq!(trn_in("{n} item", "{n} items", 1, Lang::En), "1 item");
        assert_eq!(trn_in("{n} item", "{n} items", 0, Lang::En), "0 items");
        assert_eq!(trn_in("{n} item", "{n} items", 2, Lang::En), "2 items");
        assert_eq!(trn_in("{n} item", "{n} items", 0, Lang::Fr), "0 item", "French: zero is singular");
        assert_eq!(trn_in("{n} item", "{n} items", 1, Lang::Ja), "1 items", "Japanese has one form");
    }

    // the global is set and restored inside this test only; every other test uses its own override
    #[test]
    fn the_current_language_is_global_and_misses_are_recorded() {
        with_language(Lang::De, || {
            assert_eq!(tr("Settings"), "Einstellungen");
            let _ = tr("A string nobody translated");
            assert_eq!(take_misses(), ["A string nobody translated"]);
        });
        with_language(Lang::En, || {
            assert_eq!(tr("Settings"), "Settings");
            let _ = tr("A string nobody translated");
            assert!(take_misses().is_empty(), "English never misses");
        });
        set_language(Lang::Fr);
        assert_eq!(language(), Lang::Fr, "the global language applies where nothing overrides it");
        with_language(Lang::En, || assert_eq!(language(), Lang::En));
        set_language(Lang::En);
    }

    #[test]
    fn escapes_in_the_table_are_decoded() {
        assert_eq!(unescape("a\\nb\\tc\\\\d"), "a\nb\tc\\d");
        assert_eq!(unescape("plain"), "plain");
    }

    /// The table: header, no duplicate keys, no empty cell, same placeholders as the English text.
    #[test]
    fn every_string_is_translated_into_every_language() {
        let header: Vec<&str> = TABLE.lines().next().unwrap().split('\t').collect();
        let expected: Vec<&str> = Lang::ALL.iter().map(|l| l.code()).collect();
        assert_eq!(header, expected, "header columns");
        let mut seen = std::collections::HashSet::new();
        let (mut missing, mut bad_ph) = (vec![], vec![]);
        for line in TABLE.lines().skip(1).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let cells: Vec<&str> = line.split('\t').collect();
            assert!(seen.insert(cells[0]), "duplicate key {:?}", cells[0]);
            if cells.len() != Lang::ALL.len() {
                missing.push(format!("{:?}: {} columns, expected {}", cells[0], cells.len(), Lang::ALL.len()));
                continue;
            }
            let ph = placeholders(cells[0]);
            for (i, c) in cells.iter().enumerate().skip(1) {
                if c.is_empty() {
                    missing.push(format!("{:?} [{}]", cells[0], Lang::ALL[i].code()));
                } else if placeholders(c) != ph && !(ph.len() > 1 && sorted(placeholders(c)) == sorted(ph.clone())) {
                    bad_ph.push(format!("{:?} [{}] has {:?}", cells[0], Lang::ALL[i].code(), placeholders(c)));
                }
            }
        }
        assert!(missing.is_empty(), "missing translations:\n{}", missing.join("\n"));
        assert!(bad_ph.is_empty(), "placeholders differ from English:\n{}", bad_ph.join("\n"));
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    /// Every literal handed to `tr(`/`trf(`/`trn(` in the sources is a row of the table.
    #[test]
    fn every_string_the_code_asks_for_is_in_the_table() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = vec![];
        for krate in ["core", "app", "macos"] {
            collect_rs(&root.join(krate).join("src"), &mut files);
        }
        let mut missing = std::collections::BTreeSet::new();
        for f in files {
            if f.file_name().is_some_and(|n| n == "i18n.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            for key in literal_keys(&text) {
                if !catalog().contains_key(key.as_str()) {
                    missing.insert(format!("{}: {key:?}", f.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        assert!(missing.is_empty(), "keys used in code but not in strings.tsv:\n{}", missing.into_iter().collect::<Vec<_>>().join("\n"));
    }

    fn collect_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_rs(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// String literals that are the first (or, for `trn`, second) argument of a translation call.
    fn literal_keys(text: &str) -> Vec<String> {
        let mut out = vec![];
        for call in ["tr(", "trf(", "trn(", "trc("] {
            let mut from = 0;
            while let Some(i) = text[from..].find(call) {
                let at = from + i;
                from = at + call.len();
                let before = text[..at].chars().last();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                    continue;
                }
                let mut rest = &text[from..];
                // trc(context, text): the row is `context|text`
                if call == "trc(" {
                    let r = rest.trim_start();
                    if let Some((ctx, after)) = r.starts_with('"').then(|| read_literal(r)).flatten() {
                        if let Some((text, _)) = after
                            .trim_start()
                            .strip_prefix(',')
                            .map(str::trim_start)
                            .and_then(|a| a.starts_with('"').then(|| read_literal(a)).flatten())
                        {
                            out.push(format!("{ctx}|{text}"));
                        }
                    }
                    continue;
                }
                for _ in 0..if call == "trn(" { 2 } else { 1 } {
                    let r = rest.trim_start();
                    if !r.starts_with('"') {
                        break;
                    }
                    let Some((lit, after)) = read_literal(r) else { break };
                    out.push(lit);
                    rest = after.trim_start().strip_prefix(',').unwrap_or("");
                }
            }
        }
        out
    }

    /// The decoded string literal at the start of `s` and what follows it.
    fn read_literal(s: &str) -> Option<(String, &str)> {
        let mut out = String::new();
        let mut it = s.char_indices().skip(1);
        while let Some((i, c)) = it.next() {
            match c {
                '"' => return Some((out, &s[i + 1..])),
                '\\' => match it.next()?.1 {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'u' => {
                        // \u{201c}
                        let hex: String = it.by_ref().map(|(_, c)| c).skip(1).take_while(|c| *c != '}').collect();
                        out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)?);
                    }
                    o => out.push(o),
                },
                c => out.push(c),
            }
        }
        None
    }
}
