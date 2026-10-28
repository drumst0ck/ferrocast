//! Node.js discovery (>= 22) and the opt-in download of the official build.
//!
//! Nothing here runs on its own: `find_node` only executes `node --version` on candidates, and
//! `download_node` is called solely from an explicit Settings button.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Oldest Node major the runtime supports.
pub const MIN_MAJOR: u32 = 22;
/// Version fetched by the Settings button (an official v22 release from nodejs.org).
pub const PINNED_VERSION: &str = "v22.23.4";
const DIST: &str = "https://nodejs.org/dist";

pub type Version = (u32, u32, u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    pub path: PathBuf,
    pub version: Version,
}

/// `v22.3.1` (with or without the `v`, trailing newline allowed) -> (22, 3, 1). Pre-release suffixes
/// such as `-nightly` are ignored.
pub fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.splitn(3, '.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch: String = it.next()?.chars().take_while(|c| c.is_ascii_digit()).collect();
    Some((major, minor, patch.parse().ok()?))
}

pub fn is_supported(v: Version) -> bool {
    v.0 >= MIN_MAJOR
}

/// The supported candidate with the highest version; ties keep the earliest (PATH order).
pub fn pick_best(cands: Vec<NodeInfo>) -> Option<NodeInfo> {
    let mut best: Option<NodeInfo> = None;
    for c in cands.into_iter().filter(|c| is_supported(c.version)) {
        if best.as_ref().is_none_or(|b| c.version > b.version) {
            best = Some(c);
        }
    }
    best
}

/// Where to look for `node`, in priority order: the managed copy, every PATH entry, then the usual
/// Homebrew locations. Duplicates are removed.
pub fn candidate_paths(path_env: Option<&std::ffi::OsStr>, managed: &Path) -> Vec<PathBuf> {
    let mut out = vec![managed.to_path_buf()];
    if let Some(p) = path_env {
        out.extend(std::env::split_paths(p).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join("node")));
    }
    out.push(PathBuf::from("/opt/homebrew/bin/node"));
    out.push(PathBuf::from("/usr/local/bin/node"));
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// Run `<path> --version` (5 s limit) and parse it.
pub fn probe(path: &Path) -> Option<NodeInfo> {
    let mut child = std::process::Command::new(path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait().ok()? {
            Some(st) if st.success() => break,
            Some(_) => return None,
            None if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    Some(NodeInfo { path: path.to_path_buf(), version: parse_version(&out)? })
}

/// Path of the Node this app downloaded, inside its data folder.
pub fn managed_node_path(data_dir: &Path) -> PathBuf {
    data_dir.join("runtime").join("node")
}

/// Best Node >= 22 on this Mac, or `None`.
pub fn find_node(data_dir: &Path) -> Option<NodeInfo> {
    let managed = managed_node_path(data_dir);
    let path_env = std::env::var_os("PATH");
    let found = candidate_paths(path_env.as_deref(), &managed).into_iter().filter(|p| p.is_file()).filter_map(|p| probe(&p)).collect();
    pick_best(found)
}

/// `node-v22.23.4-darwin-arm64.tar.xz` for this machine's architecture.
pub fn tarball_name(version: &str, arch: &str) -> Option<String> {
    let arch = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        _ => return None,
    };
    Some(format!("node-{version}-darwin-{arch}.tar.xz"))
}

/// SHA-256 of the official v22.23.4 darwin archives, copied from nodejs.org/dist/v22.23.4/SHASUMS256.txt
/// and pinned here so a compromised download server cannot supply its own checksum file.
pub fn pinned_sha256(file: &str) -> Option<&'static str> {
    match file {
        "node-v22.23.4-darwin-arm64.tar.gz" => Some("c9e1dbc748b33ed2d14f42445191ecaf14c56259c7a9a26d0042c49ec2b66428"),
        "node-v22.23.4-darwin-arm64.tar.xz" => Some("037650d0839f457747396f052073fb48333d31bf5abd3c98b74da20d89ebb1ad"),
        "node-v22.23.4-darwin-x64.tar.gz" => Some("efcc2b0db6f94bf53eddb038fe2b370277a042bd29e4f19b8f8e23598795b819"),
        "node-v22.23.4-darwin-x64.tar.xz" => Some("f42abca7aaac61efc57d49fb42ea84fc53bbeba9a749cb03a291ce6e4e7a69c1"),
        _ => None,
    }
}

