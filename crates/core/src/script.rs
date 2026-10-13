//! Raycast-compatible Script Commands: `# @raycast.title …` metadata parsing
//! and ANSI stripping for output display.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    FullOutput,
    Compact,
    Silent,
    Inline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScriptArgument {
    pub kind: String, // text | password | dropdown
    pub placeholder: String,
    pub optional: bool,
    pub percent_encoded: bool,
    pub options: Vec<(String, String)>, // (title, value) for dropdown
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptCommand {
    pub path: PathBuf,
    pub title: String,
    pub mode: Mode,
    pub icon: Option<String>,
    pub package_name: Option<String>,
    pub description: Option<String>,
    pub refresh_time: Option<String>,
    pub current_directory: Option<String>,
    pub needs_confirmation: bool,
    pub arguments: Vec<ScriptArgument>,
    pub author: Option<String>,
}

impl ScriptCommand {
    pub fn id(&self) -> String {
        format!("script:{}", self.path.display())
    }

    /// Working directory where the script should run.
    pub fn working_dir(&self) -> PathBuf {
        let dir = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        match &self.current_directory {
            Some(d) if d.starts_with('~') => dirs::home_dir().unwrap_or_default().join(d.trim_start_matches("~/")),
            Some(d) if Path::new(d).is_absolute() => PathBuf::from(d),
            Some(d) => dir.join(d),
            None => dir,
        }
    }

    /// Refresh interval in seconds for inline scripts (`10s`, `1m`, `1h`, `1d`), min 10s.
    pub fn refresh_secs(&self) -> Option<u64> {
        let r = self.refresh_time.as_deref()?.trim();
        let (n, unit) = r.split_at(r.len().checked_sub(1)?);
        let n: u64 = n.parse().ok()?;
        let s = match unit {
            "s" => n,
            "m" => n * 60,
            "h" => n * 3600,
            "d" => n * 86400,
            _ => return None,
        };
        Some(s.max(10))
    }
}

fn parse_json_arg(v: &str) -> Option<ScriptArgument> {
    let j: serde_json::Value = serde_json::from_str(v).ok()?;
    let options = j
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|o| Some((o.get("title")?.as_str()?.to_string(), o.get("value")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Some(ScriptArgument {
        kind: j.get("type").and_then(|t| t.as_str()).unwrap_or("text").to_string(),
        placeholder: j.get("placeholder").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        optional: j.get("optional").and_then(|t| t.as_bool()).unwrap_or(false),
        percent_encoded: j.get("percentEncoded").and_then(|t| t.as_bool()).unwrap_or(false),
        options,
    })
}

/// Parse metadata from a script's source. Returns None if required fields
/// (`schemaVersion`, `title`, `mode`) are missing.
pub fn parse(path: &Path, source: &str) -> Option<ScriptCommand> {
    let mut title = None;
    let mut mode = None;
    let mut schema = false;
    let mut cmd = ScriptCommand {
        path: path.to_path_buf(),
        title: String::new(),
        mode: Mode::FullOutput,
        icon: None,
        package_name: None,
        description: None,
        refresh_time: None,
        current_directory: None,
        needs_confirmation: false,
        arguments: vec![],
        author: None,
    };
    let mut args: Vec<(u32, ScriptArgument)> = vec![];
    for line in source.lines().take(200) {
        let Some(idx) = line.find("@raycast.") else { continue };
        let rest = &line[idx + "@raycast.".len()..];
        let (key, value) = match rest.find(char::is_whitespace) {
            Some(i) => (&rest[..i], rest[i..].trim()),
            None => (rest, ""),
        };
        let value = value.trim_end_matches("*)").trim_end_matches("-->").trim();
        match key {
            "schemaVersion" => schema = value == "1",
            "title" => title = Some(value.to_string()),
            "mode" => {
                mode = Some(match value {
                    "fullOutput" => Mode::FullOutput,
                    "compact" => Mode::Compact,
                    "silent" => Mode::Silent,
                    "inline" => Mode::Inline,
                    _ => return None,
                })
            }
            "icon" => cmd.icon = Some(value.to_string()),
            "packageName" => cmd.package_name = Some(value.to_string()),
            "description" => cmd.description = Some(value.to_string()),
            "refreshTime" => cmd.refresh_time = Some(value.to_string()),
            "currentDirectoryPath" => cmd.current_directory = Some(value.to_string()),
            "needsConfirmation" => cmd.needs_confirmation = value == "true",
            "author" => cmd.author = Some(value.to_string()),
            k if k.starts_with("argument") => {
                if let (Ok(n), Some(a)) = (k["argument".len()..].parse::<u32>(), parse_json_arg(value)) {
                    args.push((n, a));
                }
            }
            _ => {}
        }
    }
    if !schema {
        return None;
    }
    cmd.title = title.filter(|t| !t.is_empty())?;
    cmd.mode = mode?;
    if cmd.mode == Mode::Inline && cmd.refresh_time.is_none() {
        cmd.mode = Mode::Compact;
    }
    args.sort_by_key(|(n, _)| *n);
    cmd.arguments = args.into_iter().take(3).map(|(_, a)| a).collect();
    Some(cmd)
}

/// Template for "Create Script Command".
pub fn template(title: &str, mode: &str, package: &str) -> String {
    format!(
        "#!/bin/bash\n\n# Required parameters:\n# @raycast.schemaVersion 1\n# @raycast.title {title}\n# @raycast.mode {mode}\n\n# Optional parameters:\n# @raycast.icon 🤖\n# @raycast.packageName {package}\n# @raycast.argument1 {{ \"type\": \"text\", \"placeholder\": \"Value\", \"optional\": true }}\n\n# Documentation:\n# @raycast.description Describe what this does\n\necho \"Hello from {title}! Argument: $1\"\n"
    )
}

/// Strip ANSI escape sequences.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Last non-empty line of output (compact/silent modes).
pub fn last_line(s: &str) -> String {
    strip_ansi(s).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
}

/// First line of output (inline mode).
pub fn first_line(s: &str) -> String {
    strip_ansi(s).lines().next().unwrap_or("").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"#!/bin/bash
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Build XCODE
# @raycast.mode compact
# Optional parameters:
# @raycast.icon 🤖
# @raycast.packageName Dev
# @raycast.argument2 { "type": "dropdown", "placeholder": "Env", "data": [{"title": "Prod", "value": "p"}] }
# @raycast.argument1 { "type": "text", "placeholder": "Branch", "optional": true }
echo hi
"#;

    #[test]
    fn parses_metadata() {
        let c = parse(Path::new("/tmp/x.sh"), SRC).unwrap();
        assert_eq!(c.title, "Build XCODE");
        assert_eq!(c.mode, Mode::Compact);
        assert_eq!(c.icon.as_deref(), Some("🤖"));
        assert_eq!(c.arguments.len(), 2);
        assert_eq!(c.arguments[0].placeholder, "Branch");
        assert!(c.arguments[0].optional);
        assert_eq!(c.arguments[1].options, vec![("Prod".to_string(), "p".to_string())]);
    }

    #[test]
    fn requires_fields() {
        assert!(parse(Path::new("/x"), "# @raycast.title X\n# @raycast.mode silent").is_none());
        assert!(parse(Path::new("/x"), "echo hi").is_none());
    }

    #[test]
    fn inline_without_refresh_falls_back() {
        let c = parse(Path::new("/x"), "# @raycast.schemaVersion 1\n# @raycast.title X\n# @raycast.mode inline").unwrap();
        assert_eq!(c.mode, Mode::Compact);
    }

    #[test]
    fn template_parses() {
        let t = template("Hello", "fullOutput", "Demo");
        let c = parse(Path::new("/x"), &t).unwrap();
        assert_eq!(c.title, "Hello");
        assert_eq!(c.arguments.len(), 1);
    }

    #[test]
    fn ansi() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m ok"), "red ok");
        assert_eq!(last_line("a\nb\n\n"), "b");
    }

    #[test]
    fn refresh() {
        let mut c = parse(Path::new("/x"), SRC).unwrap();
        c.refresh_time = Some("5m".into());
        assert_eq!(c.refresh_secs(), Some(300));
        c.refresh_time = Some("1s".into());
        assert_eq!(c.refresh_secs(), Some(10));
    }
}
