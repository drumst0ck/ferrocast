//! Image attachments of notes.
//!
//! An image pasted, dropped or inserted into a note is copied to `<data dir>/attachments/` under a
//! content-hashed name (`<32 hex>.<ext>`, the original extension, lower case) and the note links it
//! with a relative Markdown image: `![name](attachments/<hash>.<ext>)`. Everything here is pure file
//! logic and decides what a link may point at: only that exact shape, never a path that leaves the
//! folder. Files no note links any more are removed after a grace period (see [`cleanup`]) so an
//! undo or a restored note still finds its image.

use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Folder name inside the data dir, and the first segment of every link.
pub const DIR: &str = "attachments";
/// Largest image accepted (bytes).
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;
/// An unreferenced file is kept this long (seconds) before it is deleted.
pub const GRACE_SECS: i64 = 7 * 24 * 3600;

const HASH_LEN: usize = 32;
const EXTS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "heic", "webp"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TooLarge { bytes: u64 },
    NotAnImage,
    Io(String),
}

impl Error {
    /// Text for the user.
    pub fn message(&self) -> String {
        match self {
            Error::TooLarge { bytes } => crate::i18n::trf(
                "Image too large ({size} MB). The limit is {max} MB.",
                &[("size", &bytes.div_ceil(1024 * 1024).to_string()), ("max", &(MAX_BYTES / (1024 * 1024)).to_string())],
            ),
            Error::NotAnImage => crate::i18n::tr("Not a supported image. Use PNG, JPEG, GIF, HEIC or WebP.").into_owned(),
            Error::Io(e) => crate::i18n::trf("Could not save the image: {e}", &[("e", e)]),
        }
    }
}

/// A stored attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// `<hash>.<ext>`.
    pub name: String,
    /// `attachments/<hash>.<ext>`, the link target.
    pub rel: String,
    /// Alt text for the link (the file's name without extension).
    pub alt: String,
}

/// The image type of `bytes` from their first bytes: `png`, `jpg`, `gif`, `heic` or `webp`.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1")
    {
        Some("heic")
    } else {
        None
    }
}

fn family(ext: &str) -> &str {
    if ext == "jpeg" {
        "jpg"
    } else {
        ext
    }
}

/// `<first 32 hex of sha256>.<ext>`.
pub fn hash_name(bytes: &[u8], ext: &str) -> String {
    let hex: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    format!("{}.{ext}", &hex[..HASH_LEN])
}

/// Is `name` exactly `<32 lower-case hex>.<known extension>`?
pub fn is_valid_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    stem.len() == HASH_LEN && stem.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && EXTS.contains(&ext)
}

/// Does `bytes` hash to the name (a file received from another Mac must be whole and the one asked for)?
pub fn content_matches(name: &str, bytes: &[u8]) -> bool {
    is_valid_name(name)
        && name
            .split_once('.')
            .is_some_and(|(stem, ext)| sniff(bytes).is_some_and(|k| family(k) == family(ext)) && hash_name(bytes, ext).starts_with(stem))
}

/// The file name of a link target: only `attachments/<valid name>` qualifies.
pub fn name_from_src(src: &str) -> Option<&str> {
    let name = src.strip_prefix("attachments/")?;
    is_valid_name(name).then_some(name)
}

/// Where a link points inside `root`, or `None` for anything that is not a plain attachment link.
pub fn resolve(root: &Path, src: &str) -> Option<PathBuf> {
    name_from_src(src).map(|n| root.join(DIR).join(n))
}

/// Like [`resolve`], but only for a regular file that is really in the folder (a symlink is not followed).
pub fn resolve_existing(root: &Path, src: &str) -> Option<PathBuf> {
    let p = resolve(root, src)?;
    let meta = std::fs::symlink_metadata(&p).ok()?;
    (meta.is_file() && !meta.file_type().is_symlink()).then_some(p)
}

/// `![alt](attachments/<name>)`.
pub fn markdown_link(alt: &str, name: &str) -> String {
    format!("![{alt}](attachments/{name})")
}

/// Alt text from a file name: the stem without brackets and line breaks; "image" when nothing is left.
pub fn alt_from_file_name(file_name: &str) -> String {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let clean: String = stem.chars().filter(|c| !matches!(c, '[' | ']' | '\\')).map(|c| if c.is_control() { ' ' } else { c }).collect();
    let clean = clean.trim().to_string();
    if clean.is_empty() {
        "image".into()
    } else {
        clean
    }
}

