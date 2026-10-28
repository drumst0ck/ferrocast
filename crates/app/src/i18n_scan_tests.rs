//! A new English literal handed to a text control, toast or notification must be in the
//! translation table. This reads the app's own sources and fails with the list of strings that
//! are not (or of `format!` texts that should be `trf`).

use ferrocast_core::i18n;
use std::collections::HashSet;

/// Functions and methods whose string arguments are shown to the user.
const SINKS: &[&str] = &[
    "toast",
    "toast_msg",
    "Toast",
    "notify",
    "RichText::new",
    "small_btn",
    "accent_btn",
    "accent_btn_r",
    "pill",
    "text_btn",
    "bar_btn",
    "icon_btn",
    "row",
    "row_chip",
    "row_stacked_chip",
    "form_row",
    "form_row_compact",
    "form_row_stacked",
    "plain_row",
    "note",
    "card_note",
    "note_line",
    "section",
    "status_chip",
    "empty_state",
    "hint_text",
    "on_hover_text",
    "selectable_label",
    "capture_say",
    "text",
    "button",
    "label",
    "lbl",
    "heading",
    "Button::new",
    "Label::new",
    "tip",
    "title",
    "kv",
    "Action::new",
    "cell",
    "caption",
    "toggle_row",
    "row_drag",
    "list_row",
    "info_chip",
    "progress_card",
    "secondary_button_tip",
    "primary_button",
    "primary_button_w",
    "theme_color_row",
    "segmented",
    "seg_index",
    "segmented_bar",
];

/// Literals handed to those functions that are identifiers, not text.
/// Sinks whose labels sit in a list (`&[(Value, "Low"), ...]`): every literal counts.
const DEEP: &[&str] = &["segmented", "seg_index", "segmented_bar"];

const IDS: &[&str] = &["small_btn", "fullscreen_btn", "data_source", "loading", "empty", "page", "database", "image", "video", "audio"];

/// Text that reads the same in every language (names, examples, keys, units).
fn same_everywhere(s: &str) -> bool {
    // a template: judge the text around its `{values}`
    if s.contains('{') {
        let mut plain = String::new();
        let mut depth = 0;
        for c in s.chars() {
            match c {
                '{' => depth += 1,
                '}' if depth > 0 => depth -= 1,
                c if depth == 0 => plain.push(c),
                _ => {}
            }
        }
        if plain != s {
            return same_everywhere_plain(&plain);
        }
    }
    same_everywhere_plain(s)
}

fn same_everywhere_plain(s: &str) -> bool {
    // only short unit-like words ("s", "px", "MB") left around values
    if s.split(|c: char| !c.is_alphabetic()).all(|w| w.chars().count() <= 2) {
        return true;
    }
    const NAMES: &[&str] = &[
        "Anthropic",
        "ChatGPT",
        "Claude Code",
        "LM Studio",
        "Ollama",
        "OpenAI",
        "OpenAI API",
        "Apple",
        "Notion",
        "Obsidian",
        "PNG",
        "Ferrocast",
        "Anthropic API",
        "GitHub",
        "MP4",
        "GIF",
        "ZIP",
        "PDF",
        "JPEG",
        "WAV",
        "MP3",
        "Raycast",
    ];
    !s.chars().any(|c| c.is_alphabetic())
        || NAMES.contains(&s)
        || s.chars().filter(|c| c.is_alphabetic()).count() <= 1
        || ["http", "~/", "/", "sk-", "ntn_", "e.g. ", "x-apple"].iter().any(|p| s.starts_with(p))
        || s.ends_with(" fps")
        || s.chars().all(|c| !c.is_alphabetic() || c.is_ascii_digit() || "×xmshpt%".contains(c)) && s.len() <= 6
}

/// Remove `#[cfg(test)] mod …` blocks (they run to the end of the file in this code base).
fn production_part(text: &str) -> &str {
    let cut = ["\n#[cfg(test)]\nmod ", "\n#[cfg(test)]\n#[path"].iter().filter_map(|m| text.find(m)).min();
    &text[..cut.unwrap_or(text.len())]
}

/// String literals directly in the arguments of the call that opens at `args` (just after the
/// `(`), up to its closing parenthesis. Stops at a closure (`|`) and skips nested `format!`/calls
/// that are reported separately. Returns (literals, saw_format).
fn call_literals(args: &str, deep: bool) -> (Vec<String>, bool) {
    let b: Vec<char> = args.chars().collect();
    let (mut depth, mut i) = (1i32, 0usize);
    let (mut out, mut saw_format) = (vec![], false);
    while i < b.len() && depth > 0 {
        match b[i] {
            '"' => {
                let mut lit = String::new();
                let start = i;
                i += 1;
                while i < b.len() && b[i] != '"' {
                    if b[i] == '\\' && i + 1 < b.len() {
                        i += 1;
                        match b[i] {
                            'n' => lit.push('\n'),
                            'u' => {
                                // \u{201c}
                                let mut hex = String::new();
                                i += 2;
                                while i < b.len() && b[i] != '}' {
                                    hex.push(b[i]);
                                    i += 1;
                                }
                                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    lit.push(c);
                                }
                            }
                            c => lit.push(c),
                        }
                    } else {
                        lit.push(b[i]);
                    }
                    i += 1;
                }
                // a literal that is the first argument of format!(...) is a template
                let before: String = b[..start].iter().rev().take(9).collect::<String>().chars().rev().collect();
                if before.ends_with("format!(") {
                    saw_format = saw_format || !same_everywhere(&lit);
                } else if deep || depth == 1 || before.trim_end().ends_with("else {") || before.trim_end().ends_with('{') {
                    out.push(lit);
                }
            }
            '|' => break,
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    (out, saw_format)
}

fn sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut v = vec![];
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with(".rs") && !name.contains("test") && name != "main.rs" {
            v.push((name, std::fs::read_to_string(&p).unwrap()));
        }
    }
    v.sort();
    v
}

