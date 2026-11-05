//! Per-extension persistent key/value store (LocalStorage) and saved preferences, as JSON files
//! under `<data>/extensions/<extension>/`.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

/// Handle one `localStorage.*` call against the file, without any cache (reads and, for a change,
/// writes the whole file). The app uses [`LocalStores`] instead; this is the simple, synchronous form.
pub fn local_storage(data_dir: &Path, extension: &str, method: &str, params: &Value) -> Result<Value, String> {
    let (out, persist) = LocalStores::default().call(data_dir, extension, method, params)?;
    if let Some(p) = persist {
        p.run()?;
    }
    Ok(out)
}

struct State {
    map: Map<String, Value>,
    /// The file exists but cannot be read as a JSON object: it is never overwritten.
    unreadable: bool,
    /// Bumped by every change; `persisted` is the version the file holds.
    version: u64,
    persisted: u64,
    /// A thread is writing this file (it also covers the changes made while it runs).
    flushing: bool,
}

struct Cell {
    state: Mutex<State>,
    /// Held while the file is written, so two writers cannot land out of order.
    writer: Mutex<()>,
}

/// In-memory copy of each extension's LocalStorage. The file is read once per extension; a change is
/// applied in memory (so reads and ordering are immediate) and written afterwards through a
/// [`Persist`], which the caller runs off the UI thread. Writes made meanwhile are coalesced.
#[derive(Clone, Default)]
pub struct LocalStores(Arc<Mutex<HashMap<PathBuf, Arc<Cell>>>>);

/// A pending write of one extension's LocalStorage file.
pub struct Persist {
    cell: Arc<Cell>,
    file: PathBuf,
    version: u64,
}

impl LocalStores {
    fn cell(&self, file: &Path) -> Arc<Cell> {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.entry(file.to_path_buf())
            .or_insert_with(|| {
                let (map, unreadable) = match std::fs::read(file) {
                    Ok(b) => match serde_json::from_slice::<Map<String, Value>>(&b) {
                        Ok(m) => (m, false),
                        Err(_) => (Map::new(), true),
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Map::new(), false),
                    Err(_) => (Map::new(), true),
                };
                Arc::new(Cell {
                    state: Mutex::new(State { map, unreadable, version: 0, persisted: 0, flushing: false }),
                    writer: Mutex::new(()),
                })
            })
            .clone()
    }

    /// Apply a `localStorage.*` call in memory. Returns the JSON the runtime gets back and, for a
    /// change, the write that makes it durable. A change to an unreadable file is refused.
    pub fn call(&self, data_dir: &Path, extension: &str, method: &str, params: &Value) -> Result<(Value, Option<Persist>), String> {
        let file = extension_dir(data_dir, extension).join("localstorage.json");
        let cell = self.cell(&file);
        let key = params.get("key").and_then(Value::as_str).unwrap_or("");
        let mut st = cell.state.lock().unwrap_or_else(|e| e.into_inner());
        match method {
            "localStorage.getItem" => return Ok((st.map.get(key).cloned().unwrap_or(Value::Null), None)),
            "localStorage.allItems" => return Ok((Value::Object(st.map.clone()), None)),
            "localStorage.setItem" | "localStorage.removeItem" | "localStorage.clear" => {}
            other => return Err(format!("unknown call {other}")),
        }
        if st.unreadable {
            return Err(
                "this extension's LocalStorage file is unreadable (not valid JSON); it was left as it is, so nothing was lost".into()
            );
        }
        match method {
            "localStorage.setItem" => {
                st.map.insert(key.to_string(), params.get("value").cloned().unwrap_or(Value::Null));
            }
            "localStorage.removeItem" => {
                st.map.remove(key);
            }
            _ => st.map.clear(),
        }
        st.version += 1;
        let version = st.version;
        drop(st);
        Ok((Value::Null, Some(Persist { cell, file, version })))
    }
}

