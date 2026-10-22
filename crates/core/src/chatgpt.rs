//! "Sign in with ChatGPT" for Ask AI: the OAuth flow and request format that OpenAI's Codex CLI
//! uses, so a ChatGPT subscription can answer without an API key. Pure functions here; the app
//! crate does the network calls and the local callback listener.
//!
//! This speaks to OpenAI as the Codex client does. Whether that use fits OpenAI's terms is the
//! user's call; the Settings pane says so.

use base64::Engine;
use sha2::{Digest, Sha256};

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const CALLBACK_PORT: u16 = 1455;
pub const SCOPE: &str = "openid profile email offline_access";
pub const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
pub const MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models?client_version=1.0.0";
pub const DEFAULT_MODEL: &str = "gpt-5.5";
pub const DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
pub const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
pub const DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";
pub const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
const JWT_CLAIM: &str = "https://api.openai.com/auth";

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// PKCE verifier (random) and its S256 challenge.
pub fn pkce() -> (String, String) {
    let mut raw = [0u8; 32];
    for chunk in raw.chunks_mut(16) {
        chunk.copy_from_slice(&uuid::Uuid::new_v4().into_bytes()[..chunk.len()]);
    }
    let verifier = b64url(&raw);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn random_state() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The browser URL that starts the sign-in.
pub fn authorize_url(challenge: &str, state: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "ferrocast"),
    ];
    let q: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", url_encode(v))).collect();
    format!("{AUTHORIZE_URL}?{}", q.join("&"))
}

/// `code` and `state` from the callback request line (`GET /auth/callback?code=…&state=… HTTP/1.1`)
/// or from a pasted redirect URL.
pub fn parse_callback(input: &str) -> Option<(String, Option<String>)> {
    let query = input.split_once('?')?.1;
    let query = query.split_whitespace().next().unwrap_or(query);
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            match k {
                "code" => code = Some(v.to_string()),
                "state" => state = Some(v.to_string()),
                _ => {}
            }
        }
    }
    code.map(|c| (c, state))
}

/// Form body for exchanging the authorization code.
pub fn exchange_body(code: &str, verifier: &str) -> String {
    format!(
        "grant_type=authorization_code&client_id={}&code={}&code_verifier={}&redirect_uri={}",
        url_encode(CLIENT_ID),
        url_encode(code),
        url_encode(verifier),
        url_encode(REDIRECT_URI)
    )
}

/// Form body for refreshing.
pub fn refresh_body(refresh_token: &str) -> String {
    format!("grant_type=refresh_token&client_id={}&refresh_token={}", url_encode(CLIENT_ID), url_encode(refresh_token))
}

/// Device-code flow, step 1 response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceStart {
    pub device_auth_id: String,
    pub user_code: String,
    pub interval_secs: u64,
}

pub fn device_start_body() -> String {
    serde_json::json!({"client_id": CLIENT_ID}).to_string()
}

pub fn parse_device_start(body: &str) -> Result<DeviceStart, String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| format!("Unexpected device-code response: {}", body.chars().take(160).collect::<String>()))?;
    let interval = match &v["interval"] {
        serde_json::Value::String(s) => s.trim().parse::<u64>().ok(),
        other => other.as_u64(),
    }
    .unwrap_or(5);
    Ok(DeviceStart {
        device_auth_id: v["device_auth_id"].as_str().ok_or("device-code response missing device_auth_id")?.to_string(),
        user_code: v["user_code"].as_str().ok_or("device-code response missing user_code")?.to_string(),
        interval_secs: interval.max(1),
    })
}

pub fn device_poll_body(d: &DeviceStart) -> String {
    serde_json::json!({"device_auth_id": d.device_auth_id, "user_code": d.user_code}).to_string()
}

/// Outcome of one device-code poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    /// Authorization code plus the PKCE verifier the server generated; exchange with `DEVICE_REDIRECT_URI`.
    Ready {
        code: String,
        verifier: String,
    },
    Failed(String),
}

/// `status` is the HTTP status; 403/404 mean "not yet".
pub fn parse_device_poll(status: u16, body: &str) -> DevicePoll {
    if status == 403 || status == 404 {
        return DevicePoll::Pending;
    }
    let v: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return DevicePoll::Failed(format!("device auth failed with status {status}")),
    };
    if (200..300).contains(&status) {
        return match (v["authorization_code"].as_str(), v["code_verifier"].as_str()) {
            (Some(c), Some(ver)) => DevicePoll::Ready { code: c.to_string(), verifier: ver.to_string() },
            _ => DevicePoll::Failed("device auth response missing authorization_code".into()),
        };
    }
    let code = v["error"]["code"].as_str().or_else(|| v["error"].as_str()).unwrap_or("");
    match code {
        "deviceauth_authorization_pending" => DevicePoll::Pending,
        "slow_down" => DevicePoll::SlowDown,
        _ => DevicePoll::Failed(format!("device auth failed with status {status}: {}", body.chars().take(160).collect::<String>())),
    }
}

