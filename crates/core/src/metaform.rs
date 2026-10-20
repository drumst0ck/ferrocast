//! "Edit metadata" model (Tangerine S11): read ffprobe output into fields grouped by file, track
//! and chapter, and build the ffmpeg arguments that rewrite tags with a stream copy. Pure.
//! Chapters are shown but read-only: ffmpeg has no per-chapter tag option on the command line.

/// Where a field lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Group {
    File,
    /// A stream: its ffprobe index and `codec_type` (video, audio, subtitle...).
    Track {
        index: usize,
        kind: String,
    },
    /// A chapter, by 0-based position.
    Chapter(usize),
}

/// One tag as shown in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub group: Group,
    pub key: String,
    pub value: String,
    pub editable: bool,
}

/// One requested change; `value: None` removes the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaEdit {
    pub group: Group,
    pub key: String,
    pub value: Option<String>,
}

/// Technical tags are written by the muxer and are not the user's to change.
pub fn is_editable_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    !(matches!(
        k.as_str(),
        "major_brand"
            | "minor_version"
            | "compatible_brands"
            | "encoder"
            | "vendor_id"
            | "handler_name"
            | "duration"
            | "number_of_frames"
            | "number_of_bytes"
            | "bps"
    ) || k.starts_with("_statistics"))
}

fn tag_fields(tags: &serde_json::Value, group: &Group, editable_group: bool, out: &mut Vec<Field>) {
    let Some(map) = tags.as_object() else { return };
    for (key, v) in map {
        let value = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        out.push(Field { group: group.clone(), key: key.clone(), value, editable: editable_group && is_editable_key(key) });
    }
}

/// Fields of `ffprobe -show_format -show_streams -show_chapters -of json`: file tags, then each
/// stream's tags in stream order, then chapter tags. Tag order within a group is the JSON
/// object's (serde_json sorts keys).
pub fn parse_ffprobe(json: &str) -> Result<Vec<Field>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if !v.is_object() {
        return Err("unexpected ffprobe output".into());
    }
    let mut out = vec![];
    tag_fields(&v["format"]["tags"], &Group::File, true, &mut out);
    for (i, s) in v["streams"].as_array().into_iter().flatten().enumerate() {
        let index = s["index"].as_u64().map_or(i, |n| n as usize);
        let kind = s["codec_type"].as_str().unwrap_or("unknown").to_string();
        tag_fields(&s["tags"], &Group::Track { index, kind }, true, &mut out);
    }
    for (i, c) in v["chapters"].as_array().into_iter().flatten().enumerate() {
        tag_fields(&c["tags"], &Group::Chapter(i), false, &mut out);
    }
    Ok(out)
}

/// Refuses edits ffmpeg cannot apply (chapters) or that would corrupt an argument.
pub fn validate(edits: &[MetaEdit]) -> Result<(), String> {
    for e in edits {
        if matches!(e.group, Group::Chapter(_)) {
            return Err("chapter titles cannot be edited yet (read-only)".into());
        }
        if e.key.is_empty() || e.key.contains(['=', '\0', '\n']) {
            return Err(format!("not a valid tag name: {:?}", e.key));
        }
        if e.value.as_deref().is_some_and(|v| v.contains('\0')) {
            return Err(format!("the value of {} contains a NUL character", e.key));
        }
    }
    Ok(())
}

