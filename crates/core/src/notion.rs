//! Notion API: request builders, response parsers and error mapping (no I/O).

use crate::ai::curl_config_quote;
use serde_json::{json, Value};

pub const NOTION_VERSION: &str = "2026-03-11";
const API: &str = "https://api.notion.com/v1";

pub fn search_url() -> String {
    format!("{API}/search")
}
pub fn pages_url() -> String {
    format!("{API}/pages")
}
pub fn page_markdown_url(id: &str) -> String {
    format!("{API}/pages/{id}/markdown")
}
pub fn me_url() -> String {
    format!("{API}/users/me")
}
pub fn data_source_url(id: &str) -> String {
    format!("{API}/data_sources/{id}")
}

/// curl config for stdin (`-K -`): the token and body never reach argv. The status code is
/// appended after the body as `\n<code>` (same marker as the update check); see [`split_status`].
pub fn curl_config(token: &str, method: &str, url: &str, body: Option<&str>) -> String {
    let mut cfg = format!(
        "url = {}\nrequest = {}\nheader = {}\nheader = {}\nheader = \"Content-Type: application/json\"\nmax-time = 20\nsilent\nshow-error\nwrite-out = \"\\n%{{http_code}}\"\n",
        curl_config_quote(url),
        curl_config_quote(method),
        curl_config_quote(&format!("Authorization: Bearer {token}")),
        curl_config_quote(&format!("Notion-Version: {NOTION_VERSION}")),
    );
    if let Some(b) = body {
        cfg.push_str(&format!("data = {}\n", curl_config_quote(b)));
    }
    cfg
}

/// Split curl output produced with the `write-out` marker into body and HTTP status
/// (status 0 when the marker is missing).
pub fn split_status(output: &str) -> (&str, u16) {
    crate::update::split_status(output)
}

