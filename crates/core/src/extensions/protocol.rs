//! Line-delimited JSON between the app (host) and the Node sidecar (runtime): one JSON object per
//! line on stdin / stdout. stderr is free text and goes to the log.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A node of the render tree the runtime sends: an element (`t` = component name such as
/// `List.Item`), or a text node (`t` = `#text`, `p.text`). Function props never travel; the runtime
/// keeps them and the host refers to them by the string `_id` the runtime put in `p`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct Node {
    pub t: String,
    #[serde(default)]
    pub p: Map<String, Value>,
    #[serde(default)]
    pub c: Vec<Node>,
}

/// App -> runtime.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HostMsg {
    /// Start a command: load `entry` (the extension's compiled bundle) and render it.
    #[serde(rename_all = "camelCase")]
    Launch {
        entry: String,
        extension: String,
        command: String,
        title: String,
        /// `view` or `no-view`.
        mode: String,
        arguments: Map<String, Value>,
        preferences: Map<String, Value>,
        environment: Environment,
    },
    /// The search bar text changed.
    Search { text: String },
    /// The selected list item changed (`None`: nothing selected).
    Selection { id: Option<String> },
    /// The user triggered the action whose `_id` is `id`. A form's `SubmitForm` carries the values of
    /// its fields (by field id).
    Action {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        values: Option<Map<String, Value>>,
    },
    /// The user changed a form field (`id` is its id prop) or picked another dropdown option (`id` is
    /// the dropdown's `_id`); the runtime calls its `onChange`.
    Field { id: String, value: Value },
    /// The user went back one screen.
    Pop,
    /// Answer to a runtime `Call`.
    Result { id: u64, value: Value, error: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub extension_name: String,
    pub command_name: String,
    pub support_path: String,
    pub assets_path: String,
    pub is_development: bool,
    pub raycast_version: String,
    pub launch_type: String,
}

/// Runtime -> app.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuntimeMsg {
    /// The tree of the screen at navigation `depth` (1 = the command's first screen).
    Render {
        depth: usize,
        tree: Node,
    },
    Toast {
        #[serde(default)]
        style: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        message: String,
    },
    Hud {
        title: String,
    },
    /// A request the runtime waits on: answer with `HostMsg::Result` carrying the same id.
    Call {
        id: u64,
        method: String,
        #[serde(default)]
        params: Value,
    },
    /// Close the main window / leave the extension (no more output follows `Exit`).
    PopToRoot,
    CloseMainWindow,
    Exit,
}

/// One JSON line (no trailing newline) for the runtime's stdin.
pub fn encode_host(m: &HostMsg) -> String {
    serde_json::to_string(m).unwrap_or_else(|_| "{}".into())
}

/// Parse one stdout line of the runtime.
pub fn parse_runtime(line: &str) -> Result<RuntimeMsg, String> {
    serde_json::from_str(line.trim()).map_err(|e| format!("bad runtime message: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn host_messages_encode_as_single_tagged_lines() {
        let l = encode_host(&HostMsg::Search { text: "a\nb".into() });
        assert!(!l.contains('\n'));
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"search","text":"a\nb"}));
        let l = encode_host(&HostMsg::Selection { id: None });
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"selection","id":null}));
        let l = encode_host(&HostMsg::Action { id: "0.1".into(), values: None });
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"action","id":"0.1"}));
        assert_eq!(serde_json::from_str::<Value>(&encode_host(&HostMsg::Pop)).unwrap(), json!({"type":"pop"}));
        let mut vals = Map::new();
        vals.insert("name".into(), json!("Ann"));
        let l = encode_host(&HostMsg::Action { id: "0.1".into(), values: Some(vals) });
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"action","id":"0.1","values":{"name":"Ann"}}));
        let l = encode_host(&HostMsg::Field { id: "name".into(), value: json!(true) });
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"field","id":"name","value":true}));
        let l = encode_host(&HostMsg::Result { id: 7, value: json!("x"), error: None });
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"result","id":7,"value":"x","error":null}));
    }

    #[test]
    fn launch_uses_camel_case_fields() {
        let env = Environment {
            extension_name: "e".into(),
            command_name: "c".into(),
            support_path: "/s".into(),
            assets_path: "/a".into(),
            is_development: false,
            raycast_version: "1.0.0".into(),
            launch_type: "userInitiated".into(),
        };
        let l = encode_host(&HostMsg::Launch {
            entry: "/x/c.js".into(),
            extension: "e".into(),
            command: "c".into(),
            title: "C".into(),
            mode: "view".into(),
            arguments: Map::new(),
            preferences: Map::new(),
            environment: env,
        });
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["type"], "launch");
        assert_eq!(v["environment"]["supportPath"], "/s");
        assert_eq!(v["environment"]["isDevelopment"], false);
    }

    #[test]
    fn parses_runtime_messages() {
        let m = parse_runtime(r##"{"type":"render","depth":2,"tree":{"t":"#root","c":[{"t":"List","p":{"isLoading":true}}]}}"##).unwrap();
        match m {
            RuntimeMsg::Render { depth, tree } => {
                assert_eq!(depth, 2);
                assert_eq!(tree.c[0].t, "List");
                assert_eq!(tree.c[0].p["isLoading"], json!(true));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            parse_runtime(r#"{"type":"toast","style":"failure","title":"Oops"}"#).unwrap(),
            RuntimeMsg::Toast { style: "failure".into(), title: "Oops".into(), message: String::new() }
        );
        assert_eq!(
            parse_runtime(r#"{"type":"call","id":3,"method":"clipboard.copy","params":{"text":"hi"}}"#).unwrap(),
            RuntimeMsg::Call { id: 3, method: "clipboard.copy".into(), params: json!({"text":"hi"}) }
        );
        assert_eq!(parse_runtime(r#"{"type":"exit"}"#).unwrap(), RuntimeMsg::Exit);
    }

    #[test]
    fn garbage_and_unknown_lines_are_errors_not_panics() {
        assert!(parse_runtime("").is_err());
        assert!(parse_runtime("hello world").is_err());
        assert!(parse_runtime(r#"{"type":"nope"}"#).is_err());
    }
}
