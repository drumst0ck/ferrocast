//! JSON persistence in `~/Library/Application Support/Ferrocast`.

use crate::models::*;
use crate::ranking::Frecency;
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    /// Default location (overridable with `FERROCAST_DATA_DIR`).
    pub fn default_location() -> Store {
        let root = std::env::var_os("FERROCAST_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Ferrocast"));
        Store { root }
    }

    pub fn new(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn images_dir(&self) -> PathBuf {
        self.root.join("clipboard-images")
    }

    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }

    pub fn load<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        let p = self.path(name);
        match std::fs::read(&p) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|e| {
                eprintln!("ferrocast: failed to parse {}: {e}; backing up", p.display());
                let _ = std::fs::rename(&p, p.with_extension("json.corrupt"));
                T::default()
            }),
            Err(_) => T::default(),
        }
    }

    /// Load a list whose items have an `id`. A file written before ids existed (items without one)
    /// gets its ids assigned and is saved right away, so the ids are the same on every launch:
    /// sync keys items by id and a new id per launch would look like a deletion plus an addition.
    pub fn load_with_ids<T: DeserializeOwned + Serialize>(&self, name: &str) -> Vec<T> {
        let list: Vec<T> = self.load(name);
        let missing = std::fs::read(self.path(name))
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok())
            .is_some_and(|items| items.iter().any(|v| v.get("id").and_then(|i| i.as_str()).is_none_or(str::is_empty)));
        if missing {
            if let Err(e) = self.save(name, &list) {
                eprintln!("ferrocast: could not save ids to {name}: {e}");
            }
        }
        list
    }

    /// Atomic write (temp file + rename).
    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let p = self.path(name);
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        std::fs::rename(&tmp, &p)?;
        Ok(())
    }

    pub fn load_all(&self) -> Data {
        let mut d = Data {
            settings: self.load("settings.json"),
            snippets: self.load_with_ids("snippets.json"),
            quicklinks: self.load_with_ids("quicklinks.json"),
            notes: self.load_with_ids("notes.json"),
            clipboard: self.load_with_ids("clipboard.json"),
            frecency: self.load::<Frecency>("frecency.json"),
        };
        if !self.path("quicklinks.json").exists() {
            d.quicklinks = default_quicklinks();
        }
        if !self.path("snippets.json").exists() {
            d.snippets = default_snippets();
        }
        d
    }
}

/// Built-in items have fixed ids so every Mac (and every launch) means the same item.
fn with_id<T>(mut item: T, set: impl FnOnce(&mut T, String), id: &str) -> T {
    set(&mut item, id.to_string());
    item
}

pub fn default_quicklinks() -> Vec<Quicklink> {
    let q = |id: &str, name: &str, link: &str| with_id(Quicklink::new(name, link), |q, id| q.id = id, id);
    vec![
        q("default-quicklink-google", "Search Google", "https://www.google.com/search?q={argument name=\"query\"}"),
        q("default-quicklink-github", "Search GitHub", "https://github.com/search?q={argument name=\"query\"}"),
        q("default-quicklink-youtube", "Search YouTube", "https://www.youtube.com/results?search_query={argument name=\"query\"}"),
        q("default-quicklink-crates", "Search crates.io", "https://crates.io/search?q={argument name=\"crate\"}"),
        q("default-quicklink-docsrs", "Rust Docs", "https://docs.rs/{argument name=\"crate\"}"),
        q(
            "default-quicklink-translate",
            "Translate",
            "https://translate.google.com/?sl=auto&tl={argument name=\"to\" default=\"en\"}&text={argument name=\"text\"}",
        ),
    ]
}

pub fn default_snippets() -> Vec<Snippet> {
    let s = |id: &str, name: &str, text: &str, kw: &str| with_id(Snippet::new(name, text, kw), |s, id| s.id = id, id);
    vec![
        s("default-snippet-shrug", "Shrug", "¯\\_(ツ)_/¯", "!shrug"),
        s("default-snippet-today", "Today's date", "{date format=\"yyyy-MM-dd\"}", "!today"),
        s("default-snippet-console-log", "Console log", "console.log({cursor})", ";;log"),
    ]
}

/// Delete clipboard image files no longer referenced.
pub fn prune_images(dir: &Path, entries: &[ClipEntry]) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !entries.iter().any(|c| c.kind == ClipKind::Image && c.content == name) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("ferrocast-test-{}", uuid::Uuid::new_v4()));
        let store = Store::new(&dir);
        let d = store.load_all();
        assert!(!d.quicklinks.is_empty());
        let s = vec![Snippet::new("a", "b", "c")];
        store.save("snippets.json", &s).unwrap();
        let back: Vec<Snippet> = store.load("snippets.json");
        assert_eq!(back, s);
        std::fs::write(store.path("notes.json"), "{bad").unwrap();
        let notes: Vec<Note> = store.load("notes.json");
        assert!(notes.is_empty());
        assert!(store.path("notes.json.corrupt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