/// Copy `bytes` into the attachments folder. The type comes from the content; `ext_hint` (the
/// original extension) is kept when it agrees. An identical image already there is reused and its
/// age restarts.
pub fn store(root: &Path, bytes: &[u8], ext_hint: Option<&str>) -> Result<Stored, Error> {
    store_as(root, bytes, ext_hint, "image")
}

fn store_as(root: &Path, bytes: &[u8], ext_hint: Option<&str>, alt: &str) -> Result<Stored, Error> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::TooLarge { bytes: bytes.len() as u64 });
    }
    let kind = sniff(bytes).ok_or(Error::NotAnImage)?;
    let hint = ext_hint.map(str::to_ascii_lowercase);
    let ext = match hint.as_deref() {
        Some(h) if EXTS.contains(&h) && family(h) == kind => h,
        _ => kind,
    };
    let name = hash_name(bytes, ext);
    let dir = root.join(DIR);
    let path = dir.join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| Error::Io(e.to_string()))?;
    if path.is_file() {
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
    } else {
        let tmp = dir.join(format!(".{name}.tmp"));
        std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| Error::Io(e.to_string()))?;
    }
    Ok(Stored { rel: format!("{DIR}/{name}"), name, alt: alt.to_string() })
}

/// [`store`] for a file on disk (drag and drop, the open panel). The size is checked before the file is read.
pub fn store_file(root: &Path, file: &Path) -> Result<Stored, Error> {
    let meta = std::fs::metadata(file).map_err(|e| Error::Io(e.to_string()))?;
    if meta.len() > MAX_BYTES {
        return Err(Error::TooLarge { bytes: meta.len() });
    }
    // read at most one byte over the cap: a file that grows while it is read cannot pass it
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .and_then(|f| std::io::Read::read_to_end(&mut std::io::Read::take(f, MAX_BYTES + 1), &mut bytes))
        .map_err(|e| Error::Io(e.to_string()))?;
    let ext = file.extension().map(|e| e.to_string_lossy().to_string());
    let alt = alt_from_file_name(&file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    store_as(root, &bytes, ext.as_deref(), &alt)
}

/// Attachment file names the notes link (deleted notes in the trash count: they can be restored).
pub fn referenced<'a>(texts: impl IntoIterator<Item = &'a str>) -> HashSet<String> {
    let mut out = HashSet::new();
    for t in texts {
        if !t.contains(DIR) {
            continue;
        }
        for b in crate::richtext::parse(t).blocks {
            for s in b.spans {
                if let Some(n) = s.style.image.as_deref().and_then(name_from_src) {
                    out.insert(n.to_string());
                }
            }
        }
    }
    out
}

/// File inside the attachments folder that remembers since when each file has been unlinked.
pub const UNLINKED_FILE: &str = ".unlinked.json";

/// Is a file modified at `modified` at least `grace` seconds old at `now`? An unreadable time or one
/// in the future counts as new.
fn old_enough(modified: Option<std::time::SystemTime>, now_secs: i64, grace: i64) -> bool {
    modified.and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|d| now_secs - d.as_secs() as i64 >= grace)
}

/// Delete local attachment files that no note links. See [`cleanup_dir`]; the record of unlinked
/// files is `attachments/.unlinked.json`.
pub fn cleanup(root: &Path, referenced: &HashSet<String>, now_secs: i64, grace: i64) -> usize {
    let dir = root.join(DIR);
    cleanup_dir(&dir, &dir.join(UNLINKED_FILE), referenced, now_secs, grace)
}

/// Forget when the files in `linked` were first seen unlinked (they are linked again). Call it
/// whenever notes that link them are saved, so a later unlink starts a fresh 7 days even if no
/// cleanup ran in between. Does nothing when no record exists or none of them is in it.
pub fn forget_unlinked(root: &Path, linked: &HashSet<String>) {
    let state = root.join(DIR).join(UNLINKED_FILE);
    let Some(mut rec) = std::fs::read(&state).ok().and_then(|b| serde_json::from_slice::<std::collections::BTreeMap<String, i64>>(&b).ok())
    else {
        return;
    };
    let n = rec.len();
    rec.retain(|name, _| !linked.contains(name));
    if rec.len() != n {
        if let Ok(bytes) = serde_json::to_vec(&rec) {
            let tmp = state.with_extension("tmp");
            let _ = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &state));
        }
    }
}