/// Check a downloaded archive against the pinned hash. When a SHASUMS256.txt was fetched too and lists
/// the file, it must agree (a cross-check only: it is never trusted on its own).
pub fn verify_download(bytes: &[u8], file: &str, shasums: Option<&str>) -> Result<(), String> {
    let pin = pinned_sha256(file).ok_or_else(|| format!("no pinned checksum for {file}"))?;
    verify_sha256(bytes, pin)?;
    if let Some(listed) = shasums.and_then(|t| parse_shasums(t, file)) {
        if !listed.eq_ignore_ascii_case(pin) {
            return Err("nodejs.org's SHASUMS256.txt disagrees with the pinned checksum".into());
        }
    }
    Ok(())
}

/// The hash listed for `file` in an official `SHASUMS256.txt` (`<hex>  <name>` lines).
pub fn parse_shasums(text: &str, file: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (hash, name) = l.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file && !hash.is_empty()).then(|| hash.to_string())
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Ok when the SHA-256 of `bytes` equals `expected` (hex, case-insensitive).
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), String> {
    let got = sha256_hex(bytes);
    if !expected.is_empty() && got.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(format!("checksum mismatch: expected {expected}, got {got}"))
    }
}

fn curl(url: &str, out: &Path) -> Result<(), String> {
    let st = std::process::Command::new("/usr/bin/curl")
        .args(["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--max-time", "600", "-o"])
        .arg(out)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if st.status.success() {
        Ok(())
    } else {
        Err(format!("download failed: {}", String::from_utf8_lossy(&st.stderr).trim()))
    }
}

