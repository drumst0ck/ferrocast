//! Per-extension persistent key/value store (LocalStorage) and saved preferences, as JSON files
//! under `<data>/extensions/<extension>/`.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Folder of one extension's data. The name is reduced to `[A-Za-z0-9._-]` so it cannot escape.
pub fn extension_dir(data_dir: &Path, extension: &str) -> PathBuf {
    let safe: String = extension.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect();
    let safe = if safe.is_empty() || safe.chars().all(|c| c == '.') { "_".to_string() } else { safe };
    data_dir.join("extensions").join(safe)
}

pub fn support_dir(data_dir: &Path, extension: &str) -> PathBuf {
    extension_dir(data_dir, extension).join("support")
}

fn read_map(p: &Path) -> Map<String, Value> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Map<String, Value>>(&b).ok()).unwrap_or_default()
}

fn write_map(p: &Path, m: &Map<String, Value>) -> std::io::Result<()> {
    write_map_mode(p, m, None)
}

/// Write through a temp file; `private` gives the file mode 0600 (owner only) from the start.
fn write_map_mode(p: &Path, m: &Map<String, Value>, private: Option<u32>) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = p.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(mode) = private {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = private;
    opts.open(&tmp)?.write_all(&serde_json::to_vec(m).unwrap_or_default())?;
    std::fs::rename(&tmp, p)
}

/// Saved preference values (`preferences.json`), overriding manifest defaults.
pub fn load_preferences(data_dir: &Path, extension: &str) -> Map<String, Value> {
    read_map(&extension_dir(data_dir, extension).join("preferences.json"))
}

/// Save the preference values the user edited (`preferences.json`), replacing the file.
pub fn save_preferences(data_dir: &Path, extension: &str, values: &Map<String, Value>) -> std::io::Result<()> {
    write_map_mode(&extension_dir(data_dir, extension).join("preferences.json"), values, Some(0o600))
}

/// Handle a `localStorage.*` call; returns the JSON the runtime gets back.
pub fn local_storage(data_dir: &Path, extension: &str, method: &str, params: &Value) -> Result<Value, String> {
    let file = extension_dir(data_dir, extension).join("localstorage.json");
    let mut m = read_map(&file);
    let key = params.get("key").and_then(Value::as_str).unwrap_or("");
    let out = match method {
        "localStorage.getItem" => return Ok(m.get(key).cloned().unwrap_or(Value::Null)),
        "localStorage.allItems" => return Ok(Value::Object(m)),
        "localStorage.setItem" => {
            m.insert(key.to_string(), params.get("value").cloned().unwrap_or(Value::Null));
            Value::Null
        }
        "localStorage.removeItem" => {
            m.remove(key);
            Value::Null
        }
        "localStorage.clear" => {
            m.clear();
            Value::Null
        }
        other => return Err(format!("unknown call {other}")),
    };
    write_map(&file, &m).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saved_preferences_round_trip_per_extension() {
        let d = std::env::temp_dir().join(format!("fc-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut m = Map::new();
        m.insert("token".into(), json!("abc"));
        m.insert("pretty".into(), json!(true));
        save_preferences(&d, "a", &m).unwrap();
        assert_eq!(load_preferences(&d, "a"), m);
        assert!(load_preferences(&d, "b").is_empty());
        m.insert("token".into(), json!("xyz"));
        save_preferences(&d, "a", &m).unwrap();
        assert_eq!(load_preferences(&d, "a")["token"], json!("xyz"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn preferences_file_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("fc-prefs-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut m = Map::new();
        m.insert("token".into(), json!("abc"));
        save_preferences(&d, "a", &m).unwrap();
        save_preferences(&d, "a", &m).unwrap();
        let f = extension_dir(&d, "a").join("preferences.json");
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn local_storage_round_trips_and_is_per_extension() {
        let d = std::env::temp_dir().join(format!("fc-ls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let set = |ext: &str, k: &str, v: Value| local_storage(&d, ext, "localStorage.setItem", &json!({"key":k,"value":v})).unwrap();
        set("a", "k", json!("v1"));
        set("b", "k", json!("v2"));
        assert_eq!(local_storage(&d, "a", "localStorage.getItem", &json!({"key":"k"})).unwrap(), json!("v1"));
        assert_eq!(local_storage(&d, "b", "localStorage.getItem", &json!({"key":"k"})).unwrap(), json!("v2"));
        assert_eq!(local_storage(&d, "a", "localStorage.getItem", &json!({"key":"nope"})).unwrap(), Value::Null);
        assert_eq!(local_storage(&d, "a", "localStorage.allItems", &json!({})).unwrap(), json!({"k":"v1"}));
        local_storage(&d, "a", "localStorage.removeItem", &json!({"key":"k"})).unwrap();
        assert_eq!(local_storage(&d, "a", "localStorage.allItems", &json!({})).unwrap(), json!({}));
        assert!(local_storage(&d, "a", "localStorage.nope", &json!({})).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn extension_dir_cannot_escape() {
        let d = Path::new("/data");
        assert_eq!(extension_dir(d, "../../etc"), d.join("extensions").join(".._.._etc"));
        assert_eq!(extension_dir(d, ".."), d.join("extensions").join("_"));
        assert_eq!(extension_dir(d, ""), d.join("extensions").join("_"));
    }
}
