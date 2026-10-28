//! Render tree (`protocol::Node`) -> what the launcher draws.

use super::protocol::Node;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum IconSpec {
    None,
    /// Raycast built-in icon name (`Star`, `Document`, ...).
    Builtin(String),
    Emoji(String),
    /// A file: absolute, or relative to the extension's assets folder.
    Path(String),
    Url(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shortcut {
    /// Lower-case: `cmd`, `shift`, `opt`, `ctrl`.
    pub modifiers: Vec<String>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionModel {
    pub id: String,
    pub title: String,
    /// Component name, e.g. `Action.CopyToClipboard`.
    pub kind: String,
    pub shortcut: Option<Shortcut>,
    pub destructive: bool,
    /// Title of the ActionPanel.Section it sits in.
    pub section: Option<String>,
}

/// The side pane of a list row (`List.Item.Detail`): Markdown and `(label, value)` facts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemDetail {
    pub markdown: String,
    pub meta: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: IconSpec,
    pub accessories: Vec<String>,
    pub keywords: Vec<String>,
    pub actions: Vec<ActionModel>,
    pub detail: Option<ItemDetail>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListSection {
    pub title: String,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListView {
    /// `navigationTitle` (empty: the command's title is shown).
    pub title: String,
    pub is_loading: bool,
    pub placeholder: Option<String>,
    /// The extension handles `onSearchTextChange` (so the host must not filter itself).
    pub handles_search: bool,
    /// The host should filter the items by the search text itself (Raycast's `filtering`: on unless
    /// the extension handles the search, or sets `filtering` to false).
    pub filtering: bool,
    pub sections: Vec<ListSection>,
    pub empty_title: Option<String>,
    pub empty_description: Option<String>,
    /// Names of components inside the list that Ferrocast cannot draw yet.
    pub unsupported: Vec<String>,
    /// The screen was a `Grid`; it is drawn as a list of its items.
    pub is_grid: bool,
    /// `List.Dropdown` / `Grid.Dropdown` of the search bar.
    pub dropdown: Option<DropdownModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropdownOption {
    pub value: String,
    pub title: String,
    pub section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DropdownModel {
    /// The runtime's `_id` of the dropdown (what `HostMsg::Field` names).
    pub id: String,
    pub tooltip: String,
    pub options: Vec<DropdownOption>,
    pub selected: String,
    /// The extension listens to changes.
    pub on_change: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Text,
    TextArea,
    Password,
    Checkbox,
    Dropdown,
    Date,
    Tags,
    Description,
    Separator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    /// The `id` prop (empty for descriptions and separators).
    pub id: String,
    pub kind: FieldKind,
    pub title: String,
    pub placeholder: String,
    pub info: String,
    pub error: String,
    pub options: Vec<DropdownOption>,
    /// `value` or `defaultValue` as JSON: string, bool, or array of strings (tags).
    pub initial: Value,
    /// The extension passes `value` (it owns the state and listens through `onChange`).
    pub controlled: bool,
    /// The extension listens to changes.
    pub on_change: bool,
    /// Descriptions: the text.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormView {
    pub is_loading: bool,
    pub title: String,
    pub fields: Vec<FormField>,
    pub actions: Vec<ActionModel>,
}

impl FormView {
    /// Does the extension need to hear about edits of field `id` (it listens through `onChange` or
    /// owns the value)? Anything else would only be traffic on every keystroke.
    pub fn wants_field(&self, id: &str) -> bool {
        self.fields.iter().any(|f| f.id == id && (f.on_change || f.controlled))
    }
}

impl FormView {
    /// Text of the description field at `i`.
    pub fn description_of(&self, i: usize) -> Option<&str> {
        self.fields.get(i).filter(|f| f.kind == FieldKind::Description).map(|f| f.text.as_str())
    }
}

/// Prefix of the id of the synthetic actions that pick a dropdown option.
pub const DROPDOWN_PREFIX: &str = "dd:";

/// Id of the action that picks `value` in dropdown `dropdown` (the runtime's `_id` of it).
pub fn dropdown_action_id(dropdown: &str, value: &str) -> String {
    format!("{DROPDOWN_PREFIX}{dropdown}:{value}")
}

/// `(dropdown, value)` of an action id made by `dropdown_action_id`; a runtime `_id` has no colon,
/// the value may.
pub fn parse_dropdown_action(id: &str) -> Option<(&str, &str)> {
    id.strip_prefix(DROPDOWN_PREFIX)?.split_once(':')
}

/// What the user typed into a form: one JSON value per field id. Re-synced after each render.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormState {
    values: std::collections::BTreeMap<String, Value>,
    /// Values sent to the extension for a field, with when, oldest first. An extension may echo any
    /// of them back, more than once and out of order (`useLocalStorage` re-reads the stored value
    /// after every `setItem`), so a match is never consumed.
    sent: std::collections::BTreeMap<String, std::collections::VecDeque<(Value, std::time::Instant)>>,
    /// The controlled value of each field in the extension's previous render.
    rendered: std::collections::BTreeMap<String, Value>,
}

/// How long a value we sent can still come back as a (late) echo. Older than this, the same value
/// from the extension is its own change (e.g. `reset()` to "" after the user once cleared the field).
pub const ECHO_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Sent values remembered per field (bounded: an extension that never echoes cannot grow it).
const MAX_UNACKED: usize = 256;

impl FormState {
    /// Adopt a new render: unknown fields start at their initial value, controlled fields follow the
    /// extension, edits of uncontrolled fields stay, fields that are gone are forgotten.
    pub fn sync(&mut self, f: &FormView) {
        self.sync_at(f, std::time::Instant::now());
    }

    pub fn sync_at(&mut self, f: &FormView, now: std::time::Instant) {
        let ids: std::collections::HashSet<&str> = f.fields.iter().filter(|x| has_value(&x.kind)).map(|x| x.id.as_str()).collect();
        self.values.retain(|k, _| ids.contains(k.as_str()));
        self.sent.retain(|k, _| ids.contains(k.as_str()));
        self.rendered.retain(|k, _| ids.contains(k.as_str()));
        for x in f.fields.iter().filter(|x| has_value(&x.kind) && !x.id.is_empty()) {
            // a controlled value only counts as the extension's doing when it differs from the one it
            // rendered last: re-rendering the old value (an optimistic update that returns the current
            // data, `useLocalStorage`) says nothing about what the user is typing now
            let changed = x.controlled && self.rendered.get(&x.id) != Some(&x.initial);
            if x.controlled {
                self.rendered.insert(x.id.clone(), x.initial.clone());
            }
            if !self.values.contains_key(&x.id) {
                self.values.insert(x.id.clone(), x.initial.clone());
            } else if changed && !self.is_echo(&x.id, &x.initial, now) {
                // not an echo of something we sent: the extension changed the value itself
                self.values.insert(x.id.clone(), x.initial.clone());
            }
        }
    }

    /// Is `v`, which the extension rendered for field `id`, something we sent within `ECHO_WINDOW`
    /// (a late echo: keep what the user typed since) rather than the extension's own change?
    fn is_echo(&mut self, id: &str, v: &Value, now: std::time::Instant) -> bool {
        let Some(q) = self.sent.get_mut(id) else { return false };
        while q.front().is_some_and(|(_, t)| now.saturating_duration_since(*t) > ECHO_WINDOW) {
            q.pop_front();
        }
        q.iter().any(|(x, _)| x == v)
    }

    /// The user edited field `id` here. `sent`: the extension is told (it listens or controls the
    /// value), so its echoes of this value must not overwrite newer typing.
    pub fn edit(&mut self, id: &str, v: Value, sent: bool) {
        self.edit_at(id, v, sent, std::time::Instant::now());
    }

    pub fn edit_at(&mut self, id: &str, v: Value, sent: bool, now: std::time::Instant) {
        if sent {
            let q = self.sent.entry(id.to_string()).or_default();
            q.push_back((v.clone(), now));
            while q.len() > MAX_UNACKED {
                q.pop_front();
            }
        }
        self.values.insert(id.to_string(), v);
    }

    pub fn get(&self, id: &str) -> Value {
        self.values.get(id).cloned().unwrap_or(Value::Null)
    }

    pub fn set(&mut self, id: &str, v: Value) {
        self.values.insert(id.to_string(), v);
    }

    /// The values to submit, by field id.
    pub fn values(&self, f: &FormView) -> serde_json::Map<String, Value> {
        f.fields
            .iter()
            .filter(|x| has_value(&x.kind) && !x.id.is_empty())
            .map(|x| (x.id.clone(), self.values.get(&x.id).cloned().unwrap_or_else(|| x.initial.clone())))
            .collect()
    }
}

fn has_value(k: &FieldKind) -> bool {
    !matches!(k, FieldKind::Description | FieldKind::Separator)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetailView {
    /// `navigationTitle` (empty: the command's title is shown).
    pub title: String,
    pub markdown: String,
    pub is_loading: bool,
    pub actions: Vec<ActionModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewModel {
    Empty,
    List(ListView),
    Detail(DetailView),
    Form(FormView),
    /// The screen's root component is not supported yet.
    Unsupported(String),
}

impl ViewModel {
    /// The `navigationTitle` the extension set on this screen ("" when none).
    pub fn nav_title(&self) -> &str {
        match self {
            ViewModel::List(l) => &l.title,
            ViewModel::Detail(d) => &d.title,
            ViewModel::Form(f) => &f.title,
            _ => "",
        }
    }
}

impl ListView {
    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.items.is_empty())
    }
}

/// Where the page about running Raycast extensions lives.
pub const DOCS_URL: &str = "https://ferrocast.app/docs/launcher/raycast-extensions/";

/// The message of a screen Ferrocast cannot draw: the component, the extension and the docs page.
pub fn unsupported_message(component: &str, extension: &str) -> String {
    format!("Not supported in Ferrocast yet: {component}\n\nThe extension \"{extension}\" uses a part of the Raycast API that Ferrocast does not draw yet. What works and what does not: {DOCS_URL}")
}

/// The line shown in place of a component Ferrocast cannot draw.
pub fn unsupported_line(component: &str) -> String {
    format!("not supported in Ferrocast yet: {component}")
}

/// Build the view model of a `#root` node (or of a bare element).
pub fn build(root: &Node) -> ViewModel {
    let el = if root.t == "#root" { root.c.iter().find(|n| n.t != "#text") } else { Some(root) };
    let Some(el) = el else { return ViewModel::Empty };
    match el.t.as_str() {
        "List" | "Grid" => ViewModel::List(build_list(el)),
        "Form" => ViewModel::Form(build_form(el)),
        "Detail" => {
            let mut actions = vec![];
            let mut markdown = text_of(el.p.get("markdown"));
            for c in &el.c {
                collect_actions(c, None, &mut actions);
                if c.t == "Detail.Metadata" {
                    let md = metadata_markdown(c);
                    if !md.is_empty() {
                        markdown = format!("{markdown}\n\n---\n\n{md}");
                    }
                }
            }
            ViewModel::Detail(DetailView {
                title: text_of(el.p.get("navigationTitle")),
                markdown,
                is_loading: flag(el, "isLoading"),
                actions,
            })
        }
        other => ViewModel::Unsupported(other.to_string()),
    }
}

fn flag(n: &Node, key: &str) -> bool {
    n.p.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// A string prop that may also be written `{ value: "..." }` (or a number).
fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Object(o)) => text_of(o.get("value")),
        _ => String::new(),
    }
}

fn default_action_title(kind: &str) -> &'static str {
    match kind {
        "Action.SubmitForm" => "Submit Form",
        "Action.CopyToClipboard" => "Copy to Clipboard",
        "Action.OpenInBrowser" => "Open in Browser",
        "Action.Paste" => "Paste in Frontmost App",
        "Action.Push" => "Open",
        "Action.Open" => "Open",
        "Action.ShowInFinder" => "Show in Finder",
        "Action.CopyToClipboard.Selection" => "Copy",
        _ => "Action",
    }
}

fn collect_actions(n: &Node, section: Option<&str>, out: &mut Vec<ActionModel>) {
    match n.t.as_str() {
        "ActionPanel" => n.c.iter().for_each(|c| collect_actions(c, section, out)),
        "ActionPanel.Section" | "ActionPanel.Submenu" => {
            let title = text_of(n.p.get("title"));
            let title = (!title.is_empty()).then_some(title);
            n.c.iter().for_each(|c| collect_actions(c, title.as_deref().or(section), out));
        }
        t if t == "Action" || t.starts_with("Action.") => {
            let title = text_of(n.p.get("title"));
            let shortcut = n.p.get("shortcut").and_then(|s| {
                let key = s.get("key")?.as_str()?.to_lowercase();
                let modifiers = s
                    .get("modifiers")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_lowercase)).collect())
                    .unwrap_or_default();
                Some(Shortcut { modifiers, key })
            });
            out.push(ActionModel {
                id: text_of(n.p.get("_id")),
                title: if title.is_empty() { default_action_title(t).to_string() } else { title },
                kind: t.to_string(),
                shortcut,
                destructive: n.p.get("style").and_then(Value::as_str) == Some("destructive"),
                section: section.map(str::to_string),
            });
        }
        _ => {}
    }
}