/// The native (AppKit) files of `ferrocast-macos` that draw text or report to the user; their
/// English sentences must be in the table too.
fn native_sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../macos/src");
    let files = [
        "glow.rs",
        "hud.rs",
        "notes.rs",
        "notes_format.rs",
        "notes_hints.rs",
        "notes_palette.rs",
        "notes_popups.rs",
        "shelf.rs",
        "system.rs",
        "tiling.rs",
        "tiling_overlay.rs",
        "windows.rs",
    ];
    files.iter().map(|f| (format!("macos/{f}"), std::fs::read_to_string(dir.join(f)).unwrap())).collect()
}

/// Calls to a sink in `text`: (line, sink, literals, saw_format).
fn sink_calls(text: &str) -> Vec<(usize, &'static str, Vec<String>, bool)> {
    let mut out = vec![];
    for sink in SINKS {
        let needle = format!("{sink}(");
        let mut from = 0;
        while let Some(i) = text[from..].find(&needle) {
            let at = from + i;
            from = at + needle.len();
            // a whole identifier or path: not `fn row(` , not `x_row(`, not `.row(` on another type
            let before = text[..at].chars().last();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
            if text[line_start..at].contains("fn ") {
                continue;
            }
            let (lits, fmt) = call_literals(&text[from..], DEEP.contains(sink));
            out.push((text[..at].matches('\n').count() + 1, *sink, lits, fmt));
        }
    }
    out
}

#[test]
fn every_english_literal_given_to_a_text_control_has_a_translation() {
    let table: HashSet<&str> = i18n::rows().map(|(k, _)| k).collect();
    let mut missing = std::collections::BTreeSet::new();
    for (name, text) in sources() {
        for (line, sink, lits, _) in sink_calls(production_part(&text)) {
            for l in lits {
                if !same_everywhere(&l) && !IDS.contains(&l.as_str()) && !table.contains(l.as_str()) {
                    missing.insert(format!("{name}:{line} {sink}({l:?})"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} literals without a row in strings.tsv:\n{}",
        missing.len(),
        missing.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn text_with_values_in_it_is_built_with_trf_not_format() {
    let mut found = std::collections::BTreeSet::new();
    for (name, text) in sources() {
        for (line, sink, _, fmt) in sink_calls(production_part(&text)) {
            if fmt {
                found.insert(format!("{name}:{line} {sink}(format!(..))"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} texts built with format! (use trf/trn):\n{}",
        found.len(),
        found.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// Prose that is not handed to a text control directly (error messages returned as `Err(String)`,
/// status lines, labels kept in tables) still reaches the user through a toast, a card or a
/// notification, which translate their text on display. So every English sentence in the code
/// base must be in the table too. Only text that reads the same everywhere may stay out.
#[test]
fn every_english_sentence_in_the_code_has_a_translation() {
    use std::fmt::Write;
    const ALLOWED: &[&str] = &[
        // product and provider names that appear in sentences
        "Anthropic API",
        "Claude Code",
        "OpenAI API",
        "Ferrocast launcher",
        "Ferrocast Notes",
        "Hello World",
        // text from the operating system
        "Connection refused",
        "Failed to connect",
        "Invalid argument",
        "Hardware Port:",
        "Compress: ",
        "Sample text",
    ];
    let table: HashSet<&str> = i18n::rows().map(|(k, _)| k).collect();
    let mut missing = String::new();
    let mut count = 0;
    for (name, text) in sources().into_iter().chain(native_sources()) {
        let code = production_part(&text);
        // the dev screenshot fixture is sample content, never shown to users
        let (from, to) = (code.find("fn dev_screenshot"), code.find("fn dev_fixture_path"));
        let skip = from.zip(to).map(|(a, b)| a..b);
        let mut offset = 0;
        for (n, line) in code.split('\n').enumerate() {
            let at = offset;
            offset += line.len() + 1;
            if skip.as_ref().is_some_and(|r| r.contains(&at)) {
                continue;
            }
            let t = line.trim_start();
            let skipped =
                ["//", "#[", "assert", "log(", "eprintln", "println", "panic!", ".expect(", "Id::new", "record_", ".arg(", "FERROCAST"];
            if skipped.iter().any(|p| t.starts_with(p) || line.contains(p) && *p != "//" && *p != "#[") || line.contains("cfg!") {
                continue;
            }
            let mut rest = line;
            while let Some(i) = rest.find('"') {
                let after = &rest[i + 1..];
                let Some(j) = after.find('"') else { break };
                let lit = &after[..j];
                rest = &after[j + 1..];
                let words = lit.split(' ').count();
                let prose = lit.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && words >= 2
                    && lit.len() >= 8
                    && !lit.contains(|c| "{}=/_<>\\".contains(c));
                if prose && !table.contains(lit) && !ALLOWED.contains(&lit) && !same_everywhere(lit) {
                    count += 1;
                    let _ = writeln!(missing, "{name}:{} {lit:?}", n + 1);
                }
            }
        }
    }
    assert!(count == 0, "{count} English sentences without a row in strings.tsv:\n{missing}");
}
