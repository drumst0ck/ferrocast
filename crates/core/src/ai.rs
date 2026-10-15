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

/// Streaming request carrying earlier `(question, answer)` turns plus the new question.
pub fn build_chat_request(model: &str, history: &[(String, String)], question: &str) -> String {
    let mut messages = vec![serde_json::json!({"role": "system", "content": SYSTEM_PROMPT})];
    for (q, a) in history {
        messages.push(serde_json::json!({"role": "user", "content": q}));
        messages.push(serde_json::json!({"role": "assistant", "content": a}));
    }
    messages.push(serde_json::json!({"role": "user", "content": question}));
    serde_json::json!({"model": model, "stream": true, "messages": messages}).to_string()
}

#[derive(Debug, PartialEq)]
pub enum Sse {
    Delta(String),
    Done,
    Error(String),
    /// Keep-alives, role-only chunks, blank lines…
    Ignore,
}

/// Interpret one line of a `text/event-stream` chat completion.
pub fn parse_sse_line(line: &str) -> Sse {
    let line = line.trim();
    let Some(data) = line.strip_prefix("data:") else {
        return Sse::Ignore;
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Sse::Done;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
        return Sse::Ignore;
    };
    if let Some(err) = v.get("error") {
        return Sse::Error(err.get("message").and_then(|m| m.as_str()).or_else(|| err.as_str()).unwrap_or("unknown error").to_string());
    }
    match v.pointer("/choices/0/delta/content").and_then(|c| c.as_str()) {
        Some(t) if !t.is_empty() => Sse::Delta(t.to_string()),
        _ => Sse::Ignore,
    }
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
    fn chat_request_keeps_history_in_order() {
        let h = vec![("q1".to_string(), "a1".to_string())];
        let v: serde_json::Value = serde_json::from_str(&build_chat_request("m", &h, "q2")).unwrap();
        let roles: Vec<_> = v["messages"].as_array().unwrap().iter().map(|m| m["role"].as_str().unwrap().to_string()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert_eq!(v["messages"][3]["content"], "q2");
        assert_eq!(v["stream"], true);
    }

    #[test]
    fn sse_lines() {
        assert_eq!(parse_sse_line(r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#), Sse::Delta("Hel".into()));
        assert_eq!(parse_sse_line(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#), Sse::Ignore);
        assert_eq!(parse_sse_line("data: [DONE]"), Sse::Done);
        assert_eq!(parse_sse_line(": keep-alive"), Sse::Ignore);
        assert_eq!(parse_sse_line(""), Sse::Ignore);
        assert_eq!(parse_sse_line(r#"data: {"error":{"message":"rate limited"}}"#), Sse::Error("rate limited".into()));
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
