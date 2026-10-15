//! Import snippets from JSON exports (Raycast's `[{"name","text","keyword"}]` and Ferrocast's own).

use crate::models::Snippet;

/// Parse a snippet export. Entries without a name or text are skipped.
pub fn parse_snippets(json: &str) -> Result<Vec<Snippet>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Not valid JSON: {e}"))?;
    let arr = v.as_array().ok_or("Expected a JSON array of snippets")?;
    let text_of = |o: &serde_json::Value, keys: &[&str]| keys.iter().find_map(|k| o.get(*k).and_then(|x| x.as_str())).map(str::to_string);
    let out: Vec<Snippet> = arr
        .iter()
        .filter_map(|o| {
            let text = text_of(o, &["text", "content"]).filter(|t| !t.is_empty())?;
            let name = text_of(o, &["name", "title"]).filter(|n| !n.trim().is_empty())?;
            Some(Snippet::new(name.trim(), &text, text_of(o, &["keyword", "trigger"]).unwrap_or_default().trim()))
        })
        .collect();
    if out.is_empty() {
        return Err("No snippets found in that file".into());
    }
    Ok(out)
}

/// Add `incoming` to `existing`, skipping exact duplicates and dropping keywords that would
/// collide with an existing snippet. Returns `(added, keyword_conflicts)`.
pub fn merge_snippets(existing: &mut Vec<Snippet>, incoming: Vec<Snippet>) -> (usize, usize) {
    let (mut added, mut conflicts) = (0, 0);
    for mut s in incoming {
        if existing.iter().any(|e| e.name == s.name && e.text == s.text) {
            continue;
        }
        if !s.keyword.is_empty() && existing.iter().any(|e| e.keyword == s.keyword) {
            s.keyword.clear();
            conflicts += 1;
        }
        existing.push(s);
        added += 1;
    }
    (added, conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raycast_style_export() {
        let s = parse_snippets(r#"[{"name":"Sig","text":"Best,\nMe","keyword":"!sig"},{"name":"","text":"x"},{"name":"NoText"},{"title":"Alt","content":"c"}]"#).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].name.as_str(), s[0].keyword.as_str()), ("Sig", "!sig"));
        assert_eq!(s[1].name, "Alt");
    }

    #[test]
    fn rejects_bad_input_with_readable_errors() {
        assert!(parse_snippets("nope").unwrap_err().contains("Not valid JSON"));
        assert!(parse_snippets("{}").unwrap_err().contains("array"));
        assert!(parse_snippets("[]").unwrap_err().contains("No snippets"));
    }

    #[test]
    fn merge_skips_duplicates_and_resolves_keyword_conflicts() {
        let mut have = vec![Snippet::new("A", "aa", "!a")];
        let incoming = vec![Snippet::new("A", "aa", "!a"), Snippet::new("B", "bb", "!a"), Snippet::new("C", "cc", "!c")];
        assert_eq!(merge_snippets(&mut have, incoming), (2, 1));
        assert_eq!(have.len(), 3);
        assert!(have[1].keyword.is_empty(), "conflicting keyword is dropped, snippet kept");
        assert_eq!(have[2].keyword, "!c");
    }
}