/// Arguments between `ffmpeg -nostdin ...` and the end: copy every stream, then either rewrite the
/// given tags (everything else is kept) or, with `remove_all`, drop all tags and chapters.
/// A removed key is set to an empty value, which ffmpeg treats as deletion. Chapter edits are
/// ignored here (see [`validate`]). `-fflags +bitexact` also stops ffmpeg writing its own
/// `encoder` tag when stripping.
pub fn ffmpeg_args(input: &str, edits: &[MetaEdit], remove_all: bool, out: &str) -> Vec<String> {
    let mut v: Vec<String> = ["-i", input, "-map", "0", "-c", "copy"].iter().map(|x| x.to_string()).collect();
    if remove_all {
        v.extend(["-map_metadata", "-1", "-map_chapters", "-1", "-fflags", "+bitexact"].map(String::from));
    } else {
        v.extend(["-map_metadata", "0", "-map_chapters", "0"].map(String::from));
        for e in edits {
            let flag = match &e.group {
                Group::File => "-metadata".to_string(),
                Group::Track { index, .. } => format!("-metadata:s:{index}"),
                Group::Chapter(_) => continue,
            };
            v.push(flag);
            v.push(format!("{}={}", e.key, e.value.as_deref().unwrap_or("")));
        }
    }
    v.push(out.into());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real `ffprobe -show_format -show_streams -show_chapters -of json` output of an mkv (video,
    // audio, 2 chapters); only the `disposition` blocks were removed for brevity.
    const MKV: &str = r#"{"streams":[
      {"index":0,"codec_type":"video","tags":{"title":"Main","ENCODER":"Lavc62.11.100 libx264","DURATION":"00:00:02.000000000"}},
      {"index":1,"codec_type":"audio","tags":{"language":"spa","ENCODER":"Lavc62.11.100 aac","DURATION":"00:00:02.023000000"}}],
     "chapters":[{"id":1,"start_time":"0.000000","tags":{"title":"Intro"}},{"id":2,"start_time":"1.000000","tags":{"title":"Outro"}}],
     "format":{"filename":"f.mkv","nb_streams":2,"tags":{"title":"Título ñ","ENCODER":"Lavf62.3.100"}}}"#;

    const MP4: &str = r#"{"streams":[{"index":0,"codec_type":"video","tags":{"language":"und","handler_name":"VideoHandler","vendor_id":"[0][0][0][0]","encoder":"Lavc62.11.100 libx264"}}],
     "chapters":[],
     "format":{"tags":{"major_brand":"isom","minor_version":"512","compatible_brands":"isomiso2avc1mp41","title":"T","artist":"A","encoder":"Lavf62.3.100","comment":"hi"}}}"#;

    fn f(fields: &[Field], key: &str) -> Field {
        fields.iter().find(|x| x.key == key).cloned().unwrap_or_else(|| panic!("no {key}"))
    }

    #[test]
    fn groups_come_in_file_track_chapter_order() {
        let v = parse_ffprobe(MKV).unwrap();
        let groups: Vec<_> = v.iter().map(|x| x.group.clone()).collect();
        let first_track = groups.iter().position(|g| matches!(g, Group::Track { .. })).unwrap();
        let first_chapter = groups.iter().position(|g| matches!(g, Group::Chapter(_))).unwrap();
        assert!(groups[..first_track].iter().all(|g| *g == Group::File));
        assert!(first_track < first_chapter);
        assert!(groups[first_track..first_chapter].iter().all(|g| matches!(g, Group::Track { .. })));
        let tracks: Vec<_> =
            groups.iter().filter_map(|g| if let Group::Track { index, kind } = g { Some((*index, kind.clone())) } else { None }).collect();
        assert_eq!(tracks.first(), Some(&(0, "video".to_string())));
        assert_eq!(tracks.last(), Some(&(1, "audio".to_string())));
        let chapters: Vec<_> = groups.iter().filter_map(|g| if let Group::Chapter(i) = g { Some(*i) } else { None }).collect();
        assert_eq!(chapters, vec![0, 1]);
    }

    #[test]
    fn values_and_case_are_kept_and_unicode_survives() {
        let v = parse_ffprobe(MKV).unwrap();
        let title = v.iter().find(|x| x.group == Group::File && x.key == "title").unwrap();
        assert_eq!(title.value, "Título ñ");
        assert!(title.editable);
        assert!(v.iter().any(|x| x.key == "ENCODER"), "original case kept");
        let intro = v.iter().find(|x| x.group == Group::Chapter(0)).unwrap();
        assert_eq!((intro.key.as_str(), intro.value.as_str()), ("title", "Intro"));
    }

    #[test]
    fn technical_keys_are_read_only() {
        let v = parse_ffprobe(MP4).unwrap();
        for k in ["major_brand", "minor_version", "compatible_brands", "encoder", "vendor_id", "handler_name"] {
            assert!(!f(&v, k).editable, "{k}");
        }
        for k in ["title", "artist", "comment", "language"] {
            assert!(f(&v, k).editable, "{k}");
        }
        let m = parse_ffprobe(MKV).unwrap();
        assert!(!m.iter().find(|x| x.key == "ENCODER").unwrap().editable);
        assert!(!m.iter().find(|x| x.key == "DURATION").unwrap().editable);
        assert!(!is_editable_key("_STATISTICS_TAGS"));
        assert!(!is_editable_key("number_of_frames"));
        assert!(!is_editable_key("Number_Of_Bytes"));
        assert!(!is_editable_key("bps"));
        assert!(is_editable_key("bpsx"));
    }

    #[test]
    fn chapter_fields_are_read_only() {
        let v = parse_ffprobe(MKV).unwrap();
        assert!(v.iter().filter(|x| matches!(x.group, Group::Chapter(_))).all(|x| !x.editable));
    }

    #[test]
    fn non_string_values_are_stringified() {
        let v = parse_ffprobe(r#"{"format":{"tags":{"track":3,"gapless":true,"x":null,"r":1.5}}}"#).unwrap();
        assert_eq!(f(&v, "track").value, "3");
        assert_eq!(f(&v, "gapless").value, "true");
        assert_eq!(f(&v, "x").value, "");
        assert_eq!(f(&v, "r").value, "1.5");
    }

    #[test]
    fn missing_sections_and_bad_json() {
        assert!(parse_ffprobe("{}").unwrap().is_empty());
        assert!(parse_ffprobe(r#"{"chapters":[],"streams":[{"index":0}]}"#).unwrap().is_empty());
        assert!(parse_ffprobe("not json").is_err());
        assert!(parse_ffprobe("").is_err());
        assert!(parse_ffprobe("[1,2]").is_err(), "not an object");
    }

    fn edit(group: Group, key: &str, value: Option<&str>) -> MetaEdit {
        MetaEdit { group, key: key.into(), value: value.map(String::from) }
    }

    #[test]
    fn edit_keeps_everything_else_and_copies_streams() {
        let a = ffmpeg_args("in.mp4", &[edit(Group::File, "title", Some("Nuevo ñ = x"))], false, "out.mp4");
        assert_eq!(
            a,
            [
                "-i",
                "in.mp4",
                "-map",
                "0",
                "-c",
                "copy",
                "-map_metadata",
                "0",
                "-map_chapters",
                "0",
                "-metadata",
                "title=Nuevo ñ = x",
                "out.mp4"
            ]
        );
    }

    #[test]
    fn removing_a_key_sets_it_empty() {
        let a = ffmpeg_args("in.mp4", &[edit(Group::File, "artist", None)], false, "out.mp4");
        assert!(a.windows(2).any(|w| w == ["-metadata", "artist="]));
        assert!(a.windows(2).any(|w| w == ["-map_metadata", "0"]));
    }

    #[test]
    fn track_edit_addresses_the_stream() {
        let g = Group::Track { index: 1, kind: "audio".into() };
        let a = ffmpeg_args("in.mkv", &[edit(g.clone(), "language", Some("spa")), edit(g, "title", None)], false, "o.mkv");
        assert!(a.windows(2).any(|w| w == ["-metadata:s:1", "language=spa"]));
        assert!(a.windows(2).any(|w| w == ["-metadata:s:1", "title="]));
    }

    #[test]
    fn chapter_edits_are_never_passed_and_are_rejected() {
        let e = edit(Group::Chapter(0), "title", Some("x"));
        let a = ffmpeg_args("in.mkv", std::slice::from_ref(&e), false, "o.mkv");
        assert!(!a.iter().any(|x| x == "-metadata" || x.contains("title")));
        assert!(validate(&[e]).unwrap_err().contains("chapter"));
        assert!(validate(&[edit(Group::File, "", Some("x"))]).is_err());
        assert!(validate(&[edit(Group::File, "a=b", Some("x"))]).is_err());
        assert!(validate(&[edit(Group::File, "a", Some("x\0y"))]).is_err());
        assert!(validate(&[edit(Group::File, "title", Some("ñ")), edit(Group::File, "artist", None)]).is_ok());
    }

    #[test]
    fn remove_all_drops_global_metadata_and_chapters() {
        let a = ffmpeg_args("in.mp4", &[edit(Group::File, "title", Some("ignored"))], true, "out.mp4");
        assert_eq!(
            a,
            ["-i", "in.mp4", "-map", "0", "-c", "copy", "-map_metadata", "-1", "-map_chapters", "-1", "-fflags", "+bitexact", "out.mp4"]
        );
    }
}