/// `(label, value)` facts of a `*.Metadata` node (labels, links, tag lists); separators are skipped.
fn metadata_facts(m: &Node) -> Vec<(String, String)> {
    let mut out = vec![];
    for c in &m.c {
        let title = text_of(c.p.get("title"));
        let kind = c.t.rsplit('.').next().unwrap_or("");
        let value = match kind {
            "Label" => text_of(c.p.get("text")),
            "Link" => {
                let (text, target) = (text_of(c.p.get("text")), text_of(c.p.get("target")));
                if target.is_empty() {
                    text
                } else {
                    format!("[{text}]({target})")
                }
            }
            "TagList" => c.c.iter().map(|i| text_of(i.p.get("text"))).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "),
            _ => continue,
        };
        if !(title.is_empty() && value.is_empty()) {
            out.push((title, value));
        }
    }
    out
}

fn metadata_markdown(m: &Node) -> String {
    let mut lines: Vec<String> = vec![];
    for c in &m.c {
        if c.t.ends_with(".Separator") {
            lines.push("---".into());
            continue;
        }
        let facts = metadata_facts(&Node { t: m.t.clone(), p: Default::default(), c: vec![c.clone()] });
        for (k, v) in facts {
            lines.push(if k.is_empty() { v } else { format!("**{k}**: {v}") });
        }
    }
    lines.join("\n\n")
}

