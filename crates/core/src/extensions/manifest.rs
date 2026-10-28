//! Reading the `package.json` manifests of extensions Raycast already installed. Only manifests are
//! read (name, title, commands, preferences), never the extension code, and never Raycast's Store API.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    View,
    NoView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefSpec {
    pub name: String,
    pub kind: String,
    pub title: String,
    pub required: bool,
    pub default: Option<Value>,
    /// Help text of the manifest ("" when none).
    pub description: String,
    /// `(title, value)` of a dropdown's `data`.
    pub options: Vec<(String, String)>,
}

/// How a preference is edited in Ferrocast's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefInput {
    Text,
    /// Masked while typing, never logged.
    Password,
    Checkbox,
    Dropdown,
}

impl PrefSpec {
    /// `textfield`, `file`, `directory` and `appPicker` are edited as text (a path or a name).
    pub fn input(&self) -> PrefInput {
        match self.kind.as_str() {
            "password" => PrefInput::Password,
            "checkbox" => PrefInput::Checkbox,
            "dropdown" => PrefInput::Dropdown,
            _ => PrefInput::Text,
        }
    }

    pub fn is_secret(&self) -> bool {
        self.input() == PrefInput::Password
    }

    /// What the form starts with: the saved value, else the manifest default, else empty / off;
    /// always of the type the input needs (a bool for a checkbox, a string otherwise).
    pub fn form_value(&self, saved: &Map<String, Value>) -> Value {
        let v = saved.get(&self.name).or(self.default.as_ref());
        match self.input() {
            PrefInput::Checkbox => Value::Bool(saved.get(&self.name).or(self.default.as_ref()).and_then(Value::as_bool).unwrap_or(false)),
            PrefInput::Dropdown => {
                let s = v.and_then(Value::as_str).map(str::to_string);
                let known = s.filter(|s| self.options.iter().any(|(_, o)| o == s));
                Value::String(known.or_else(|| self.options.first().map(|(_, o)| o.clone())).unwrap_or_default())
            }
            _ => Value::String(match v {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandSpec {
    pub name: String,
    pub title: String,
    pub subtitle: String,
    pub mode: Mode,
    pub preferences: Vec<PrefSpec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// Folder the manifest was read from (empty for `parse_manifest` alone).
    pub dir: PathBuf,
    pub name: String,
    pub title: String,
    pub description: String,
    /// `author` / `owner` of package.json (`launchCommand`'s `ownerOrAuthorName` must be one of them).
    pub owner: String,
    pub author: String,
    pub commands: Vec<CommandSpec>,
    pub preferences: Vec<PrefSpec>,
}

/// A command name is a single plain file-name component (it becomes `<name>.js` in the extension folder).
fn safe_command_name(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(['/', '\\', '\0'])
}

/// Parse a Raycast extension `package.json`. Commands with a mode other than `view` / `no-view`
/// (menu-bar, ...) are left out: they are not supported yet.
pub fn parse_manifest(json: &str) -> Result<Manifest, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let text = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let name = text(&v, "name");
    if name.is_empty() {
        return Err("package.json has no name".into());
    }
    let prefs = |o: &Value| -> Vec<PrefSpec> {
        o.get("preferences")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|p| {
                        let n = text(p, "name");
                        (!n.is_empty()).then(|| PrefSpec {
                            title: Some(text(p, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| n.clone()),
                            name: n,
                            kind: text(p, "type"),
                            required: p.get("required").and_then(Value::as_bool).unwrap_or(false),
                            default: p.get("default").cloned(),
                            description: text(p, "description"),
                            options: p
                                .get("data")
                                .and_then(Value::as_array)
                                .map(|d| d.iter().map(|o| (text(o, "title"), text(o, "value"))).collect())
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let commands = v
        .get("commands")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    let mode = match c.get("mode").and_then(Value::as_str)? {
                        "view" => Mode::View,
                        "no-view" => Mode::NoView,
                        _ => return None,
                    };
                    let n = text(c, "name");
                    safe_command_name(&n).then(|| CommandSpec {
                        title: Some(text(c, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| n.clone()),
                        subtitle: text(c, "subtitle"),
                        preferences: prefs(c),
                        name: n,
                        mode,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Manifest {
        dir: PathBuf::new(),
        title: Some(text(&v, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| name.clone()),
        description: text(&v, "description"),
        owner: text(&v, "owner"),
        author: text(&v, "author"),
        preferences: prefs(&v),
        name,
        commands,
    })
}

/// Manifests of every `<root>/*/package.json` that parses and has at least one supported command,
/// sorted by title. A missing `root` is an empty list.
pub fn discover(root: &Path) -> Vec<Manifest> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut out: Vec<Manifest> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let dir = e.path();
            let mut m = parse_manifest(&std::fs::read_to_string(dir.join("package.json")).ok()?).ok()?;
            m.dir = dir;
            (!m.commands.is_empty()).then_some(m)
        })
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()).then_with(|| a.dir.cmp(&b.dir)));
    out
}

impl Manifest {
    /// The non-empty author and owner names.
    pub fn owner_names(&self) -> Vec<&str> {
        [self.author.as_str(), self.owner.as_str()].into_iter().filter(|s| !s.is_empty()).collect()
    }

    /// Every preference of the extension and of its commands, extension level first, a name once.
    pub fn all_preferences(&self) -> Vec<&PrefSpec> {
        let mut seen = std::collections::HashSet::new();
        self.preferences.iter().chain(self.commands.iter().flat_map(|c| &c.preferences)).filter(|p| seen.insert(p.name.as_str())).collect()
    }

    /// The names of the preferences for a log line. Values are never logged: a token can be a
    /// plain text field as well as a password field.
    pub fn logged_keys(&self, values: &Map<String, Value>) -> String {
        let mut keys: Vec<&str> = values.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys.join(", ")
    }

    /// Values `getPreferenceValues()` returns: the manifest defaults (extension then command level),
    /// overridden by `saved`.
    pub fn preference_values(&self, cmd: &CommandSpec, saved: &Map<String, Value>) -> Map<String, Value> {
        let mut out = Map::new();
        for p in self.preferences.iter().chain(&cmd.preferences) {
            if let Some(d) = &p.default {
                out.insert(p.name.clone(), d.clone());
            }
        }
        out.extend(saved.iter().map(|(k, v)| (k.clone(), v.clone())));
        out
    }

    /// Titles of required preferences that have neither a default nor a saved value.
    pub fn missing_required(&self, cmd: &CommandSpec, saved: &Map<String, Value>) -> Vec<String> {
        let vals = self.preference_values(cmd, saved);
        self.preferences
            .iter()
            .chain(&cmd.preferences)
            .filter(|p| p.required && vals.get(&p.name).is_none_or(|v| v.is_null() || v.as_str() == Some("")))
            .map(|p| p.title.clone())
            .collect()
    }

    /// File the command's compiled bundle is in (`<dir>/<command>.js`).
    pub fn entry(&self, cmd: &CommandSpec) -> Result<PathBuf, String> {
        if !safe_command_name(&cmd.name) {
            return Err(format!("unsafe command name {:?}", cmd.name));
        }
        let p = self.dir.join(format!("{}.js", cmd.name));
        // a symlink must not lead out of the extension folder either
        if let (Ok(real), Ok(base)) = (p.canonicalize(), self.dir.canonicalize()) {
            if !real.starts_with(&base) {
                return Err("the command file is outside the extension folder".into());
            }
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PKG: &str = r#"{
      "name": "json-tools", "title": "JSON Tools", "description": "Format JSON", "icon": "icon.png",
      "preferences": [
        {"name":"indent","type":"textfield","title":"Indent","required":false,"default":"2"},
        {"name":"token","type":"password","title":"API token","required":true}
      ],
      "commands": [
        {"name":"format","title":"Format","subtitle":"JSON","mode":"view"},
        {"name":"quick","title":"Quick","mode":"no-view",
         "preferences":[{"name":"pretty","type":"checkbox","title":"Pretty","required":false,"default":true}]},
        {"name":"bar","title":"Menu","mode":"menu-bar"}
      ],
      "dependencies": {"@raycast/api": "^1.0.0"}
    }"#;

    #[test]
    fn parses_name_title_and_supported_commands_only() {
        let m = parse_manifest(PKG).unwrap();
        assert_eq!((m.name.as_str(), m.title.as_str()), ("json-tools", "JSON Tools"));
        let names: Vec<_> = m.commands.iter().map(|c| (c.name.as_str(), c.mode)).collect();
        assert_eq!(names, vec![("format", Mode::View), ("quick", Mode::NoView)]);
        assert_eq!(m.commands[0].subtitle, "JSON");
        assert_eq!(m.commands[1].preferences.len(), 1);
    }

    #[test]
    fn owner_and_author_are_read_for_launch_command_checks() {
        let m = parse_manifest(r#"{"name":"x","author":"ann","owner":"acme","commands":[{"name":"a","mode":"view"}]}"#).unwrap();
        assert_eq!(m.owner_names(), vec!["ann", "acme"]);
        let m = parse_manifest(r#"{"name":"x","author":"ann","commands":[{"name":"a","mode":"view"}]}"#).unwrap();
        assert_eq!(m.owner_names(), vec!["ann"]);
        assert!(parse_manifest(r#"{"name":"x"}"#).unwrap().owner_names().is_empty());
    }

    #[test]
    fn title_falls_back_to_name_and_bad_json_errors() {
        let m = parse_manifest(r#"{"name":"x","commands":[{"name":"a","title":"A","mode":"view"}]}"#).unwrap();
        assert_eq!(m.title, "x");
        assert!(parse_manifest("not json").is_err());
        assert!(parse_manifest(r#"{"title":"no name"}"#).is_err());
    }

    #[test]
    fn preference_values_merge_defaults_and_saved() {
        let m = parse_manifest(PKG).unwrap();
        let quick = &m.commands[1];
        let mut saved = Map::new();
        saved.insert("token".into(), json!("abc"));
        let v = m.preference_values(quick, &saved);
        assert_eq!(v["indent"], json!("2"));
        assert_eq!(v["pretty"], json!(true));
        assert_eq!(v["token"], json!("abc"));
        saved.insert("indent".into(), json!("4"));
        assert_eq!(m.preference_values(quick, &saved)["indent"], json!("4"));
    }

    #[test]
    fn missing_required_lists_only_unset_ones() {
        let m = parse_manifest(PKG).unwrap();
        assert_eq!(m.missing_required(&m.commands[0], &Map::new()), vec!["API token".to_string()]);
        let mut saved = Map::new();
        saved.insert("token".into(), json!("abc"));
        assert!(m.missing_required(&m.commands[0], &saved).is_empty());
    }

    #[test]
    fn command_names_that_escape_the_extension_folder_are_rejected() {
        for bad in ["../x", "/etc/passwd", "a/b", "..", ".", "a\\b", ""] {
            let json = format!(
                r#"{{"name":"x","commands":[{{"name":{},"title":"T","mode":"view"}},{{"name":"ok","title":"Ok","mode":"view"}}]}}"#,
                serde_json::to_string(bad).unwrap()
            );
            let m = parse_manifest(&json).unwrap();
            assert_eq!(m.commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["ok"], "{bad:?} must be dropped");
        }
        // a hand-built command is still refused by entry()
        let mut m = parse_manifest(PKG).unwrap();
        m.dir = PathBuf::from("/ext/dir");
        let mut c = m.commands[0].clone();
        for bad in ["../x", "/abs", "a/b", ".."] {
            c.name = bad.into();
            assert!(m.entry(&c).is_err(), "{bad}");
        }
        c.name = "format".into();
        assert_eq!(m.entry(&c), Ok(PathBuf::from("/ext/dir/format.js")));
    }

    #[test]
    fn discover_reads_only_manifests_and_skips_broken_or_empty_ones() {
        let root = std::env::temp_dir().join(format!("fc-ext-discover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (dir, body) in [
            ("b-uuid", PKG.replace("JSON Tools", "Zed")),
            ("a-uuid", PKG.replace("JSON Tools", "Alpha")),
            ("broken", "{".to_string()),
            ("menu", r#"{"name":"m","commands":[{"name":"x","title":"X","mode":"menu-bar"}]}"#.to_string()),
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join("package.json"), body).unwrap();
        }
        std::fs::create_dir_all(root.join("nopkg")).unwrap();
        std::fs::write(root.join("stray-file"), "x").unwrap();
        let found = discover(&root);
        assert_eq!(found.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(), vec!["Alpha", "Zed"]);
        assert_eq!(found[0].dir, root.join("a-uuid"));
        assert_eq!(found[0].entry(&found[0].commands[0]), Ok(root.join("a-uuid").join("format.js")));
        assert!(discover(&root.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    const PREFS_PKG: &str = r#"{
      "name": "p", "commands": [{"name":"a","title":"A","mode":"view",
        "preferences":[{"name":"token","type":"password","title":"Token","required":true}]}],
      "preferences": [
        {"name":"host","type":"textfield","title":"Host","description":"Server","default":"localhost"},
        {"name":"safe","type":"checkbox","title":"Safe","label":"Enable"},
        {"name":"mode","type":"dropdown","title":"Mode","default":"b","data":[{"title":"Alpha","value":"a"},{"title":"Beta","value":"b"}]},
        {"name":"app","type":"appPicker","title":"App"},
        {"name":"dir","type":"directory","title":"Folder"},
        {"name":"file","type":"file","title":"File"}
      ]}"#;

    #[test]
    fn every_preference_type_maps_to_a_form_input() {
        let m = parse_manifest(PREFS_PKG).unwrap();
        let kinds: Vec<_> = m.all_preferences().iter().map(|p| (p.name.as_str(), p.input())).collect();
        assert_eq!(
            kinds,
            vec![
                ("host", PrefInput::Text),
                ("safe", PrefInput::Checkbox),
                ("mode", PrefInput::Dropdown),
                ("app", PrefInput::Text),
                ("dir", PrefInput::Text),
                ("file", PrefInput::Text),
                ("token", PrefInput::Password),
            ]
        );
        let mode = m.all_preferences().into_iter().find(|p| p.name == "mode").unwrap();
        assert_eq!(mode.options, vec![("Alpha".to_string(), "a".to_string()), ("Beta".to_string(), "b".to_string())]);
        assert_eq!(m.all_preferences()[0].description, "Server");
    }

    #[test]
    fn the_form_value_is_the_saved_one_then_the_default_then_empty() {
        let m = parse_manifest(PREFS_PKG).unwrap();
        let all = m.all_preferences();
        let mut saved = Map::new();
        saved.insert("host".into(), json!("example.org"));
        assert_eq!(all[0].form_value(&saved), json!("example.org"));
        assert_eq!(all[0].form_value(&Map::new()), json!("localhost"));
        assert_eq!(all[6].form_value(&Map::new()), json!(""));
        assert_eq!(all[1].form_value(&Map::new()), json!(false));
        assert_eq!(all[2].form_value(&Map::new()), json!("b"));
        // a saved value of the wrong type does not break the form
        saved.insert("safe".into(), json!("yes"));
        assert_eq!(all[1].form_value(&saved), json!(false));
    }

    #[test]
    fn only_passwords_are_secret() {
        let m = parse_manifest(PREFS_PKG).unwrap();
        let secrets: Vec<_> = m.all_preferences().iter().filter(|p| p.is_secret()).map(|p| p.name.clone()).collect();
        assert_eq!(secrets, vec!["token"]);
    }

    #[test]
    fn a_log_line_about_preferences_names_keys_and_never_values() {
        let m = parse_manifest(PREFS_PKG).unwrap();
        let mut v = Map::new();
        v.insert("token".into(), json!("s3cr3t-value"));
        v.insert("host".into(), json!("tok-in-a-textfield"));
        let line = m.logged_keys(&v);
        assert!(!line.contains("s3cr3t-value") && !line.contains("tok-in-a-textfield"), "{line}");
        assert_eq!(line, "host, token");
    }

    #[test]
    fn a_non_string_default_is_kept_as_text_in_a_text_field() {
        let m = parse_manifest(
            r#"{"name":"e","title":"E","commands":[{"name":"x","title":"X","mode":"view"}],
            "preferences":[{"name":"n","title":"N","type":"textfield","default":42,"required":false}]}"#,
        )
        .unwrap();
        assert_eq!(m.all_preferences()[0].form_value(&Map::new()), json!("42"));
    }
}