impl Persist {
    /// Write the file on a background thread, one thread per extension however many changes come
    /// in; `on_error` gets the reason when the disk write fails (the changes stay in memory).
    pub fn spawn(self, on_error: impl FnOnce(String) + Send + 'static) {
        {
            let mut st = self.cell.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.flushing {
                return; // the running thread loops until it has written the newest version
            }
            st.flushing = true;
        }
        std::thread::spawn(move || loop {
            let result = Persist { cell: self.cell.clone(), file: self.file.clone(), version: u64::MAX }.run();
            let mut st = self.cell.state.lock().unwrap_or_else(|e| e.into_inner());
            match result {
                Err(e) => {
                    st.flushing = false;
                    drop(st);
                    on_error(e);
                    return;
                }
                Ok(()) if st.persisted >= st.version => {
                    st.flushing = false;
                    return;
                }
                Ok(()) => {}
            }
        });
    }

    /// Write the file (disk I/O: call it off the UI thread). A write that a newer one already covers is skipped.
    pub fn run(self) -> Result<(), String> {
        let _w = self.cell.writer.lock().unwrap_or_else(|e| e.into_inner());
        let (snapshot, version) = {
            let st = self.cell.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.persisted >= self.version {
                return Ok(());
            }
            (st.map.clone(), st.version)
        };
        write_map(&self.file, &snapshot).map_err(|e| e.to_string())?;
        self.cell.state.lock().unwrap_or_else(|e| e.into_inner()).persisted = version;
        Ok(())
    }
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
    fn a_corrupt_local_storage_file_is_not_overwritten_by_a_write() {
        let d = std::env::temp_dir().join(format!("fc-ls-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = extension_dir(&d, "a").join("localstorage.json");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, b"{\"keep\": \"me\", oops").unwrap();
        for (m, p) in [
            ("localStorage.setItem", json!({"key":"k","value":"v"})),
            ("localStorage.removeItem", json!({"key":"keep"})),
            ("localStorage.clear", json!({})),
        ] {
            let e = local_storage(&d, "a", m, &p).unwrap_err();
            assert!(e.contains("unreadable"), "{m}: {e}");
            assert_eq!(std::fs::read(&f).unwrap(), b"{\"keep\": \"me\", oops", "{m} left the file alone");
        }
        // reads see an empty store but never touch the file
        assert_eq!(local_storage(&d, "a", "localStorage.allItems", &json!({})).unwrap(), json!({}));
        assert_eq!(std::fs::read(&f).unwrap(), b"{\"keep\": \"me\", oops");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_file_is_read_once_and_a_burst_of_writes_is_coalesced() {
        let d = std::env::temp_dir().join(format!("fc-ls-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = extension_dir(&d, "a").join("localstorage.json");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, br#"{"old":1}"#).unwrap();
        let st = LocalStores::default();
        let mut pending = vec![];
        for i in 0..50 {
            let (_, p) = st.call(&d, "a", "localStorage.setItem", &json!({"key":format!("k{i}"),"value":i})).unwrap();
            pending.push(p.expect("a change has a write"));
        }
        // nothing written yet, yet reads already see every change and the pre-existing key
        assert_eq!(std::fs::read(&f).unwrap(), br#"{"old":1}"#);
        let (all, p) = st.call(&d, "a", "localStorage.allItems", &json!({})).unwrap();
        assert!(p.is_none());
        assert_eq!(all.as_object().unwrap().len(), 51);
        // the last write covers the others; the older ones are skipped, and none can undo it
        let last = pending.pop().unwrap();
        last.run().unwrap();
        let on_disk: Map<String, Value> = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
        assert_eq!(on_disk.len(), 51);
        std::fs::write(&f, b"tampered").unwrap();
        for p in pending {
            p.run().unwrap();
        }
        assert_eq!(std::fs::read(&f).unwrap(), b"tampered", "an older write never rewrites a newer file");
        // the file is not read again: the cache answers
        assert_eq!(st.call(&d, "a", "localStorage.getItem", &json!({"key":"k7"})).unwrap().0, json!(7));
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
