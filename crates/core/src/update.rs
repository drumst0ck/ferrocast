//! Release feed parsing, version comparison and update planning. Pure: no
//! network or filesystem access; the app crate does the I/O.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::path::Path;

/// `X.Y.Z` with an optional `-pre` suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) if !p.is_empty() => (c, Some(p.to_string())),
            Some(_) => return None,
            None => (s, None),
        };
        let mut it = core.split('.');
        let mut num = || it.next()?.parse::<u64>().ok();
        let (major, minor, patch) = (num()?, num()?, num()?);
        if it.next().is_some() {
            return None;
        }
        Some(Version { major, minor, patch, pre })
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(o.major, o.minor, o.patch)).then_with(|| match (&self.pre, &o.pre) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub zip_url: String,
    pub sums_url: String,
    pub notes: String,
    pub html_url: String,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

/// Parse the GitHub `/releases/latest` JSON into a stable, installable release.
/// The outcome of asking GitHub for the latest release. `curl_ok` is false when curl itself failed
/// (no network, DNS, timeout); `status` is the HTTP code. A 404 means the repository has no
/// published release yet, which is "nothing to update to", not an error.
pub fn feed_result(curl_ok: bool, status: u16, body: &str) -> Result<Option<Release>, String> {
    if !curl_ok {
        return Err("No connection to GitHub. Check your network and try again.".into());
    }
    match status {
        200 => Ok(parse_latest(body)),
        404 => Ok(None),
        403 | 429 => Err("GitHub rate limit reached. Try again in an hour.".into()),
        s => Err(format!("GitHub answered HTTP {s}. Try again later.")),
    }
}

/// Split curl output produced with `-w "\n%{http_code}"` into the body and the status code
/// (0 when the last line is not a number).
pub fn split_status(out: &str) -> (&str, u16) {
    let (body, code) = out.rsplit_once('\n').unwrap_or(("", out));
    match code.trim().parse::<u16>() {
        Ok(c) => (body, c),
        Err(_) => (out, 0),
    }
}

/// After a failed automatic check, wait an hour before trying again (never every few minutes).
pub fn retry_due(last_failure: Option<i64>, now: i64) -> bool {
    last_failure.is_none_or(|f| now - f >= 3_600)
}

pub fn parse_latest(json: &str) -> Option<Release> {
    let raw: RawRelease = serde_json::from_str(json).ok()?;
    if raw.draft || raw.prerelease {
        return None;
    }
    let version = Version::parse(&raw.tag_name)?;
    let zip_name = format!("Ferrocast-{}.zip", raw.tag_name.trim().trim_start_matches('v'));
    let find = |name: &str| raw.assets.iter().find(|a| a.name == name).map(|a| a.browser_download_url.clone());
    Some(Release {
        zip_url: find(&zip_name)?,
        sums_url: find("SHA256SUMS.txt")?,
        version,
        tag: raw.tag_name,
        notes: raw.body.unwrap_or_default(),
        html_url: raw.html_url,
    })
}

/// Look up `file_name` in `shasum -a 256` output; returns lowercase hex.
pub fn sum_for(sums_txt: &str, file_name: &str) -> Option<String> {
    sums_txt.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        (name == file_name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Brew,
    Direct,
    Dev,
}

pub fn channel_of(exe: &Path) -> Channel {
    let p = exe.to_string_lossy();
    if p.contains("/Cellar/") {
        Channel::Brew
    } else if p.contains("/target/") || p.contains("/dist/") {
        Channel::Dev
    } else {
        Channel::Direct
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    UpToDate,
    Available(Release),
    Skipped(Release),
}

pub fn decide(current: &Version, latest: Option<Release>, skipped: Option<&str>) -> Decision {
    match latest {
        Some(r) if r.version > *current => {
            let ver = r.tag.trim_start_matches('v');
            if skipped.is_some_and(|s| s.trim_start_matches('v') == ver) {
                Decision::Skipped(r)
            } else {
                Decision::Available(r)
            }
        }
        _ => Decision::UpToDate,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Download,
    BrewUpgrade,
    Refuse(String),
}

pub fn plan_for(channel: Channel, parent_writable: bool) -> Plan {
    match channel {
        Channel::Brew => Plan::BrewUpgrade,
        Channel::Dev => Plan::Refuse("Development build: updates are checked but not installed".into()),
        Channel::Direct if !parent_writable => {
            Plan::Refuse("The app folder is not writable; download the DMG from the release page".into())
        }
        Channel::Direct => Plan::Download,
    }
}

/// True when a 24 h check is due (also when the clock went backwards).
pub fn due(last_check: Option<i64>, now: i64) -> bool {
    last_check.is_none_or(|l| now < l || now - l >= 86_400)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    sha256_hex(bytes) == expected_hex.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {

    #[test]
    fn feed_result_tells_no_release_rate_limit_and_offline_apart() {
        // GitHub answers 404 when the repository has no published release yet: nothing to update to
        assert_eq!(feed_result(true, 404, "{\"message\":\"Not Found\"}"), Ok(None));
        let e = feed_result(true, 403, "{}").unwrap_err();
        assert!(e.contains("rate limit"), "{e}");
        assert!(feed_result(true, 429, "").unwrap_err().contains("rate limit"));
        let e = feed_result(true, 500, "").unwrap_err();
        assert!(e.contains("500"), "{e}");
        let e = feed_result(false, 0, "").unwrap_err();
        assert!(e.contains("No connection"), "{e}");
        // 200 with an unusable body (no assets yet) is "no release to offer", not an error
        assert_eq!(feed_result(true, 200, "{}"), Ok(None));
    }

    #[test]
    fn split_status_reads_the_code_curl_appends() {
        assert_eq!(split_status("{\"a\":1}\n200"), ("{\"a\":1}", 200));
        assert_eq!(split_status("404"), ("", 404));
        assert_eq!(split_status("garbage"), ("garbage", 0));
    }

    #[test]
    fn retry_waits_an_hour_after_a_failed_check() {
        assert!(retry_due(None, 1_000));
        assert!(!retry_due(Some(1_000), 1_000 + 3_599));
        assert!(retry_due(Some(1_000), 1_000 + 3_600));
    }
    use super::*;
    use std::path::Path;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn rel(ver: &str) -> Release {
        Release {
            version: v(ver),
            tag: format!("v{ver}"),
            zip_url: "z".into(),
            sums_url: "s".into(),
            notes: String::new(),
            html_url: "h".into(),
        }
    }

    #[test]
    fn version_parse_and_order() {
        assert!(v("0.1.0") < v("0.2.0"));
        assert!(v("0.9.9") < v("0.10.0"));
        assert!(v("1.0.0-beta.1") < v("1.0.0"));
        assert!(v("1.0.0-alpha") < v("1.0.0-beta"));
        assert!(v("1.0.0") < v("1.0.1-rc1"));
        assert_eq!(v("v1.2.3"), v("1.2.3"));
        for bad in ["", "1.2", "a.b.c", "1.2.3.4", "v", "1.2.x"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    const FEED: &str = r#"{
      "tag_name": "v0.2.0", "body": "notes here", "html_url": "https://github.com/x/y/releases/tag/v0.2.0",
      "draft": false, "prerelease": false,
      "assets": [
        {"name": "Ferrocast-0.2.0.dmg", "browser_download_url": "https://d/dmg"},
        {"name": "Ferrocast-0.2.0.zip", "browser_download_url": "https://d/zip"},
        {"name": "SHA256SUMS.txt", "browser_download_url": "https://d/sums"}
      ]}"#;

    #[test]
    fn parse_latest_cases() {
        let r = parse_latest(FEED).unwrap();
        assert_eq!(r.version, v("0.2.0"));
        assert_eq!(r.tag, "v0.2.0");
        assert_eq!(r.zip_url, "https://d/zip");
        assert_eq!(r.sums_url, "https://d/sums");
        assert_eq!(r.notes, "notes here");
        assert!(r.html_url.ends_with("v0.2.0"));
        let no_sums = FEED.replace("SHA256SUMS.txt", "other.txt");
        assert!(parse_latest(&no_sums).is_none());
        let no_zip = FEED.replace("Ferrocast-0.2.0.zip", "Ferrocast-0.3.0.zip");
        assert!(parse_latest(&no_zip).is_none());
        assert!(parse_latest(&FEED.replace("\"prerelease\": false", "\"prerelease\": true")).is_none());
        assert!(parse_latest(&FEED.replace("\"draft\": false", "\"draft\": true")).is_none());
        assert!(parse_latest(&FEED.replace("v0.2.0\", \"body", "nope\", \"body")).is_none());
        assert!(parse_latest("not json").is_none());
    }

    #[test]
    fn sums() {
        let h = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let up = h.to_uppercase();
        let txt = format!("{}  Ferrocast-0.2.0.dmg\n{h}  Ferrocast-0.2.0.zip\n{up} *star.zip\n", "0".repeat(64));
        assert_eq!(sum_for(&txt, "Ferrocast-0.2.0.zip").as_deref(), Some(h));
        assert_eq!(sum_for(&txt, "star.zip").as_deref(), Some(h));
        assert!(sum_for(&txt, "missing.zip").is_none());
        assert!(sum_for("abc  f.zip", "f.zip").is_none());
    }

    #[test]
    fn channels() {
        let c = |p: &str| channel_of(Path::new(p));
        assert_eq!(c("/opt/homebrew/Cellar/ferrocast/0.1.0/Ferrocast.app/Contents/MacOS/ferrocast"), Channel::Brew);
        assert_eq!(c("/Users/x/dev/ferrocast/target/release/ferrocast"), Channel::Dev);
        assert_eq!(c("/Users/x/dev/ferrocast/dist/Ferrocast.app/Contents/MacOS/ferrocast"), Channel::Dev);
        assert_eq!(c("/Applications/Ferrocast.app/Contents/MacOS/ferrocast"), Channel::Direct);
    }

    #[test]
    fn decisions() {
        assert_eq!(decide(&v("0.1.0"), None, None), Decision::UpToDate);
        assert_eq!(decide(&v("0.2.0"), Some(rel("0.2.0")), None), Decision::UpToDate);
        assert_eq!(decide(&v("0.3.0"), Some(rel("0.2.0")), None), Decision::UpToDate);
        assert_eq!(decide(&v("0.1.0"), Some(rel("0.2.0")), None), Decision::Available(rel("0.2.0")));
        assert_eq!(decide(&v("0.1.0"), Some(rel("0.2.0")), Some("0.1.5")), Decision::Available(rel("0.2.0")));
        assert_eq!(decide(&v("0.1.0"), Some(rel("0.2.0")), Some("0.2.0")), Decision::Skipped(rel("0.2.0")));
    }

    #[test]
    fn plans() {
        assert_eq!(plan_for(Channel::Brew, true), Plan::BrewUpgrade);
        assert_eq!(plan_for(Channel::Brew, false), Plan::BrewUpgrade);
        assert!(matches!(plan_for(Channel::Dev, true), Plan::Refuse(m) if m.contains("Development build")));
        assert!(matches!(plan_for(Channel::Direct, false), Plan::Refuse(m) if m.contains("not writable") && m.contains("DMG")));
        assert_eq!(plan_for(Channel::Direct, true), Plan::Download);
    }

    #[test]
    fn cadence() {
        assert!(due(None, 5));
        assert!(!due(Some(1000), 1000 + 86_399));
        assert!(due(Some(1000), 1000 + 86_400));
        assert!(due(Some(2000), 1000));
    }

    #[test]
    fn hashing() {
        let h = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_hex(b"abc"), h);
        assert!(verify_sha256(b"abc", h));
        assert!(verify_sha256(b"abc", &h.to_uppercase()));
        assert!(!verify_sha256(b"abd", h));
    }

    #[test]
    fn old_settings_load_with_update_defaults() {
        let s: crate::models::Settings = serde_json::from_str(r#"{"theme":"Dark"}"#).unwrap();
        assert!(s.update_check);
        assert_eq!(s.update_install, crate::models::UpdateInstall::AskFirst);
        assert_eq!(s.update_skipped, None);
        assert_eq!(s.update_last_check, None);
    }
}