fn parse_options(parent: &Node, section: Option<&str>, out: &mut Vec<DropdownOption>) {
    for c in &parent.c {
        if c.t.ends_with(".Item") {
            out.push(DropdownOption {
                value: text_of(c.p.get("value")),
                title: text_of(c.p.get("title")),
                section: section.map(str::to_string),
            });
        } else if c.t.ends_with(".Section") {
            let title = text_of(c.p.get("title"));
            parse_options(c, Some(&title).filter(|t| !t.is_empty()).map(String::as_str).or(section), out);
        }
    }
}

fn build_dropdown(n: &Node) -> DropdownModel {
    let mut options = vec![];
    parse_options(n, None, &mut options);
    let selected = ["value", "defaultValue"]
        .iter()
        .find_map(|k| n.p.get(*k).map(|v| text_of(Some(v))))
        .or_else(|| options.first().map(|o| o.value.clone()))
        .unwrap_or_default();
    DropdownModel { id: text_of(n.p.get("_id")), tooltip: text_of(n.p.get("tooltip")), options, selected, on_change: flag(n, "onChange") }
}

fn build_form(el: &Node) -> FormView {
    let mut f = FormView { is_loading: flag(el, "isLoading"), title: text_of(el.p.get("navigationTitle")), ..Default::default() };
    for c in &el.c {
        collect_actions(c, None, &mut f.actions);
        let kind = match c.t.as_str() {
            "Form.TextField" => FieldKind::Text,
            "Form.TextArea" => FieldKind::TextArea,
            "Form.PasswordField" => FieldKind::Password,
            "Form.Checkbox" => FieldKind::Checkbox,
            "Form.Dropdown" => FieldKind::Dropdown,
            "Form.DatePicker" => FieldKind::Date,
            "Form.TagPicker" => FieldKind::Tags,
            "Form.Description" => FieldKind::Description,
            "Form.Separator" => FieldKind::Separator,
            _ => continue,
        };
        let mut options = vec![];
        parse_options(c, None, &mut options);
        let given = c.p.get("value").or_else(|| c.p.get("defaultValue")).filter(|v| !v.is_null());
        let initial = match (&kind, given) {
            (FieldKind::Checkbox, Some(v)) => Value::Bool(v.as_bool().unwrap_or(false)),
            (FieldKind::Checkbox, None) => Value::Bool(false),
            (FieldKind::Tags, Some(Value::Array(a))) => {
                Value::Array(a.iter().filter_map(|x| x.as_str().map(|s| Value::String(s.into()))).collect())
            }
            (FieldKind::Tags, _) => Value::Array(vec![]),
            (FieldKind::Dropdown, Some(v)) => Value::String(text_of(Some(v))),
            (FieldKind::Dropdown, None) => Value::String(options.first().map(|o| o.value.clone()).unwrap_or_default()),
            (_, Some(v)) => Value::String(text_of(Some(v))),
            (_, None) => Value::String(String::new()),
        };
        let checkbox_label = text_of(c.p.get("label"));
        let title = text_of(c.p.get("title"));
        f.fields.push(FormField {
            id: text_of(c.p.get("id")),
            title: if kind == FieldKind::Checkbox && !checkbox_label.is_empty() { checkbox_label } else { title },
            placeholder: text_of(c.p.get("placeholder")),
            info: text_of(c.p.get("info")),
            error: text_of(c.p.get("error")),
            controlled: c.p.get("value").is_some_and(|v| !v.is_null()),
            on_change: flag(c, "onChange"),
            text: text_of(c.p.get("text")),
            kind,
            options,
            initial,
        });
    }
    f
}

/// Icon of a grid cell: `content` is an image source, `{ source }`, `{ value: <source>, tooltip }` or `{ color }`.
fn grid_icon(v: Option<&Value>) -> IconSpec {
    match v {
        Some(Value::Object(o)) if o.contains_key("value") => o.get("value").map(grid_icon_inner).unwrap_or(IconSpec::None),
        Some(v) => grid_icon_inner(v),
        None => IconSpec::None,
    }
}
fn grid_icon_inner(v: &Value) -> IconSpec {
    parse_icon(v)
}

