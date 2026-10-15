//! OpenAI-compatible chat completions (works with Ollama, LM Studio, OpenAI, OpenRouter…).

pub const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";
pub const DEFAULT_MODEL: &str = "llama3.2";
pub const SYSTEM_PROMPT: &str = "You are a concise assistant inside a keyboard launcher. Answer directly; prefer short answers.";

pub fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim().trim_end_matches('/'))
}

pub fn build_request(model: &str, prompt: &str) -> String {
    serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": prompt},
        ],
    })
    .to_string()
}

/// Extract the assistant text, or a readable provider error.
pub fn parse_response(body: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        let snippet: String = body.chars().take(160).collect();
        format!("Unexpected response from the AI server: {snippet}")
    })?;
    if let Some(err) = v.get("error") {
        let msg = err.get("message").and_then(|m| m.as_str()).or_else(|| err.as_str()).unwrap_or("unknown error");
        return Err(msg.to_string());
    }
    v.pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "The AI server returned an empty answer".to_string())
}

/// Escape a value for a double-quoted curl config (`-K`) string.
pub fn curl_config_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// curl config read from stdin: keeps the API key and prompt out of `ps`.
pub fn curl_config(url: &str, api_key: Option<&str>, body: &str) -> String {
    let mut cfg = format!("url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\n", curl_config_quote(url));
    if let Some(k) = api_key.filter(|k| !k.is_empty()) {
        cfg.push_str(&format!("header = {}\n", curl_config_quote(&format!("Authorization: Bearer {k}"))));
    }
    cfg.push_str(&format!("data = {}\n", curl_config_quote(body)));
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_normalises_slashes() {
        assert_eq!(endpoint("http://localhost:11434/v1/"), "http://localhost:11434/v1/chat/completions");
        assert_eq!(endpoint(" https://api.openai.com/v1 "), "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn request_is_valid_json_with_escaped_prompt() {
        let r = build_request("m", "say \"hi\"\nplease");
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["messages"][1]["content"], "say \"hi\"\nplease");
        assert_eq!(v["stream"], false);
    }

    #[test]
    fn parses_answer_and_errors() {
        assert_eq!(parse_response(r#"{"choices":[{"message":{"content":"  42 \n"}}]}"#).unwrap(), "42");
        assert_eq!(parse_response(r#"{"error":{"message":"model not found"}}"#).unwrap_err(), "model not found");
        assert!(parse_response("<html>502</html>").unwrap_err().contains("Unexpected response"));
        assert!(parse_response(r#"{"choices":[]}"#).is_err());
    }

    #[test]
    fn curl_config_round_trips_special_characters() {
        let body = build_request("m", "a \"b\" \\ c\nd");
        let cfg = curl_config("http://x/v1/chat/completions", Some("sk-secret"), &body);
        assert!(cfg.contains("Authorization: Bearer sk-secret"));
        assert!(!cfg.contains('\n') || cfg.lines().all(|l| !l.is_empty()), "no stray blank lines");
        // the data line must be a single line (newlines escaped) so curl reads it whole
        assert_eq!(cfg.lines().filter(|l| l.starts_with("data = ")).count(), 1);
        assert!(!curl_config("u", None, "{}").contains("Authorization"));
    }
}
