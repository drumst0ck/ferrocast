//! The wire protocol of `ferrocast-translate`, the Swift helper inside the app bundle that calls
//! Apple's Translation framework (Swift-only, macOS 26+). One JSON object goes in on stdin and one
//! comes out on stdout. Pure encode/decode here; the process runs in `ferrocast_macos`.

use serde_json::{json, Value};

/// Whether the language pair is ready on this Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Downloaded: translating works offline.
    Installed,
    /// Apple supports the pair but the language pack is not downloaded yet.
    Supported,
    /// Apple does not translate this pair.
    Unsupported,
}

/// Why a translation did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslateError {
    /// The pack of this language is not downloaded (the code is the one we asked for).
    NeedsDownload(String),
    /// Apple does not support this language (or we have no code for it).
    Unsupported(String),
    /// macOS older than 26: the framework's headless API is missing.
    UnsupportedOs,
    /// Anything else, as a short message.
    Other(String),
}

/// The request asking whether `source` (or the system language when `None`) to `target` is ready.
pub fn status_request(source: Option<&str>, target: &str) -> String {
    let mut v = json!({ "cmd": "status", "target": target });
    if let Some(s) = source {
        v["source"] = json!(s);
    }
    v.to_string()
}

/// The request translating `text` into `target`; without `source` the helper detects it.
pub fn translate_request(source: Option<&str>, target: &str, text: &str) -> String {
    let mut v = json!({ "cmd": "translate", "target": target, "text": text });
    if let Some(s) = source {
        v["source"] = json!(s);
    }
    v.to_string()
}

fn object(out: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(out.trim()).map_err(|_| "the translation helper answered something unreadable".to_string())
}

/// The status in the helper's answer to a status request.
pub fn parse_status(out: &str) -> Result<Status, TranslateError> {
    let v = object(out).map_err(TranslateError::Other)?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(error_of(e, &v));
    }
    match v.get("status").and_then(Value::as_str) {
        Some("installed") => Ok(Status::Installed),
        Some("supported") => Ok(Status::Supported),
        Some("unsupported") => Ok(Status::Unsupported),
        _ => Err(TranslateError::Other("the translation helper sent no status".into())),
    }
}

/// The translated text in the helper's answer to a translate request.
pub fn parse_translation(out: &str) -> Result<String, TranslateError> {
    let v = object(out).map_err(TranslateError::Other)?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(error_of(e, &v));
    }
    match v.get("text").and_then(Value::as_str) {
        Some(t) if !t.trim().is_empty() => Ok(t.to_string()),
        _ => Err(TranslateError::Other("Apple Translation returned an empty result".into())),
    }
}

fn error_of(code: &str, v: &Value) -> TranslateError {
    let language = v.get("language").and_then(Value::as_str).unwrap_or_default().to_string();
    match code {
        "needs-download" => TranslateError::NeedsDownload(language),
        "unsupported" => TranslateError::Unsupported(language),
        "unsupported-os" => TranslateError::UnsupportedOs,
        other => TranslateError::Other(other.chars().take(160).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_one_json_object_and_carry_the_text_untouched() {
        let text = "Line 1\n\"quoted\" \\ back — 日本語 {not: json}";
        let v: Value = serde_json::from_str(&translate_request(Some("en"), "de", text)).unwrap();
        assert_eq!(v, json!({ "cmd": "translate", "source": "en", "target": "de", "text": text }));
        let v: Value = serde_json::from_str(&translate_request(None, "zh-Hans", "hi")).unwrap();
        assert!(v.get("source").is_none(), "no source: the helper detects it");
        let v: Value = serde_json::from_str(&status_request(None, "de")).unwrap();
        assert_eq!(v, json!({ "cmd": "status", "target": "de" }));
        let v: Value = serde_json::from_str(&status_request(Some("es"), "de")).unwrap();
        assert_eq!(v, json!({ "cmd": "status", "source": "es", "target": "de" }));
    }

    #[test]
    fn statuses_decode() {
        assert_eq!(parse_status(r#"{"status":"installed"}"#), Ok(Status::Installed));
        assert_eq!(parse_status("{\"status\":\"supported\"}\n"), Ok(Status::Supported));
        assert_eq!(parse_status(r#"{"status":"unsupported"}"#), Ok(Status::Unsupported));
        assert_eq!(parse_status(r#"{"error":"unsupported-os"}"#), Err(TranslateError::UnsupportedOs));
        assert!(matches!(parse_status(r#"{"status":"maybe"}"#), Err(TranslateError::Other(_))));
        assert!(matches!(parse_status("not json"), Err(TranslateError::Other(_))));
        assert!(matches!(parse_status(""), Err(TranslateError::Other(_))));
    }

    #[test]
    fn translations_and_their_errors_decode() {
        assert_eq!(parse_translation(r#"{"text":"Guten Morgen"}"#), Ok("Guten Morgen".to_string()));
        assert_eq!(parse_translation(r#"{"error":"needs-download","language":"de"}"#), Err(TranslateError::NeedsDownload("de".into())));
        assert_eq!(parse_translation(r#"{"error":"unsupported","language":"sw"}"#), Err(TranslateError::Unsupported("sw".into())));
        assert_eq!(parse_translation(r#"{"error":"unsupported-os"}"#), Err(TranslateError::UnsupportedOs));
        assert_eq!(parse_translation(r#"{"error":"session failed"}"#), Err(TranslateError::Other("session failed".into())));
        assert!(matches!(parse_translation(r#"{"text":"  "}"#), Err(TranslateError::Other(_))), "an empty result is an error");
        assert!(matches!(parse_translation("{}"), Err(TranslateError::Other(_))));
        let long = format!(r#"{{"error":"{}"}}"#, "x".repeat(500));
        match parse_translation(&long) {
            Err(TranslateError::Other(m)) => assert_eq!(m.len(), 160, "a long error message is cut"),
            other => panic!("{other:?}"),
        }
    }
}