/// Body of a finished request: `Ok` on 2xx, otherwise the mapped error (status 0 → `Network`).
pub fn from_output(output: &str) -> Result<&str, NotionError> {
    let (body, status) = split_status(output);
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(NotionError::from_status(status, body))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parent {
    Page(String),
    DataSource(String),
}

pub fn search_body(query: &str, cursor: Option<&str>, page_size: u32) -> String {
    let mut b = json!({ "page_size": page_size });
    if query.is_empty() {
        b["sort"] = json!({"timestamp":"last_edited_time","direction":"descending"});
    } else {
        b["query"] = json!(query);
    }
    if let Some(c) = cursor {
        b["start_cursor"] = json!(c);
    }
    b.to_string()
}

pub fn create_page_body(parent: &Parent, title_property: &str, title: &str, markdown: &str) -> String {
    let (parent, prop) = match parent {
        Parent::Page(id) => (json!({ "page_id": id }), "title"),
        Parent::DataSource(id) => (json!({ "data_source_id": id }), title_property),
    };
    json!({
        "parent": parent,
        "properties": { prop: { "title": [{ "text": { "content": title } }] } },
        "markdown": markdown,
    })
    .to_string()
}

pub fn append_markdown_body(content: &str) -> String {
    json!({"type":"insert_content","insert_content":{"content":content,"position":{"type":"end"}}}).to_string()
}

pub fn replace_markdown_body(content: &str) -> String {
    json!({"type":"replace_content","replace_content":{"content":content}}).to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Page,
    DataSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NotionItem {
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    pub url: String,
    pub icon_emoji: Option<String>,
    pub last_edited: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchPage {
    pub items: Vec<NotionItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BotInfo {
    pub name: String,
    pub workspace: Option<String>,
}

fn parse(json: &str) -> Result<Value, NotionError> {
    serde_json::from_str(json).map_err(|_| NotionError::Malformed)
}

fn plain(rich: &Value) -> String {
    rich.as_array().map(|a| a.iter().filter_map(|t| t["plain_text"].as_str()).collect::<String>()).unwrap_or_default()
}

fn or_untitled(s: String) -> String {
    if s.trim().is_empty() {
        "Untitled".into()
    } else {
        s
    }
}

fn item(r: &Value) -> Option<NotionItem> {
    let (kind, title) = match r["object"].as_str()? {
        "page" => {
            let t = r["properties"]
                .as_object()
                .and_then(|p| p.values().find(|v| v["type"] == "title"))
                .map(|v| plain(&v["title"]))
                .unwrap_or_default();
            (ItemKind::Page, t)
        }
        "data_source" => (ItemKind::DataSource, plain(&r["title"])),
        _ => return None,
    };
    let s = |k: &str| r[k].as_str().map(str::to_string);
    Some(NotionItem {
        id: s("id")?,
        kind,
        title: or_untitled(title),
        url: s("url").unwrap_or_default(),
        icon_emoji: (r["icon"]["type"] == "emoji").then(|| s_of(&r["icon"]["emoji"])).flatten(),
        last_edited: s("last_edited_time"),
    })
}

fn s_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

pub fn parse_search(json: &str) -> Result<SearchPage, NotionError> {
    let v = parse(json)?;
    let results = v["results"].as_array().ok_or(NotionError::Malformed)?;
    Ok(SearchPage {
        items: results.iter().filter_map(item).collect(),
        next_cursor: s_of(&v["next_cursor"]),
        has_more: v["has_more"].as_bool().unwrap_or(false),
    })
}

pub fn parse_me(json: &str) -> Result<BotInfo, NotionError> {
    let v = parse(json)?;
    Ok(BotInfo { name: s_of(&v["name"]).ok_or(NotionError::Malformed)?, workspace: s_of(&v["bot"]["workspace_name"]) })
}

pub fn parse_data_source_title_property(json: &str) -> Option<String> {
    let v = parse(json).ok()?;
    v["properties"].as_object()?.iter().find(|(_, p)| p["type"] == "title").map(|(k, _)| k.clone())
}

pub fn parse_created_page(json: &str) -> Result<(String, String), NotionError> {
    let v = parse(json)?;
    Ok((s_of(&v["id"]).ok_or(NotionError::Malformed)?, s_of(&v["url"]).ok_or(NotionError::Malformed)?))
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotionError {
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited { retry_after_secs: Option<u64> },
    Validation(String),
    Server(u16),
    Network,
    Malformed,
}

impl NotionError {
    /// Map an HTTP status and error body (`{"code","message"}`); the code wins over the status.
    pub fn from_status(status: u16, body: &str) -> Self {
        let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let message = v["message"].as_str().unwrap_or_default().to_string();
        match v["code"].as_str() {
            Some("unauthorized") => return Self::Unauthorized,
            Some("restricted_resource") => return Self::Forbidden,
            Some("object_not_found") => return Self::NotFound,
            Some("rate_limited") => return Self::RateLimited { retry_after_secs: None },
            Some("validation_error") => return Self::Validation(message),
            _ => {}
        }
        match status {
            0 => Self::Network,
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            429 => Self::RateLimited { retry_after_secs: None },
            400..=499 => Self::Validation(message),
            500..=599 => Self::Server(status),
            _ => Self::Malformed,
        }
    }

    /// Attach the `Retry-After` seconds (only meaningful for `RateLimited`).
    pub fn with_retry_after(self, secs: Option<u64>) -> Self {
        match self {
            Self::RateLimited { .. } => Self::RateLimited { retry_after_secs: secs },
            e => e,
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Unauthorized => "Notion rejected the token. Paste a new one in Settings → Integrations.".into(),
            Self::Forbidden => {
                "The Notion integration lacks permission for this. In Notion open the page, ••• → Connections, and add Ferrocast.".into()
            }
            Self::NotFound => "Notion cannot see that page. Share it with your integration (••• → Connections).".into(),
            Self::RateLimited { .. } => "Notion is rate limiting requests. Try again in a few seconds.".into(),
            Self::Network => "Could not reach Notion. Check your connection.".into(),
            Self::Server(n) => format!("Notion had a problem (HTTP {n}). Try again."),
            Self::Malformed => "Notion sent a response Ferrocast could not read.".into(),
            Self::Validation(m) => format!("Notion did not accept that: {}", m.chars().take(160).collect::<String>()),
        }
    }
}

#[cfg(test)]
mod tests {
    // Fixtures are written by hand from the documented response shapes; they are NOT captured from the live API.
    use super::*;
    use serde_json::{json, Value};

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn curl_config_carries_token_and_markers_in_the_config() {
        let c = curl_config("secret_tok", "POST", "https://api.notion.com/v1/search", Some("{\"a\":\"b\\\"c\"}\n"));
        assert!(c.contains("url = \"https://api.notion.com/v1/search\"\n"));
        assert!(c.contains("request = \"POST\"\n"));
        assert!(c.contains("header = \"Authorization: Bearer secret_tok\"\n"));
        assert!(c.contains(&format!("header = \"Notion-Version: {NOTION_VERSION}\"\n")));
        assert!(c.contains("header = \"Content-Type: application/json\"\n"));
        assert!(c.contains("max-time = 20\n"));
        assert!(c.contains("write-out = \"\\n%{http_code}\"\n"));
        assert!(c.contains("data = \"{\\\"a\\\":\\\"b\\\\\\\"c\\\"}\\n\"\n"));
        let get = curl_config("t", "GET", "https://x", None);
        assert!(!get.contains("data ="));
        assert!(get.contains("request = \"GET\""));
    }

    #[test]
    fn split_status_follows_the_curl_marker() {
        assert_eq!(split_status("{\"a\":1}\n200"), ("{\"a\":1}", 200));
        assert_eq!(split_status("\n404"), ("", 404));
        assert_eq!(split_status("{\"a\":1}"), ("{\"a\":1}", 0));
        assert_eq!(split_status(""), ("", 0));
        assert_eq!(from_output("{}\n0").unwrap_err(), NotionError::Network);
        assert_eq!(from_output("oops").unwrap_err(), NotionError::Network);
        assert_eq!(from_output("{\"ok\":1}\n200").unwrap(), "{\"ok\":1}");
        assert_eq!(from_output("{\"code\":\"unauthorized\"}\n401").unwrap_err(), NotionError::Unauthorized);
    }

    #[test]
    fn search_body_sorts_only_when_the_query_is_empty() {
        let b = v(&search_body("", None, 20));
        assert_eq!(b["sort"], json!({"timestamp":"last_edited_time","direction":"descending"}));
        assert_eq!(b["page_size"], 20);
        assert!(b.get("filter").is_none() && b.get("start_cursor").is_none() && b.get("query").is_none());
        let b = v(&search_body("héllo 日本", Some("cur"), 5));
        assert_eq!(b["query"], "héllo 日本");
        assert_eq!(b["start_cursor"], "cur");
        assert!(b.get("sort").is_none() && b.get("filter").is_none());
        assert_eq!(b["page_size"], 5);
    }

    #[test]
    fn create_page_body_shapes() {
        let b = v(&create_page_body(&Parent::Page("p1".into()), "Name", "T \"x\"", "# md\nbody"));
        assert_eq!(b["parent"], json!({"page_id":"p1"}));
        assert_eq!(b["properties"], json!({"title":{"title":[{"text":{"content":"T \"x\""}}]}}));
        assert_eq!(b["markdown"], "# md\nbody");
        assert!(b.get("children").is_none());
        let b = v(&create_page_body(&Parent::DataSource("d1".into()), "Name", "T", "m"));
        assert_eq!(b["parent"], json!({"data_source_id":"d1"}));
        assert_eq!(b["properties"], json!({"Name":{"title":[{"text":{"content":"T"}}]}}));
    }

    #[test]
    fn markdown_bodies() {
        assert_eq!(
            v(&append_markdown_body("hi")),
            json!({"type":"insert_content","insert_content":{"content":"hi","position":{"type":"end"}}})
        );
        assert_eq!(v(&replace_markdown_body("new")), json!({"type":"replace_content","replace_content":{"content":"new"}}));
    }

    #[test]
    fn urls() {
        assert_eq!(search_url(), "https://api.notion.com/v1/search");
        assert_eq!(pages_url(), "https://api.notion.com/v1/pages");
        assert_eq!(page_markdown_url("abc"), "https://api.notion.com/v1/pages/abc/markdown");
        assert_eq!(me_url(), "https://api.notion.com/v1/users/me");
        assert_eq!(data_source_url("ds"), "https://api.notion.com/v1/data_sources/ds");
    }

    const SEARCH: &str = r#"{"object":"list","has_more":true,"next_cursor":"c2","results":[
      {"object":"page","id":"p1","url":"https://app.notion.com/p/p1","last_edited_time":"2026-06-01T10:00:00.000Z",
       "icon":{"type":"emoji","emoji":"📄"},
       "properties":{"Tags":{"type":"multi_select"},"Nombre":{"type":"title","title":[{"plain_text":"Plan "},{"plain_text":"日本 ✓"}]}}},
      {"object":"data_source","id":"d1","url":"https://app.notion.com/p/d1","title":[{"plain_text":"Tasks"}],
       "icon":{"type":"external","external":{"url":"https://x"}}},
      {"object":"database","id":"x"},
      {"object":"page","id":"p2","url":"https://app.notion.com/p/p2","properties":{"title":{"type":"title","title":[]}}},
      {"object":"data_source","id":"d2","url":"u"}
    ]}"#;

    #[test]
    fn parse_search_pages_and_data_sources() {
        let s = parse_search(SEARCH).unwrap();
        assert!(s.has_more);
        assert_eq!(s.next_cursor.as_deref(), Some("c2"));
        assert_eq!(s.items.len(), 4);
        let p = &s.items[0];
        assert_eq!((p.id.as_str(), p.kind, p.title.as_str()), ("p1", ItemKind::Page, "Plan 日本 ✓"));
        assert_eq!(p.url, "https://app.notion.com/p/p1");
        assert_eq!(p.icon_emoji.as_deref(), Some("📄"));
        assert_eq!(p.last_edited.as_deref(), Some("2026-06-01T10:00:00.000Z"));
        let d = &s.items[1];
        assert_eq!((d.kind, d.title.as_str(), d.icon_emoji.clone(), d.last_edited.clone()), (ItemKind::DataSource, "Tasks", None, None));
        assert_eq!(s.items[2].title, "Untitled");
        assert_eq!(s.items[3].title, "Untitled");
        assert_eq!(s.items[3].id, "d2");
    }

    #[test]
    fn parse_search_empty_and_end_of_list() {
        let s = parse_search(r#"{"results":[],"has_more":false,"next_cursor":null}"#).unwrap();
        assert!(s.items.is_empty() && !s.has_more && s.next_cursor.is_none());
        let s = parse_search(r#"{"results":[]}"#).unwrap();
        assert!(!s.has_more && s.next_cursor.is_none());
    }

    #[test]
    fn parsers_report_malformed() {
        assert_eq!(parse_search("not json").unwrap_err(), NotionError::Malformed);
        assert_eq!(parse_search(r#"{"x":1}"#).unwrap_err(), NotionError::Malformed);
        assert_eq!(parse_me("").unwrap_err(), NotionError::Malformed);
        assert_eq!(parse_created_page("{}").unwrap_err(), NotionError::Malformed);
        assert_eq!(parse_created_page("[").unwrap_err(), NotionError::Malformed);
    }

    #[test]
    fn parse_me_and_created_page() {
        let b = parse_me(r#"{"object":"user","type":"bot","name":"Ferrocast","bot":{"workspace_name":"Acme ñ"}}"#).unwrap();
        assert_eq!(b, BotInfo { name: "Ferrocast".into(), workspace: Some("Acme ñ".into()) });
        let b = parse_me(r#"{"name":"Bot"}"#).unwrap();
        assert_eq!(b, BotInfo { name: "Bot".into(), workspace: None });
        assert_eq!(
            parse_created_page(r#"{"object":"page","id":"n1","url":"https://app.notion.com/p/n1"}"#).unwrap(),
            ("n1".into(), "https://app.notion.com/p/n1".into())
        );
    }

    #[test]
    fn data_source_title_property() {
        let j = r#"{"object":"data_source","properties":{"Status":{"type":"select"},"Task name":{"type":"title","id":"title"}}}"#;
        assert_eq!(parse_data_source_title_property(j).as_deref(), Some("Task name"));
        assert_eq!(parse_data_source_title_property(r#"{"properties":{"A":{"type":"select"}}}"#), None);
        assert_eq!(parse_data_source_title_property("nope"), None);
    }

    #[test]
    fn error_mapping_prefers_code_then_status() {
        let c = |code: &str| format!("{{\"object\":\"error\",\"code\":\"{code}\",\"message\":\"m\"}}");
        assert_eq!(NotionError::from_status(401, &c("unauthorized")), NotionError::Unauthorized);
        assert_eq!(NotionError::from_status(403, &c("restricted_resource")), NotionError::Forbidden);
        assert_eq!(NotionError::from_status(404, &c("object_not_found")), NotionError::NotFound);
        assert_eq!(NotionError::from_status(429, &c("rate_limited")), NotionError::RateLimited { retry_after_secs: None });
        assert_eq!(NotionError::from_status(400, &c("validation_error")), NotionError::Validation("m".into()));
        assert_eq!(NotionError::from_status(401, ""), NotionError::Unauthorized);
        assert_eq!(NotionError::from_status(403, "x"), NotionError::Forbidden);
        assert_eq!(NotionError::from_status(404, "{}"), NotionError::NotFound);
        assert_eq!(NotionError::from_status(429, ""), NotionError::RateLimited { retry_after_secs: None });
        assert_eq!(NotionError::from_status(400, ""), NotionError::Validation(String::new()));
        assert_eq!(NotionError::from_status(503, ""), NotionError::Server(503));
        assert_eq!(NotionError::from_status(0, ""), NotionError::Network);
        assert_eq!(NotionError::from_status(302, ""), NotionError::Malformed);
        assert_eq!(
            NotionError::RateLimited { retry_after_secs: None }.with_retry_after(Some(7)),
            NotionError::RateLimited { retry_after_secs: Some(7) }
        );
        assert_eq!(NotionError::NotFound.with_retry_after(Some(7)), NotionError::NotFound);
    }

    #[test]
    fn user_messages_are_exact() {
        assert_eq!(NotionError::Unauthorized.user_message(), "Notion rejected the token. Paste a new one in Settings → Integrations.");
        assert_eq!(
            NotionError::Forbidden.user_message(),
            "The Notion integration lacks permission for this. In Notion open the page, ••• → Connections, and add Ferrocast."
        );
        assert_eq!(
            NotionError::NotFound.user_message(),
            "Notion cannot see that page. Share it with your integration (••• → Connections)."
        );
        assert_eq!(
            NotionError::RateLimited { retry_after_secs: Some(3) }.user_message(),
            "Notion is rate limiting requests. Try again in a few seconds."
        );
        assert_eq!(NotionError::Network.user_message(), "Could not reach Notion. Check your connection.");
        assert_eq!(NotionError::Server(502).user_message(), "Notion had a problem (HTTP 502). Try again.");
        assert_eq!(NotionError::Malformed.user_message(), "Notion sent a response Ferrocast could not read.");
        assert_eq!(NotionError::Validation("bad".into()).user_message(), "Notion did not accept that: bad");
        let long = "é".repeat(300);
        assert_eq!(NotionError::Validation(long).user_message(), format!("Notion did not accept that: {}", "é".repeat(160)));
    }

    #[test]
    fn the_token_only_appears_in_curl_config() {
        let tok = "secret_SENTINEL";
        let all = [
            search_body("q", Some("c"), 20),
            search_body("", None, 20),
            create_page_body(&Parent::Page("p".into()), "t", "title", "md"),
            create_page_body(&Parent::DataSource("d".into()), "Name", "title", "md"),
            append_markdown_body("x"),
            replace_markdown_body("x"),
            search_url(),
            pages_url(),
            page_markdown_url("i"),
            me_url(),
            data_source_url("i"),
        ];
        for s in all {
            assert!(!s.contains(tok));
        }
        for e in [
            NotionError::Unauthorized,
            NotionError::Forbidden,
            NotionError::NotFound,
            NotionError::RateLimited { retry_after_secs: Some(1) },
            NotionError::Validation("m".into()),
            NotionError::Server(500),
            NotionError::Network,
            NotionError::Malformed,
        ] {
            assert!(!e.user_message().contains(tok) && !format!("{e:?}").contains(tok));
        }
        assert!(curl_config(tok, "GET", &me_url(), None).contains(tok));
    }
}
