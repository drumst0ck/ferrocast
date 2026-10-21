//! Drift guard for the website data: the committed JSON must equal what the generator writes now.

#[allow(dead_code)]
#[path = "../examples/site_data.rs"]
mod site_data;

use ferrocast_core::{matrix, tools};
use serde_json::Value;
use std::path::PathBuf;

const HINT: &str = "run `cargo run -p ferrocast-core --example site_data`";

fn data_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/src/data").join(name)
}

fn committed(name: &str) -> String {
    std::fs::read_to_string(data_path(name)).unwrap_or_else(|e| panic!("cannot read {name} ({e}): {HINT}"))
}

fn parse(s: &str) -> Value {
    serde_json::from_str(s).expect("valid json")
}

#[test]
fn site_data_matches_matrix() {
    let (conversions, tools) = site_data::generate();
    assert!(committed("conversions.json") == conversions, "web/src/data/conversions.json is stale: {HINT}");
    assert!(committed("tools.json") == tools, "web/src/data/tools.json is stale: {HINT}");
}

#[test]
fn conversions_json_has_46_inputs_and_275_conversions() {
    let v = parse(&committed("conversions.json"));
    assert_eq!(v["inputs"], 46);
    assert_eq!(v["conversions"], 275);
    let mut inputs = 0;
    let mut outputs = 0;
    for f in v["families"].as_array().unwrap() {
        for i in f["inputs"].as_array().unwrap() {
            inputs += 1;
            outputs += i["outputs"].as_array().unwrap().len();
        }
    }
    assert_eq!((inputs, outputs), (46, 275), "{HINT}");
    assert_eq!(matrix::inputs().len(), 46);
}

#[test]
fn every_tool_appears_exactly_once_with_its_real_status() {
    let v = parse(&committed("tools.json"));
    let list = v["tools"].as_array().unwrap();
    assert_eq!(list.len(), 26);
    for t in tools::ToolId::ALL {
        let id = site_data::tool_key(t);
        let hits: Vec<_> = list.iter().filter(|x| x["id"] == id).collect();
        assert_eq!(hits.len(), 1, "tool {id} must appear exactly once");
        assert_eq!(hits[0]["status"], site_data::status_key(tools::implemented(t)), "status of {id}");
    }
    assert_eq!(v["actions"].as_array().unwrap().len(), 7, "Tangerine's six actions plus Zip");
}

#[test]
fn by_format_exts_come_from_the_matrix() {
    let v = parse(&committed("tools.json"));
    let known = matrix::inputs();
    let rows = v["byFormat"].as_array().unwrap();
    assert!(!rows.is_empty());
    for r in rows {
        let ext = r["ext"].as_str().unwrap();
        assert!(known.contains(&ext), "{ext} is not a matrix input");
        let single: Vec<_> = r["single"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let want: Vec<_> = tools::tools_for(ext, false).into_iter().map(site_data::tool_key).map(String::from).collect();
        assert_eq!(single, want, "single list for {ext}");
        let multi: Vec<_> = r["multi"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let want: Vec<_> = tools::tools_for(ext, true).into_iter().map(site_data::tool_key).map(String::from).collect();
        assert_eq!(multi, want, "multi list for {ext}");
    }
}