/// Form body for exchanging a device-flow code (its own redirect URI).
pub fn device_exchange_body(code: &str, verifier: &str) -> String {
    format!(
        "grant_type=authorization_code&client_id={}&code={}&code_verifier={}&redirect_uri={}",
        url_encode(CLIENT_ID),
        url_encode(code),
        url_encode(verifier),
        url_encode(DEVICE_REDIRECT_URI)
    )
}

/// Stored credentials.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Credentials {
    pub access: String,
    pub refresh: String,
    /// Unix seconds.
    pub expires: i64,
    pub account_id: String,
}

impl Credentials {
    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires - 60
    }
}

/// Parse a token endpoint response into credentials (needs the account id inside the JWT).
pub fn parse_token_response(body: &str, now: i64) -> Result<Credentials, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| format!("Unexpected token response: {}", body.chars().take(160).collect::<String>()))?;
    if let Some(e) = v.get("error") {
        let msg = v.get("error_description").and_then(|d| d.as_str()).or_else(|| e.as_str()).unwrap_or("unknown error");
        return Err(msg.to_string());
    }
    let access = v["access_token"].as_str().ok_or("token response missing access_token")?.to_string();
    let refresh = v["refresh_token"].as_str().ok_or("token response missing refresh_token")?.to_string();
    let expires_in = v["expires_in"].as_i64().ok_or("token response missing expires_in")?;
    let account_id = account_id(&access).ok_or("could not read the ChatGPT account id from the token")?;
    Ok(Credentials { access, refresh, expires: now + expires_in, account_id })
}

/// `chatgpt_account_id` from the access token's payload.
pub fn account_id(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v[JWT_CLAIM]["chatgpt_account_id"].as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Responses API body (streaming, nothing stored server-side).
pub fn build_request(model: &str, history: &[(String, String)], question: &str) -> String {
    build_request_with(crate::ai::SYSTEM_PROMPT, model, history, question)
}

/// `build_request` with its own instructions (translations use a translator prompt).
pub fn build_request_with(system: &str, model: &str, history: &[(String, String)], question: &str) -> String {
    let mut input = vec![];
    for (q, a) in history {
        input.push(serde_json::json!({"role": "user", "content": [{"type": "input_text", "text": q}]}));
        input.push(serde_json::json!({"role": "assistant", "content": [{"type": "output_text", "text": a}]}));
    }
    input.push(serde_json::json!({"role": "user", "content": [{"type": "input_text", "text": question}]}));
    serde_json::json!({
        "model": model,
        "store": false,
        "stream": true,
        "instructions": system,
        "input": input,
        "text": {"verbosity": "low"},
    })
    .to_string()
}

/// curl config for a streaming request.
pub fn curl_config(creds: &Credentials, body: &str) -> String {
    use crate::ai::curl_config_quote as q;
    format!(
        "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\nheader = \"Accept: text/event-stream\"\nheader = \"OpenAI-Beta: responses=experimental\"\nheader = \"originator: ferrocast\"\nheader = {}\nheader = {}\ndata = {}\n",
        q(RESPONSES_URL),
        q(&format!("Authorization: Bearer {}", creds.access)),
        q(&format!("chatgpt-account-id: {}", creds.account_id)),
        q(body)
    )
}

/// curl config for the list of models this account can use.
pub fn models_curl_config(creds: &Credentials) -> String {
    use crate::ai::curl_config_quote as q;
    format!(
        "url = {}\nheader = \"originator: ferrocast\"\nheader = {}\nheader = {}\n",
        q(MODELS_URL),
        q(&format!("Authorization: Bearer {}", creds.access)),
        q(&format!("chatgpt-account-id: {}", creds.account_id))
    )
}

/// Model ids from the models endpoint, hidden ones left out. Err carries the server's message.
pub fn parse_models(body: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "unexpected response".to_string())?;
    if let Some(e) = v.get("error").or_else(|| v.get("detail")) {
        let msg = e.get("message").and_then(|m| m.as_str()).or_else(|| e.as_str()).unwrap_or("server error");
        return Err(msg.to_string());
    }
    let list = v.get("models").or_else(|| v.get("data")).and_then(|m| m.as_array()).ok_or("no model list in the response")?;
    let mut out: Vec<String> = list
        .iter()
        .filter(|m| !matches!(m["visibility"].as_str(), Some("hide") | Some("none")))
        .filter_map(|m| m["slug"].as_str().or_else(|| m["id"].as_str()).map(str::to_string))
        .collect();
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err("the account lists no models".into());
    }
    Ok(out)
}