fn build_item(n: &Node) -> ListItem {
    let explicit = text_of(n.p.get("id"));
    let title = text_of(n.p.get("title"));
    let mut actions = vec![];
    let mut detail = None;
    for c in &n.c {
        collect_actions(c, None, &mut actions);
        if c.t == "List.Item.Detail" {
            let meta = c.c.iter().find(|m| m.t == "List.Item.Detail.Metadata").map(metadata_facts).unwrap_or_default();
            detail = Some(ItemDetail { markdown: text_of(c.p.get("markdown")), meta });
        }
    }
    let accessories =
        n.p.get("accessories")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let t = text_of(x.get("text"));
                        let tag = text_of(x.get("tag"));
                        let date = text_of(x.get("date"));
                        let s = if !t.is_empty() {
                            t
                        } else if !tag.is_empty() {
                            tag
                        } else {
                            date.get(..10).filter(|_| date.len() >= 10).unwrap_or(&date).to_string()
                        };
                        (!s.is_empty()).then_some(s)
                    })
                    .collect()
            })
            .unwrap_or_default();
    ListItem {
        id: if explicit.is_empty() {
            let g = text_of(n.p.get("_id"));
            if g.is_empty() {
                title.clone()
            } else {
                g
            }
        } else {
            explicit
        },
        subtitle: text_of(n.p.get("subtitle")),
        icon: if n.t == "Grid.Item" { grid_icon(n.p.get("content")) } else { n.p.get("icon").map(parse_icon).unwrap_or(IconSpec::None) },
        keywords: n
            .p
            .get("keywords")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        title,
        accessories,
        actions,
        detail,
    }
}

fn build_list(el: &Node) -> ListView {
    let mut l = ListView {
        title: text_of(el.p.get("navigationTitle")),
        is_loading: flag(el, "isLoading"),
        placeholder: el.p.get("searchBarPlaceholder").and_then(Value::as_str).map(str::to_string),
        handles_search: flag(el, "onSearchTextChange"),
        is_grid: el.t == "Grid",
        filtering: match el.p.get("filtering") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Object(_)) => true,
            _ => !flag(el, "onSearchTextChange"),
        },
        ..Default::default()
    };
    let mut loose: Option<ListSection> = None;
    let flush = |loose: &mut Option<ListSection>, sections: &mut Vec<ListSection>| {
        if let Some(s) = loose.take() {
            sections.push(s);
        }
    };
    for c in &el.c {
        match c.t.as_str() {
            "List.Section" | "Grid.Section" => {
                flush(&mut loose, &mut l.sections);
                let is_item = |t: &str| t == "List.Item" || t == "Grid.Item";
                let items = c.c.iter().filter(|i| is_item(&i.t)).map(build_item).collect();
                for bad in c.c.iter().filter(|i| !is_item(&i.t) && i.t != "#text") {
                    if !l.unsupported.contains(&bad.t) {
                        l.unsupported.push(bad.t.clone());
                    }
                }
                l.sections.push(ListSection { title: text_of(c.p.get("title")), items });
            }
            "List.Item" | "Grid.Item" => loose.get_or_insert_with(ListSection::default).items.push(build_item(c)),
            "List.Dropdown" | "Grid.Dropdown" => l.dropdown = Some(build_dropdown(c)),
            "List.EmptyView" | "Grid.EmptyView" => {
                l.empty_title = Some(text_of(c.p.get("title"))).filter(|t| !t.is_empty());
                l.empty_description = Some(text_of(c.p.get("description"))).filter(|t| !t.is_empty());
            }
            // the list's own actions (they apply when it is empty): not drawn, nothing is lost
            "#text" | "ActionPanel" => {}
            other => {
                if !l.unsupported.contains(&other.to_string()) {
                    l.unsupported.push(other.to_string());
                }
            }
        }
    }
    flush(&mut loose, &mut l.sections);
    l
}