/// Download the pinned official Node, verify it against the pinned SHA-256 (a fetched SHASUMS256.txt
/// is only a cross-check) and extract only
/// `bin/node` to `managed_node_path(data_dir)`. Call it only from an explicit user action.
pub fn download_node(data_dir: &Path) -> Result<PathBuf, String> {
    let name = tarball_name(PINNED_VERSION, std::env::consts::ARCH).ok_or("this Mac's CPU is not supported")?;
    let work = data_dir.join("runtime").join("download");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = (|| {
        // the pinned hash is the authority; SHASUMS256.txt is only a cross-check when it can be fetched
        let sums = work.join("SHASUMS256.txt");
        let sums_text =
            curl(&format!("{DIST}/{PINNED_VERSION}/SHASUMS256.txt"), &sums).ok().and_then(|_| std::fs::read_to_string(&sums).ok());
        let archive = work.join(&name);
        curl(&format!("{DIST}/{PINNED_VERSION}/{name}"), &archive)?;
        verify_download(&std::fs::read(&archive).map_err(|e| e.to_string())?, &name, sums_text.as_deref())?;
        let member = format!("{}/bin/node", name.trim_end_matches(".tar.xz"));
        let st = std::process::Command::new("/usr/bin/tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&work)
            .arg(&member)
            .output()
            .map_err(|e| format!("cannot run tar: {e}"))?;
        if !st.status.success() {
            return Err(format!("extract failed: {}", String::from_utf8_lossy(&st.stderr).trim()));
        }
        let dest = managed_node_path(data_dir);
        std::fs::rename(work.join(&member), &dest).map_err(|e| e.to_string())?;
        probe(&dest).filter(|n| is_supported(n.version)).ok_or("the downloaded node does not run")?;
        Ok(dest)
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ni(p: &str, v: Version) -> NodeInfo {
        NodeInfo { path: PathBuf::from(p), version: v }
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("v22.3.1\n"), Some((22, 3, 1)));
        assert_eq!(parse_version("25.4.0"), Some((25, 4, 0)));
        assert_eq!(parse_version("v23.0.0-nightly2024"), Some((23, 0, 0)));
        assert_eq!(parse_version("v22"), None);
        assert_eq!(parse_version("hello"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn support_threshold_is_22() {
        assert!(!is_supported((21, 99, 99)));
        assert!(is_supported((22, 0, 0)));
        assert!(is_supported((25, 4, 0)));
    }

    #[test]
    fn picks_highest_supported_and_ignores_old() {
        let best = pick_best(vec![ni("/a", (20, 1, 0)), ni("/b", (22, 5, 0)), ni("/c", (25, 4, 0)), ni("/d", (25, 4, 0))]);
        assert_eq!(best, Some(ni("/c", (25, 4, 0))));
        assert_eq!(pick_best(vec![ni("/a", (18, 0, 0)), ni("/b", (21, 7, 0))]), None);
        assert_eq!(pick_best(vec![]), None);
    }

    #[test]
    fn candidates_cover_path_and_homebrew_without_duplicates() {
        let c = candidate_paths(Some(std::ffi::OsStr::new("/usr/bin:/opt/homebrew/bin:/x")), Path::new("/data/runtime/node"));
        assert_eq!(c[0], PathBuf::from("/data/runtime/node"));
        assert!(c.contains(&PathBuf::from("/usr/bin/node")));
        assert!(c.contains(&PathBuf::from("/usr/local/bin/node")));
        assert_eq!(c.iter().filter(|p| **p == Path::new("/opt/homebrew/bin/node")).count(), 1);
    }

    #[test]
    fn tarball_names_per_arch() {
        assert_eq!(tarball_name("v22.23.4", "aarch64").as_deref(), Some("node-v22.23.4-darwin-arm64.tar.xz"));
        assert_eq!(tarball_name("v22.23.4", "x86_64").as_deref(), Some("node-v22.23.4-darwin-x64.tar.xz"));
        assert_eq!(tarball_name("v22.23.4", "riscv64"), None);
    }

    #[test]
    fn shasums_lookup_matches_exact_file_name() {
        let t = "aaaa1111  node-v22.23.4-darwin-arm64.tar.gz\nbbbb2222  node-v22.23.4-darwin-arm64.tar.xz\ncccc3333 *node-v22.23.4-linux-x64.tar.gz\n";
        assert_eq!(parse_shasums(t, "node-v22.23.4-darwin-arm64.tar.gz").as_deref(), Some("aaaa1111"));
        assert_eq!(parse_shasums(t, "node-v22.23.4-linux-x64.tar.gz").as_deref(), Some("cccc3333"));
        assert_eq!(parse_shasums(t, "node-v22.23.4-darwin-arm64"), None);
    }

    const ARM_GZ: &str = "c9e1dbc748b33ed2d14f42445191ecaf14c56259c7a9a26d0042c49ec2b66428";

    #[test]
    fn pinned_hashes_cover_the_four_darwin_archives() {
        assert_eq!(pinned_sha256("node-v22.23.4-darwin-arm64.tar.gz"), Some(ARM_GZ));
        assert_eq!(
            pinned_sha256("node-v22.23.4-darwin-arm64.tar.xz"),
            Some("037650d0839f457747396f052073fb48333d31bf5abd3c98b74da20d89ebb1ad")
        );
        assert_eq!(
            pinned_sha256("node-v22.23.4-darwin-x64.tar.gz"),
            Some("efcc2b0db6f94bf53eddb038fe2b370277a042bd29e4f19b8f8e23598795b819")
        );
        assert_eq!(
            pinned_sha256("node-v22.23.4-darwin-x64.tar.xz"),
            Some("f42abca7aaac61efc57d49fb42ea84fc53bbeba9a749cb03a291ce6e4e7a69c1")
        );
        assert_eq!(pinned_sha256("node-v22.99.0-darwin-arm64.tar.gz"), None);
    }

    #[test]
    fn download_is_checked_against_the_pin_not_the_servers_checksum_file() {
        let name = "node-v22.23.4-darwin-arm64.tar.gz";
        // an attacker's tarball with a matching attacker-supplied SHASUMS256.txt is refused
        let evil = b"evil tarball";
        let fake_sums = format!("{}  {name}\n", sha256_hex(evil));
        assert!(verify_download(evil, name, Some(&fake_sums)).is_err());
        assert!(verify_download(evil, name, None).is_err());
        // an unknown file name has no pin: refused
        assert!(verify_download(evil, "node-v22.99.0-darwin-arm64.tar.gz", None).is_err());
    }

    #[test]
    fn checksum_verification_accepts_match_and_rejects_tamper() {
        let data = b"hello";
        let good = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert_eq!(sha256_hex(data), good);
        assert!(verify_sha256(data, good).is_ok());
        assert!(verify_sha256(data, &good.to_uppercase()).is_ok());
        assert!(verify_sha256(b"hellp", good).is_err());
        assert!(verify_sha256(data, "").is_err());
    }

    /// Network test, run by hand: `cargo test -p ferrocast-core downloads_and_runs_the_real_node -- --ignored`.
    /// Downloads the pinned Node into a scratch data dir, checks it runs, then deletes the scratch dir.
    #[test]
    #[ignore = "downloads about 26 MB from nodejs.org"]
    fn downloads_and_runs_the_real_node() {
        let dir = std::env::temp_dir().join(format!("fc-node-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = download_node(&dir).expect("download, checksum, extract and run");
        assert_eq!(path, managed_node_path(&dir));
        let out = std::process::Command::new(&path).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), PINNED_VERSION);
        assert!(!dir.join("runtime").join("download").exists(), "the work folder is removed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