/// One SSE line of a Responses stream.
pub fn parse_sse_line(line: &str) -> crate::ai::Sse {
    use crate::ai::Sse;
    let Some(data) = line.trim().strip_prefix("data:") else { return Sse::Ignore };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data.trim()) else { return Sse::Ignore };
    match v["type"].as_str() {
        Some("response.output_text.delta") => match v["delta"].as_str() {
            Some(t) if !t.is_empty() => Sse::Delta(t.to_string()),
            _ => Sse::Ignore,
        },
        Some("response.completed") | Some("response.done") | Some("response.incomplete") => Sse::Done,
        Some("response.failed") | Some("error") => Sse::Error(
            v.pointer("/response/error/message")
                .or_else(|| v.pointer("/error/message"))
                .or_else(|| v.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("ChatGPT request failed")
                .to_string(),
        ),
        _ => Sse::Ignore,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_custom_instruction_replaces_only_the_instructions() {
        let h = vec![("q".to_string(), "a".to_string())];
        assert_eq!(build_request("m", &h, "x"), build_request_with(crate::ai::SYSTEM_PROMPT, "m", &h, "x"));
        let v: serde_json::Value = serde_json::from_str(&build_request_with("SYS", "m", &h, "x")).unwrap();
        assert_eq!(v["instructions"], "SYS");
    }
    #[test]
    fn parse_models_keeps_listed_slugs_and_drops_hidden_ones() {
        let body = r#"{"models":[{"slug":"gpt-b","visibility":"list"},{"slug":"gpt-a"},{"slug":"old","visibility":"hide"}]}"#;
        assert_eq!(super::parse_models(body).unwrap(), vec!["gpt-a", "gpt-b"]);
        assert_eq!(super::parse_models(r#"{"detail":"nope"}"#).unwrap_err(), "nope");
        assert!(super::parse_models(r#"{"models":[]}"#).is_err());
        assert!(super::parse_models("<html>").is_err());
    }

    use super::*;
    use crate::ai::Sse;

    #[test]
    fn pkce_is_s256_of_verifier() {
        let (v, c) = pkce();
        assert_eq!(c, b64url(&Sha256::digest(v.as_bytes())));
        assert!(authorize_url(&c, "st").contains("code_challenge_method=S256"));
        assert!(authorize_url(&c, "st").contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
    }

    #[test]
    fn callback_parsing() {
        assert_eq!(parse_callback("GET /auth/callback?code=abc&state=xyz HTTP/1.1"), Some(("abc".into(), Some("xyz".into()))));
        assert_eq!(parse_callback("http://localhost:1455/auth/callback?state=s&code=c"), Some(("c".into(), Some("s".into()))));
        assert_eq!(parse_callback("GET /favicon.ico HTTP/1.1"), None);
    }

    #[test]
    fn token_response_and_account_id() {
        let payload = b64url(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc_1"}}"#);
        let jwt = format!("h.{payload}.s");
        assert_eq!(account_id(&jwt).as_deref(), Some("acc_1"));
        let body = format!(r#"{{"access_token":"{jwt}","refresh_token":"r","expires_in":3600}}"#);
        let c = parse_token_response(&body, 1000).unwrap();
        assert_eq!((c.expires, c.account_id.as_str()), (4600, "acc_1"));
        assert!(!c.is_expired(1000));
        assert!(c.is_expired(4600));
        assert!(parse_token_response(r#"{"error":"invalid_grant","error_description":"expired"}"#, 0).unwrap_err().contains("expired"));
    }

    #[test]
    fn device_code_flow_parsing() {
        let d = parse_device_start(r#"{"device_auth_id":"d1","user_code":"ABCD-1234","interval":"5"}"#).unwrap();
        assert_eq!((d.user_code.as_str(), d.interval_secs), ("ABCD-1234", 5));
        assert_eq!(parse_device_poll(403, ""), DevicePoll::Pending);
        assert_eq!(parse_device_poll(400, r#"{"error":{"code":"deviceauth_authorization_pending"}}"#), DevicePoll::Pending);
        assert_eq!(parse_device_poll(400, r#"{"error":{"code":"slow_down"}}"#), DevicePoll::SlowDown);
        assert_eq!(
            parse_device_poll(200, r#"{"authorization_code":"c","code_verifier":"v"}"#),
            DevicePoll::Ready { code: "c".into(), verifier: "v".into() }
        );
        assert!(matches!(parse_device_poll(500, "boom"), DevicePoll::Failed(_)));
        assert!(device_exchange_body("c", "v").contains("deviceauth%2Fcallback"));
    }

    #[test]
    fn request_and_stream() {
        let v: serde_json::Value = serde_json::from_str(&build_request("m", &[("q".into(), "a".into())], "q2")).unwrap();
        assert_eq!(v["store"], false);
        assert_eq!(v["input"].as_array().unwrap().len(), 3);
        assert_eq!(v["input"][1]["content"][0]["type"], "output_text");
        assert_eq!(parse_sse_line(r#"data: {"type":"response.output_text.delta","delta":"Hi"}"#), Sse::Delta("Hi".into()));
        assert_eq!(parse_sse_line(r#"data: {"type":"response.completed","response":{}}"#), Sse::Done);
        assert_eq!(
            parse_sse_line(r#"data: {"type":"response.failed","response":{"error":{"message":"quota"}}}"#),
            Sse::Error("quota".into())
        );
        let c = Credentials { access: "A".into(), refresh: "R".into(), expires: 0, account_id: "acc".into() };
        let cfg = curl_config(&c, "{}");
        assert!(cfg.contains("chatgpt-account-id: acc") && cfg.contains("Bearer A"));
    }
}