/// Delete attachment files in `dir` that no note links, `grace` seconds after they were first seen
/// unlinked (not after the file's own age: a picture linked for a year is kept a week after its
/// link is removed, so an undo still finds it). `state` is a JSON file with that first time per
/// name; a name that is linked again loses its entry, a missing or unreadable record starts every
/// clock again, and a file whose modified time is unreadable or recent (it was just stored again
/// while this cleanup held older notes) is kept. Returns how many files were removed. Files that
/// are not named like attachments are never touched.
pub fn cleanup_dir(dir: &Path, state: &Path, referenced: &HashSet<String>, now_secs: i64, grace: i64) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    let before: std::collections::BTreeMap<String, i64> =
        std::fs::read(state).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let mut after = std::collections::BTreeMap::new();
    let mut removed = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !is_valid_name(&name) || referenced.contains(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let since = before.get(&name).copied().unwrap_or(now_secs);
        let due = before.contains_key(&name) && now_secs - since >= grace && old_enough(meta.modified().ok(), now_secs, grace);
        if due && std::fs::remove_file(e.path()).is_ok() {
            removed += 1;
            continue;
        }
        after.insert(name, since);
    }
    if after != before {
        if let Ok(bytes) = serde_json::to_vec(&after) {
            let tmp = state.with_extension("tmp");
            let _ = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, state));
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferrocast-attach-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
    const JPG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F'];

    fn heic() -> Vec<u8> {
        let mut v = vec![0, 0, 0, 24];
        v.extend_from_slice(b"ftypheic");
        v.extend_from_slice(&[0; 8]);
        v
    }

    fn webp() -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[10, 0, 0, 0]);
        v.extend_from_slice(b"WEBPVP8 ");
        v
    }

    #[test]
    fn content_decides_the_type_and_the_five_formats_are_accepted() {
        assert_eq!(sniff(PNG), Some("png"));
        assert_eq!(sniff(JPG), Some("jpg"));
        assert_eq!(sniff(b"GIF89a..."), Some("gif"));
        assert_eq!(sniff(&heic()), Some("heic"));
        assert_eq!(sniff(&webp()), Some("webp"));
        assert_eq!(sniff(b"%PDF-1.4"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn the_name_is_the_content_hash_with_the_original_extension() {
        let a = hash_name(PNG, "png");
        assert_eq!(a, hash_name(PNG, "png"));
        assert_ne!(a, hash_name(JPG, "jpg"));
        assert!(a.ends_with(".png") && a.len() == 32 + 4);
        assert!(is_valid_name(&a));
    }

    #[test]
    fn store_copies_once_keeps_the_extension_and_reports_the_link() {
        let root = tmp("store");
        let one = store(&root, PNG, Some("PNG")).unwrap();
        assert_eq!(one.rel, format!("attachments/{}", one.name));
        assert!(one.name.ends_with(".png"));
        assert_eq!(std::fs::read(root.join(&one.rel)).unwrap(), PNG);
        let two = store(&root, PNG, Some("png")).unwrap();
        assert_eq!(one.name, two.name);
        assert_eq!(std::fs::read_dir(root.join(DIR)).unwrap().count(), 1);
        // jpeg keeps the spelling the user had, a wrong hint falls back to the real type
        assert!(store(&root, JPG, Some("jpeg")).unwrap().name.ends_with(".jpeg"));
        assert!(store(&root, JPG, Some("png")).unwrap().name.ends_with(".jpg"));
        assert!(store(&root, &heic(), None).unwrap().name.ends_with(".heic"));
    }

    #[test]
    fn big_or_non_image_files_are_refused_with_a_clear_message() {
        let root = tmp("refuse");
        let big = vec![0u8; MAX_BYTES as usize + 1];
        let e = store(&root, &big, Some("png")).unwrap_err();
        assert_eq!(e, Error::TooLarge { bytes: MAX_BYTES + 1 });
        assert!(e.message().contains("20 MB"), "{}", e.message());
        let e = store(&root, b"hello world", Some("png")).unwrap_err();
        assert_eq!(e, Error::NotAnImage);
        assert!(e.message().contains("PNG"), "{}", e.message());
        assert!(!root.join(DIR).exists() || std::fs::read_dir(root.join(DIR)).unwrap().count() == 0);
    }

    #[test]
    fn a_file_over_the_cap_is_refused_before_it_is_read() {
        let root = tmp("filecap");
        let f = root.join("huge.png");
        let file = std::fs::File::create(&f).unwrap();
        file.set_len(MAX_BYTES + 1).unwrap();
        assert!(matches!(store_file(&root, &f), Err(Error::TooLarge { .. })));
        let ok = root.join("My Shot.PNG");
        std::fs::write(&ok, PNG).unwrap();
        let s = store_file(&root, &ok).unwrap();
        assert_eq!(s.alt, "My Shot");
        assert!(matches!(store_file(&root, &root.join("missing.png")), Err(Error::Io(_))));
    }

    #[test]
    fn markdown_link_is_relative_and_alt_is_safe() {
        assert_eq!(
            markdown_link("shot", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png"),
            "![shot](attachments/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png)"
        );
        assert_eq!(alt_from_file_name("a [b] (c)\n.png"), "a b (c)");
        assert_eq!(alt_from_file_name(".png"), "image");
    }

    #[test]
    fn links_never_leave_the_attachments_folder() {
        let root = tmp("paths");
        let name = "0123456789abcdef0123456789abcdef.png";
        assert_eq!(resolve(&root, &format!("attachments/{name}")), Some(root.join("attachments").join(name)));
        for bad in [
            "../secret.png",
            "attachments/../secret.png",
            "attachments/../../etc/passwd",
            "/etc/passwd",
            "/attachments/0123456789abcdef0123456789abcdef.png",
            "attachments/sub/0123456789abcdef0123456789abcdef.png",
            "attachments\\0123456789abcdef0123456789abcdef.png",
            "attachments/0123456789abcdef0123456789abcdef.png/..",
            "attachments/0123456789ABCDEF0123456789abcdef.png",
            "attachments/0123456789abcdef0123456789abcdef.exe",
            "attachments/short.png",
            "file:///etc/passwd",
            "https://x.test/a.png",
            "attachments/%2e%2e%2fx.png",
            "attachments/0123456789abcdef0123456789abcdef.png\0",
            "",
        ] {
            assert_eq!(resolve(&root, bad), None, "{bad}");
        }
    }

    #[test]
    fn a_symlink_in_the_folder_is_not_followed_out() {
        let root = tmp("symlink");
        let outside = tmp("outside");
        std::fs::write(outside.join("x.png"), PNG).unwrap();
        std::fs::create_dir_all(root.join(DIR)).unwrap();
        let name = "0123456789abcdef0123456789abcdef.png";
        std::os::unix::fs::symlink(outside.join("x.png"), root.join(DIR).join(name)).unwrap();
        assert_eq!(resolve_existing(&root, &format!("attachments/{name}")), None);
    }

    #[test]
    fn references_come_from_image_links_in_every_note_including_deleted_ones() {
        let a = "0123456789abcdef0123456789abcdef.png";
        let b = "fedcba9876543210fedcba9876543210.jpg";
        let notes = [
            format!("hello ![x](attachments/{a})"),
            format!("- item\n\n![y](attachments/{b})"),
            "![z](../secret.png) [t](attachments/0123456789abcdef0123456789abcdee.png)".to_string(),
        ];
        let set = referenced(notes.iter().map(String::as_str));
        assert_eq!(set, [a.to_string(), b.to_string()].into_iter().collect());
    }

    fn now_secs() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
    }

    fn set_age(path: &Path, days: i64) {
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs((now_secs() - days * 86_400) as u64)).unwrap();
    }

    const DAY: i64 = 86_400;

    #[test]
    fn the_grace_period_counts_from_when_the_link_went_away_not_from_the_files_age() {
        let root = tmp("clean");
        let keep = store(&root, PNG, Some("png")).unwrap().name;
        let gone = store(&root, &heic(), Some("heic")).unwrap().name;
        let back = store(&root, JPG, Some("jpg")).unwrap().name;
        for n in [&keep, &gone, &back] {
            set_age(&root.join(DIR).join(n), 30);
        }
        std::fs::write(root.join(DIR).join("notes.txt"), "not ours").unwrap();
        let now = now_secs();
        let refs = |names: &[&String]| -> HashSet<String> { names.iter().map(|n| (*n).clone()).collect() };
        // the first cleanup after the links went away only notes the time
        assert_eq!(cleanup(&root, &refs(&[&keep]), now, GRACE_SECS), 0);
        assert!(root.join(DIR).join(&gone).exists(), "a 30 day old file is kept right after it was unlinked (undo)");
        // still inside the grace period
        assert_eq!(cleanup(&root, &refs(&[&keep]), now + 6 * DAY, GRACE_SECS), 0);
        // `back` is linked again before the period is over: its clock is dropped
        assert_eq!(cleanup(&root, &refs(&[&keep, &back]), now + 6 * DAY + 1, GRACE_SECS), 0);
        // 8 days after it was first unlinked
        assert_eq!(cleanup(&root, &refs(&[&keep]), now + 8 * DAY, GRACE_SECS), 1);
        assert!(!root.join(DIR).join(&gone).exists());
        assert!(root.join(DIR).join(&keep).exists(), "referenced files stay");
        assert!(root.join(DIR).join(&back).exists(), "relinked in between: kept");
        assert!(root.join(DIR).join("notes.txt").exists(), "unknown files are never touched");
        // unlinking `back` again starts a fresh clock instead of reusing the old one
        assert_eq!(cleanup(&root, &refs(&[&keep]), now + 8 * DAY + 10, GRACE_SECS), 0);
        assert!(root.join(DIR).join(&back).exists());
        assert_eq!(cleanup(&root, &refs(&[&keep]), now + 16 * DAY, GRACE_SECS), 1);
    }

    #[test]
    fn a_relink_and_a_new_unlink_between_two_cleanups_start_a_fresh_week() {
        let root = tmp("relink");
        let name = store(&root, PNG, Some("png")).unwrap().name;
        set_age(&root.join(DIR).join(&name), 30);
        let now = now_secs();
        let none = HashSet::new();
        let linked: HashSet<String> = [name.clone()].into_iter().collect();
        assert_eq!(cleanup(&root, &none, now, GRACE_SECS), 0, "unlinked on day 0: the clock starts");
        // undo relinks it and the note is saved; on day 6 it is unlinked again, no cleanup in between
        forget_unlinked(&root, &linked);
        assert_eq!(cleanup(&root, &none, now + 6 * DAY, GRACE_SECS), 0, "the second week starts now");
        assert_eq!(cleanup(&root, &none, now + 7 * DAY, GRACE_SECS), 0, "one day after the second unlink: kept");
        assert!(root.join(DIR).join(&name).exists());
        assert_eq!(cleanup(&root, &none, now + 14 * DAY, GRACE_SECS), 1);
        // forgetting names that were never recorded, or with no record at all, is harmless
        forget_unlinked(&root, &linked);
        forget_unlinked(&tmp("norecord"), &linked);
    }

    #[test]
    fn a_missing_or_corrupt_record_never_deletes_anything() {
        let root = tmp("record");
        let name = store(&root, PNG, Some("png")).unwrap().name;
        set_age(&root.join(DIR).join(&name), 30);
        let now = now_secs();
        assert_eq!(cleanup(&root, &HashSet::new(), now, GRACE_SECS), 0);
        std::fs::write(root.join(DIR).join(UNLINKED_FILE), b"{ nope").unwrap();
        assert_eq!(cleanup(&root, &HashSet::new(), now + 30 * DAY, GRACE_SECS), 0, "unreadable record: start the clock again");
        assert!(root.join(DIR).join(&name).exists());
    }

    #[test]
    fn an_unreadable_modified_time_counts_as_new() {
        let now = 100 * DAY;
        assert!(!old_enough(None, now, GRACE_SECS));
        let at = |s: i64| Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s as u64));
        assert!(old_enough(at(now - 8 * DAY), now, GRACE_SECS));
        assert!(!old_enough(at(now - DAY), now, GRACE_SECS));
        assert!(!old_enough(at(now + DAY), now, GRACE_SECS), "a time in the future is new");
    }

    #[test]
    fn pasting_the_same_image_again_keeps_it_even_for_a_cleanup_with_older_notes() {
        let root = tmp("touch");
        let name = store(&root, PNG, Some("png")).unwrap().name;
        let now = now_secs();
        set_age(&root.join(DIR).join(&name), 30);
        assert_eq!(cleanup(&root, &HashSet::new(), now - 9 * DAY, GRACE_SECS), 0);
        // pasted again while a cleanup that started before the paste still holds the old notes
        store(&root, PNG, Some("png")).unwrap();
        assert_eq!(cleanup(&root, &HashSet::new(), now, GRACE_SECS), 0);
        assert!(root.join(DIR).join(&name).exists());
    }

    #[test]
    fn a_downloaded_file_must_match_its_hash() {
        let name = hash_name(PNG, "png");
        assert!(content_matches(&name, PNG));
        assert!(!content_matches(&name, JPG));
        assert!(!content_matches("../x.png", PNG));
    }
}