pub fn parse_icon(v: &Value) -> IconSpec {
    match v {
        Value::String(s) => {
            if let Some(name) = s.strip_prefix("icon:") {
                IconSpec::Builtin(name.to_string())
            } else if s.starts_with("http://") || s.starts_with("https://") {
                IconSpec::Url(s.clone())
            } else if s.is_empty() {
                IconSpec::None
            } else if !s.is_ascii() && s.chars().count() <= 8 && !s.contains('/') {
                IconSpec::Emoji(s.clone())
            } else {
                IconSpec::Path(s.clone())
            }
        }
        Value::Object(o) => match (o.get("source"), o.get("fileIcon")) {
            (Some(src), _) => match src {
                // {source: {light, dark}} -> light
                Value::Object(m) => m.get("light").or(m.get("dark")).map(parse_icon).unwrap_or(IconSpec::None),
                other => parse_icon(other),
            },
            (None, Some(Value::String(p))) => IconSpec::Path(p.clone()),
            _ => IconSpec::None,
        },
        _ => IconSpec::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tree(v: Value) -> Node {
        serde_json::from_value(v).unwrap()
    }

    fn list_tree() -> Node {
        tree(json!({"t":"#root","c":[{"t":"List","p":{"isLoading":true,"searchBarPlaceholder":"Find…","onSearchTextChange":true},"c":[
          {"t":"List.Section","p":{"title":"Fruit"},"c":[
            {"t":"List.Item","p":{"_id":"0.0.0","title":"Apple","subtitle":"red","icon":"icon:Star",
                "accessories":[{"text":"3 left"},{"tag":{"value":"new"}},{"date":"2024-05-06T10:00:00.000Z"}],"keywords":["pome"]},"c":[
              {"t":"ActionPanel","c":[
                {"t":"Action.CopyToClipboard","p":{"_id":"0.0.0.0.0","title":"Copy Name","content":"Apple",
                    "shortcut":{"modifiers":["cmd","shift"],"key":"c"}}},
                {"t":"ActionPanel.Section","p":{"title":"Danger"},"c":[
                  {"t":"Action","p":{"_id":"0.0.0.0.1","title":"Delete","style":"destructive"}}]}]}]},
            {"t":"List.Item","p":{"id":"pear","title":{"value":"Pear"}}}]},
          {"t":"List.Item","p":{"_id":"0.1","title":"Loose"}},
          {"t":"List.EmptyView","p":{"title":"Nothing","description":"Try again"}}
        ]}]}))
    }

    #[test]
    fn list_tree_becomes_sections_items_and_actions() {
        let ViewModel::List(l) = build(&list_tree()) else { panic!("not a list") };
        assert!(l.is_loading && l.handles_search && !l.filtering);
        assert_eq!(l.placeholder.as_deref(), Some("Find…"));
        assert_eq!(l.sections.len(), 2);
        assert_eq!(l.sections[0].title, "Fruit");
        let apple = &l.sections[0].items[0];
        assert_eq!((apple.id.as_str(), apple.title.as_str(), apple.subtitle.as_str()), ("0.0.0", "Apple", "red"));
        assert_eq!(apple.icon, IconSpec::Builtin("Star".into()));
        assert_eq!(apple.accessories, vec!["3 left", "new", "2024-05-06"]);
        assert_eq!(apple.keywords, vec!["pome"]);
        assert_eq!(apple.actions.len(), 2);
        assert_eq!(apple.actions[0].kind, "Action.CopyToClipboard");
        assert_eq!(apple.actions[0].shortcut, Some(Shortcut { modifiers: vec!["cmd".into(), "shift".into()], key: "c".into() }));
        assert_eq!(apple.actions[1].section.as_deref(), Some("Danger"));
        assert!(apple.actions[1].destructive);
        // explicit id wins, title object accepted
        let pear = &l.sections[0].items[1];
        assert_eq!((pear.id.as_str(), pear.title.as_str()), ("pear", "Pear"));
        // items outside a section go to an untitled one
        assert_eq!(l.sections[1].title, "");
        assert_eq!(l.sections[1].items[0].title, "Loose");
        assert_eq!(l.empty_title.as_deref(), Some("Nothing"));
        assert!(!l.is_empty());
    }

    #[test]
    fn detail_tree_gives_markdown_and_actions() {
        let t = tree(json!({"t":"#root","c":[{"t":"Detail","p":{"markdown":"# Hi","isLoading":false},"c":[
            {"t":"ActionPanel","c":[{"t":"Action.OpenInBrowser","p":{"_id":"0.0","title":"Open","url":"https://x.test"}}]}]}]}));
        let ViewModel::Detail(d) = build(&t) else { panic!("not a detail") };
        assert_eq!(d.markdown, "# Hi");
        assert_eq!(d.actions[0].id, "0.0");
        assert_eq!(d.actions[0].kind, "Action.OpenInBrowser");
    }

    #[test]
    fn unsupported_roots_and_children_are_reported_not_dropped() {
        let t = tree(json!({"t":"#root","c":[{"t":"MenuBarExtra","c":[]}]}));
        assert_eq!(build(&t), ViewModel::Unsupported("MenuBarExtra".into()));
        let t = tree(json!({"t":"#root","c":[{"t":"List","c":[{"t":"List.Item","p":{"_id":"1","title":"a"}},{"t":"MenuBarExtra.Item"}]}]}));
        let ViewModel::List(l) = build(&t) else { panic!() };
        assert_eq!(l.unsupported, vec!["MenuBarExtra.Item"]);
        assert_eq!(l.sections[0].items.len(), 1);
        assert_eq!(unsupported_line("MenuBarExtra"), "not supported in Ferrocast yet: MenuBarExtra");
        assert_eq!(build(&tree(json!({"t":"#root"}))), ViewModel::Empty);
    }

    #[test]
    fn the_unsupported_message_names_the_component_the_extension_and_the_docs() {
        let m = unsupported_message("MenuBarExtra", "Timers");
        assert!(m.starts_with("Not supported in Ferrocast yet: MenuBarExtra"), "{m}");
        assert!(m.contains("Timers"), "{m}");
        assert!(m.contains(DOCS_URL), "{m}");
        assert_eq!(DOCS_URL, "https://ferrocast.app/docs/launcher/raycast-extensions/");
        assert!(!m.to_lowercase().contains("prototype"));
    }

    #[test]
    fn filtering_defaults_follow_the_search_handler() {
        let f = |props: Value| {
            let ViewModel::List(l) = build(&tree(json!({"t":"#root","c":[{"t":"List","p":props}]}))) else { panic!() };
            l.filtering
        };
        assert!(f(json!({})));
        assert!(!f(json!({"onSearchTextChange":true})));
        assert!(f(json!({"onSearchTextChange":true,"filtering":true})));
        assert!(!f(json!({"filtering":false})));
    }

    #[test]
    fn icon_forms() {
        assert_eq!(parse_icon(&json!("icon:Clipboard")), IconSpec::Builtin("Clipboard".into()));
        assert_eq!(parse_icon(&json!("🍎")), IconSpec::Emoji("🍎".into()));
        assert_eq!(parse_icon(&json!("https://x.test/a.png")), IconSpec::Url("https://x.test/a.png".into()));
        assert_eq!(parse_icon(&json!("a.png")), IconSpec::Path("a.png".into()));
        assert_eq!(parse_icon(&json!({"source":"icon:Star","tintColor":"#f00"})), IconSpec::Builtin("Star".into()));
        assert_eq!(parse_icon(&json!({"fileIcon":"/Applications/Safari.app"})), IconSpec::Path("/Applications/Safari.app".into()));
        assert_eq!(parse_icon(&json!(null)), IconSpec::None);
    }

    fn form_tree() -> Node {
        tree(json!({"t":"#root","c":[{"t":"Form","p":{"isLoading":false,"navigationTitle":"New"},"c":[
          {"t":"Form.Description","p":{"title":"Hi","text":"Fill this in"}},
          {"t":"Form.TextField","p":{"_id":"0.1","id":"name","title":"Name","placeholder":"Ann","info":"Your name","error":"Required","defaultValue":"Ann"}},
          {"t":"Form.TextArea","p":{"id":"bio","title":"Bio","value":"x","onChange":true}},
          {"t":"Form.PasswordField","p":{"id":"pw","title":"Password"}},
          {"t":"Form.Checkbox","p":{"id":"ok","title":"Opt","label":"Enable","defaultValue":true}},
          {"t":"Form.Separator"},
          {"t":"Form.Dropdown","p":{"id":"color","title":"Color"},"c":[
            {"t":"Form.Dropdown.Item","p":{"value":"r","title":"Red"}},
            {"t":"Form.Dropdown.Section","p":{"title":"More"},"c":[{"t":"Form.Dropdown.Item","p":{"value":"g","title":"Green"}}]}]},
          {"t":"Form.DatePicker","p":{"id":"when","title":"When","type":"date","defaultValue":"2024-05-06T10:00:00.000Z"}},
          {"t":"Form.TagPicker","p":{"id":"tags","title":"Tags","defaultValue":["a"]},"c":[
            {"t":"Form.TagPicker.Item","p":{"value":"a","title":"A"}},{"t":"Form.TagPicker.Item","p":{"value":"b","title":"B"}}]},
          {"t":"ActionPanel","c":[{"t":"Action.SubmitForm","p":{"_id":"0.9.0","title":"Save"}}]}
        ]}]}))
    }

    #[test]
    fn form_tree_becomes_fields_with_initial_values_and_actions() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!("not a form") };
        let kinds: Vec<_> = f.fields.iter().map(|x| (x.id.as_str(), x.kind.clone())).collect();
        assert_eq!(
            kinds,
            vec![
                ("", FieldKind::Description),
                ("name", FieldKind::Text),
                ("bio", FieldKind::TextArea),
                ("pw", FieldKind::Password),
                ("ok", FieldKind::Checkbox),
                ("", FieldKind::Separator),
                ("color", FieldKind::Dropdown),
                ("when", FieldKind::Date),
                ("tags", FieldKind::Tags),
            ]
        );
        let name = &f.fields[1];
        assert_eq!(
            (name.title.as_str(), name.placeholder.as_str(), name.info.as_str(), name.error.as_str()),
            ("Name", "Ann", "Your name", "Required")
        );
        assert_eq!(name.initial, json!("Ann"));
        assert!(!name.controlled);
        assert!(f.fields[2].controlled && f.fields[2].initial == json!("x"));
        assert_eq!(f.fields[3].initial, json!(""));
        assert_eq!(f.fields[4].initial, json!(true));
        assert_eq!(f.fields[4].title, "Enable", "a checkbox shows its label");
        // a dropdown without a default starts on its first option; sections flatten with their title
        let dd = &f.fields[6];
        assert_eq!(dd.initial, json!("r"));
        assert_eq!(
            dd.options.iter().map(|o| (o.value.as_str(), o.title.as_str(), o.section.as_deref())).collect::<Vec<_>>(),
            vec![("r", "Red", None), ("g", "Green", Some("More"))]
        );
        assert_eq!(f.fields[7].initial, json!("2024-05-06T10:00:00.000Z"));
        assert_eq!(f.fields[8].initial, json!(["a"]));
        assert_eq!(f.fields[8].options.len(), 2);
        assert_eq!(f.actions[0].kind, "Action.SubmitForm");
        assert_eq!(f.actions[0].title, "Save");
        assert_eq!(f.description_of(0), Some("Fill this in"));
    }

    #[test]
    fn the_host_only_reports_edits_of_fields_the_extension_listens_to_or_controls() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        assert!(!f.wants_field("name"), "no onChange, not controlled: nobody is listening");
        assert!(f.wants_field("bio"), "controlled (and onChange)");
        assert!(!f.wants_field("nope"));
        let mut g = f.clone();
        g.fields[1].on_change = true;
        assert!(g.wants_field("name"));
        let mut h = f.clone();
        h.fields[1].controlled = true;
        assert!(h.wants_field("name"), "a controlled value must follow even without onChange");
    }

    fn echo(f: &FormView, id: &str, v: Value) -> FormView {
        let mut g = f.clone();
        g.fields.iter_mut().filter(|x| x.id == id).for_each(|x| x.initial = v.clone());
        g
    }

    #[test]
    fn late_echoes_of_what_we_sent_never_overwrite_newer_typing() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f);
        for t in ["h", "ho", "hol"] {
            st.edit("bio", json!(t), true);
        }
        st.sync(&echo(&f, "bio", json!("h")));
        assert_eq!(st.get("bio"), json!("hol"), "echo of the first keystroke arrives late");
        st.sync(&echo(&f, "bio", json!("ho")));
        assert_eq!(st.get("bio"), json!("hol"));
        st.sync(&echo(&f, "bio", json!("hol")));
        assert_eq!(st.get("bio"), json!("hol"));
        // everything is acknowledged now: a value that differs is the extension's own change
        st.sync(&echo(&f, "bio", json!("HOL!")));
        assert_eq!(st.get("bio"), json!("HOL!"));
    }

    /// Live finding (L15): `useLocalStorage` revalidates after every `setItem`, so the same stored
    /// value comes back several times and out of step with the typing.
    #[test]
    fn repeated_and_out_of_order_echoes_of_sent_values_never_revert_the_text() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f);
        let typed = ["a", "ab", "abc", "abcd", "abcde"];
        for t in typed {
            st.edit("bio", json!(t), true);
        }
        // the echoes: ab, abc, abc again (a second revalidation), then ab (late), then a
        for e in ["ab", "abc", "abc", "ab", "a"] {
            st.sync(&echo(&f, "bio", json!(e)));
            assert_eq!(st.get("bio"), json!("abcde"), "echo {e:?} must not revert the text");
        }
        // the user keeps typing on the real text, not on a reverted base
        st.edit("bio", json!("abcdef"), true);
        st.sync(&echo(&f, "bio", json!("abcde")));
        assert_eq!(st.get("bio"), json!("abcdef"));
    }

    /// Live finding (L15, root cause): `useLocalStorage.setValue` calls `mutate(..., {optimisticUpdate(c){return c}})`,
    /// which re-renders the field with the extension's CURRENT value (the one before this keystroke)
    /// as soon as `onChange` runs, long before the value read back from storage arrives. A re-render
    /// of a value that did not change is not an extension-side change.
    #[test]
    fn a_re_render_with_the_unchanged_old_value_does_not_revert_typing() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f); // bio = "x"
        st.edit("bio", json!("xa"), true);
        st.sync(&echo(&f, "bio", json!("x"))); // optimistic render: the old value again
        assert_eq!(st.get("bio"), json!("xa"));
        st.sync(&echo(&f, "bio", json!("xa"))); // the value read back from storage
        st.edit("bio", json!("xab"), true);
        st.sync(&echo(&f, "bio", json!("xa"))); // optimistic again (current data is "xa")
        assert_eq!(st.get("bio"), json!("xab"));
        st.sync(&echo(&f, "bio", json!("xab")));
        assert_eq!(st.get("bio"), json!("xab"));
    }

    /// The extension of the live test, as a model: every keystroke re-renders the old value at once
    /// and the stored value only some keystrokes later. The host text must end as typed, at any speed.
    #[test]
    fn fast_typing_against_an_optimistic_extension_ends_with_host_equal_to_extension() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        for lag in 0..4 {
            let mut st = FormState::default();
            st.sync(&echo(&f, "bio", json!("draft")));
            let mut ext = "draft".to_string(); // the extension's current value
            let mut pending: std::collections::VecDeque<String> = Default::default(); // stored values to read back
            let mut host = "draft".to_string();
            for (i, ch) in "hola \"mundo\" con prisa 123".chars().enumerate() {
                host.insert(1, ch); // cursor stays after the first character
                st.edit("bio", json!(host.clone()), true);
                pending.push_back(host.clone());
                st.sync(&echo(&f, "bio", json!(ext.clone()))); // optimistic update: the current value
                if i >= lag {
                    ext = pending.pop_front().unwrap(); // a read-back arrives
                    st.sync(&echo(&f, "bio", json!(ext.clone())));
                }
                assert_eq!(st.get("bio"), json!(host.clone()), "lag {lag}, key {i}: the host text is what was typed");
                host = st.get("bio").as_str().unwrap().to_string();
            }
            while let Some(v) = pending.pop_front() {
                ext = v;
                st.sync(&echo(&f, "bio", json!(ext.clone())));
            }
            assert_eq!(ext, host, "lag {lag}: extension == host");
            assert_eq!(st.get("bio"), json!(ext));
        }
    }

    #[test]
    fn an_old_sent_value_is_an_extension_change_once_it_cannot_be_a_late_echo_any_more() {
        use std::time::{Duration, Instant};
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        let t0 = Instant::now();
        st.sync_at(&f, t0);
        st.edit_at("bio", json!(""), true, t0);
        st.edit_at("bio", json!("typed"), true, t0 + Duration::from_secs(1));
        st.sync_at(&echo(&f, "bio", json!("")), t0 + Duration::from_secs(2));
        assert_eq!(st.get("bio"), json!("typed"), "2 s later it can still be a late echo");
        st.sync_at(&echo(&f, "bio", json!("typed")), t0 + Duration::from_secs(3));
        // the extension resets the field to "" long after: that is its own change
        st.sync_at(&echo(&f, "bio", json!("")), t0 + ECHO_WINDOW + Duration::from_secs(2));
        assert_eq!(st.get("bio"), json!(""));
    }

    #[test]
    fn an_extension_side_change_is_applied_even_with_edits_in_flight() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f);
        st.edit("bio", json!("a"), true);
        st.edit("bio", json!("ab"), true);
        st.sync(&echo(&f, "bio", json!("")));
        assert_eq!(st.get("bio"), json!(""), "reset() from the extension wins");
        st.sync(&echo(&f, "bio", json!("a")));
        assert_eq!(st.get("bio"), json!(""), "...and the late echo of an older keystroke does not undo it");
    }

    #[test]
    fn edits_that_were_not_sent_are_not_tracked() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f);
        st.edit("bio", json!("typed"), false);
        st.sync(&echo(&f, "bio", json!("typed")));
        st.sync(&echo(&f, "bio", json!("other")));
        assert_eq!(st.get("bio"), json!("other"));
    }

    #[test]
    fn views_carry_the_navigation_title() {
        let l = build(&tree(json!({"t":"#root","c":[{"t":"List","p":{"navigationTitle":"Pick one"}}]})));
        let d = build(&tree(json!({"t":"#root","c":[{"t":"Detail","p":{"navigationTitle":"Read","markdown":"x"}}]})));
        let f = build(&form_tree());
        assert_eq!((l.nav_title(), d.nav_title(), f.nav_title()), ("Pick one", "Read", "New"));
        assert_eq!(build(&tree(json!({"t":"#root","c":[{"t":"List"}]}))).nav_title(), "");
        assert_eq!(ViewModel::Empty.nav_title(), "");
    }

    #[test]
    fn form_state_keeps_edits_follows_controlled_values_and_drops_gone_fields() {
        let ViewModel::Form(f) = build(&form_tree()) else { panic!() };
        let mut st = FormState::default();
        st.sync(&f);
        assert_eq!(st.get("name"), json!("Ann"));
        st.set("name", json!("Bob"));
        st.set("bio", json!("typed"));
        st.sync(&f);
        assert_eq!(st.get("name"), json!("Bob"), "an uncontrolled edit survives a re-render");
        assert_eq!(st.get("bio"), json!("typed"), "a re-render with the same controlled value is not a change");
        st.sync(&echo(&f, "bio", json!("y")));
        assert_eq!(st.get("bio"), json!("y"), "a controlled field shows the extension's new value");
        let v = st.values(&f);
        assert_eq!(v["name"], json!("Bob"));
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["color"], json!("r"));
        assert!(!v.contains_key(""), "descriptions and separators have no value");
        let mut f2 = f.clone();
        f2.fields.retain(|x| x.id != "name");
        st.sync(&f2);
        assert_eq!(st.get("name"), Value::Null);
    }

    /// Measurement check (run by hand): `FERROCAST_MEASURE_TREES=<file written by
    /// `extensions-runtime/scripts/measure.mjs --dump-trees`> cargo test -p ferrocast-core -- --ignored
    /// measured_trees`. Every tree must become a view model the launcher draws (no unsupported parts).
    #[test]
    #[ignore]
    fn measured_trees_build_without_unsupported_parts() {
        let Ok(path) = std::env::var("FERROCAST_MEASURE_TREES") else { return };
        let all: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut bad = vec![];
        for e in &all {
            let name = e["name"].as_str().unwrap_or("?").to_string();
            let node: Node = serde_json::from_value(e["tree"].clone()).unwrap();
            match build(&node) {
                ViewModel::Unsupported(n) => bad.push(format!("{name}: root {n}")),
                ViewModel::List(l) if !l.unsupported.is_empty() => bad.push(format!("{name}: {:?}", l.unsupported)),
                _ => {}
            }
        }
        println!("{} trees, {} with unsupported parts", all.len(), bad.len());
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn list_level_actions_are_not_an_unsupported_part() {
        let t = tree(json!({"t":"#root","c":[{"t":"List","c":[
          {"t":"List.Item","p":{"_id":"0.0","title":"a"}},
          {"t":"ActionPanel","c":[{"t":"Action","p":{"_id":"0.1.0","title":"Go"}}]}]}]}));
        let ViewModel::List(l) = build(&t) else { panic!() };
        assert!(l.unsupported.is_empty(), "{:?}", l.unsupported);
    }

    #[test]
    fn dropdown_action_ids_round_trip_even_with_colons_in_the_value() {
        let id = dropdown_action_id("0.0", "a:b");
        assert_eq!(parse_dropdown_action(&id), Some(("0.0", "a:b")));
        assert_eq!(parse_dropdown_action("0.1.2"), None);
        assert_eq!(parse_dropdown_action("dd:nocolon"), None);
    }

    #[test]
    fn grid_degrades_to_a_list_of_its_items_with_content_as_icon() {
        let t = tree(json!({"t":"#root","c":[{"t":"Grid","p":{"columns":4,"inset":"medium","onSearchTextChange":true},"c":[
          {"t":"Grid.Section","p":{"title":"S"},"c":[{"t":"Grid.Item","p":{"_id":"0.0.0","id":"a","title":"A","subtitle":"sa","content":"icon:Star"},"c":[
            {"t":"ActionPanel","c":[{"t":"Action","p":{"_id":"0.0.0.0","title":"Go"}}]}]}]},
          {"t":"Grid.Item","p":{"_id":"0.1","title":"B","content":{"source":"x.png"}}},
          {"t":"Grid.Item","p":{"_id":"0.2","title":"C","content":{"value":{"source":"https://x.test/c.png"},"tooltip":"t"}}},
          {"t":"Grid.EmptyView","p":{"title":"Nothing"}}]}]}));
        let ViewModel::List(l) = build(&t) else { panic!("not a list") };
        assert!(l.is_grid && l.handles_search && !l.filtering);
        assert_eq!(l.sections.len(), 2);
        let a = &l.sections[0].items[0];
        assert_eq!((a.id.as_str(), a.title.as_str(), a.subtitle.as_str()), ("a", "A", "sa"));
        assert_eq!(a.icon, IconSpec::Builtin("Star".into()));
        assert_eq!(a.actions.len(), 1);
        assert_eq!(l.sections[1].items[0].icon, IconSpec::Path("x.png".into()));
        assert_eq!(l.sections[1].items[1].icon, IconSpec::Url("https://x.test/c.png".into()));
        assert_eq!(l.empty_title.as_deref(), Some("Nothing"));
        assert!(l.unsupported.is_empty());
    }

    #[test]
    fn list_dropdown_becomes_a_model_with_the_initial_selection() {
        let t = tree(json!({"t":"#root","c":[{"t":"List","c":[
          {"t":"List.Dropdown","p":{"_id":"0.0","tooltip":"Page","defaultValue":"b","onChange":true},"c":[
            {"t":"List.Dropdown.Item","p":{"value":"a","title":"A"}},
            {"t":"List.Dropdown.Section","p":{"title":"Sec"},"c":[{"t":"List.Dropdown.Item","p":{"value":"b","title":"B"}}]}]},
          {"t":"List.Item","p":{"_id":"0.1","title":"x"}}]}]}));
        let ViewModel::List(l) = build(&t) else { panic!() };
        let d = l.dropdown.expect("dropdown");
        assert_eq!((d.id.as_str(), d.tooltip.as_str(), d.selected.as_str()), ("0.0", "Page", "b"));
        assert_eq!(d.options.len(), 2);
        assert_eq!(d.options[1].section.as_deref(), Some("Sec"));
        assert!(d.on_change);
        assert!(l.unsupported.is_empty());
    }

    #[test]
    fn detail_metadata_is_appended_to_the_markdown() {
        let t = tree(json!({"t":"#root","c":[{"t":"Detail","p":{"markdown":"# Hi"},"c":[
          {"t":"Detail.Metadata","c":[
            {"t":"Detail.Metadata.Label","p":{"title":"Author","text":"Ann"}},
            {"t":"Detail.Metadata.Separator"},
            {"t":"Detail.Metadata.Link","p":{"title":"Site","text":"home","target":"https://x.test"}},
            {"t":"Detail.Metadata.TagList","p":{"title":"Tags"},"c":[{"t":"Detail.Metadata.TagList.Item","p":{"text":"one"}},{"t":"Detail.Metadata.TagList.Item","p":{"text":"two"}}]}]}]}]}));
        let ViewModel::Detail(d) = build(&t) else { panic!() };
        assert!(d.markdown.starts_with("# Hi"));
        assert!(d.markdown.contains("**Author**: Ann"));
        assert!(d.markdown.contains("**Site**: [home](https://x.test)"));
        assert!(d.markdown.contains("**Tags**: one, two"));
        assert!(d.markdown.contains("---"));
    }

    #[test]
    fn list_item_detail_gives_markdown_and_facts_for_the_side_pane() {
        let t = tree(json!({"t":"#root","c":[{"t":"List","p":{"isShowingDetail":true},"c":[
          {"t":"List.Item","p":{"_id":"0.0","title":"x"},"c":[
            {"t":"List.Item.Detail","p":{"markdown":"body"},"c":[{"t":"List.Item.Detail.Metadata","c":[
              {"t":"List.Item.Detail.Metadata.Label","p":{"title":"Size","text":"3 KB"}},
              {"t":"List.Item.Detail.Metadata.Separator"},
              {"t":"List.Item.Detail.Metadata.TagList","p":{"title":"T"},"c":[{"t":"List.Item.Detail.Metadata.TagList.Item","p":{"text":"k"}}]}]}]}]}]}]}));
        let ViewModel::List(l) = build(&t) else { panic!() };
        let d = l.sections[0].items[0].detail.clone().expect("detail");
        assert_eq!(d.markdown, "body");
        assert_eq!(d.meta, vec![("Size".to_string(), "3 KB".to_string()), ("T".to_string(), "k".to_string())]);
        assert!(l.unsupported.is_empty());
    }
}
