//! Turns names chosen by a remote peer into paths that are safe to create.
use std::path::{Path, PathBuf};

const MAX_COMPONENT_BYTES: usize = 200;
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `name` may use `/` or `\` for folders. The result is always a non-empty
/// relative path whose components are all plain names.
pub fn sanitize_relative(name: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for raw in name.split(['/', '\\']) {
        if raw == "." || raw == ".." {
            continue;
        }
        if let Some(c) = sanitize_component(raw) {
            out.push(c);
        }
    }
    if out.as_os_str().is_empty() {
        out.push("file");
    }
    out
}

fn sanitize_component(raw: &str) -> Option<String> {
    let mut s: String = raw
        .chars()
        .map(|c| {
            let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
            if c.is_control() || bidi || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    if s.len() > MAX_COMPONENT_BYTES {
        let mut end = MAX_COMPONENT_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    // Windows silently strips trailing dots/spaces, which can collide names.
    let s = s.trim_start().trim_end_matches(['.', ' ']).to_string();
    if s.is_empty() {
        return None;
    }
    let stem = s.split('.').next().unwrap_or("");
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Some(format!("_{s}"));
    }
    Some(s)
}

/// `dest/rel`, or `dest/<stem> (n).<ext>` if that already exists.
pub fn unique_path(dest: &Path, rel: &Path) -> PathBuf {
    let first = dest.join(rel);
    if !first.exists() {
        return first;
    }
    let parent = first.parent().unwrap_or(dest).to_path_buf();
    let stem = first
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = first
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    (1..)
        .map(|n| parent.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("unbounded range")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::{Component, Path, PathBuf};

    fn p(parts: &[&str]) -> PathBuf {
        parts.iter().collect()
    }

    #[test]
    fn keeps_folders() {
        assert_eq!(
            sanitize_relative("album/sub/b.jpg"),
            p(&["album", "sub", "b.jpg"])
        );
        assert_eq!(sanitize_relative("album\\b.jpg"), p(&["album", "b.jpg"]));
    }

    #[test]
    fn drops_traversal_and_roots() {
        assert_eq!(sanitize_relative("../../etc/passwd"), p(&["etc", "passwd"]));
        assert_eq!(sanitize_relative("/abs/x.txt"), p(&["abs", "x.txt"]));
        assert_eq!(sanitize_relative("a/./../b"), p(&["a", "b"]));
    }

    #[test]
    fn windows_hazards() {
        assert_eq!(
            sanitize_relative("C:\\Windows\\x"),
            p(&["C_", "Windows", "x"])
        );
        assert_eq!(sanitize_relative("CON.txt"), p(&["_CON.txt"]));
        assert_eq!(sanitize_relative("lpt1"), p(&["_lpt1"]));
        assert_eq!(sanitize_relative("file. . "), p(&["file"]));
        assert_eq!(sanitize_relative("a?b*c|d\"e<f>g"), p(&["a_b_c_d_e_f_g"]));
    }

    #[test]
    fn control_and_bidi_chars_replaced() {
        assert_eq!(sanitize_relative("a\u{0}b\nc"), p(&["a_b_c"]));
        assert_eq!(
            sanitize_relative("invoice\u{202E}fdp.exe"),
            p(&["invoice_fdp.exe"])
        );
    }

    #[test]
    fn empty_becomes_file() {
        assert_eq!(sanitize_relative(""), p(&["file"]));
        assert_eq!(sanitize_relative("../.."), p(&["file"]));
        assert_eq!(sanitize_relative("..."), p(&["file"]));
    }

    #[test]
    fn long_names_truncated_on_char_boundary() {
        let name = "ก".repeat(300);
        let out = sanitize_relative(&name);
        assert!(out.to_str().unwrap().len() <= 200);
    }

    #[test]
    fn unique_path_adds_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let rel = Path::new("same.txt");
        assert_eq!(unique_path(dir.path(), rel), dir.path().join("same.txt"));
        std::fs::write(dir.path().join("same.txt"), "x").unwrap();
        assert_eq!(
            unique_path(dir.path(), rel),
            dir.path().join("same (1).txt")
        );
        std::fs::write(dir.path().join("same (1).txt"), "x").unwrap();
        assert_eq!(
            unique_path(dir.path(), rel),
            dir.path().join("same (2).txt")
        );
    }

    proptest! {
        #[test]
        fn never_escapes(s in ".*") {
            check(&s)?;
        }

        #[test]
        fn never_escapes_pathy(s in "[./\\\\a-zA-Z: ]{0,40}") {
            check(&s)?;
        }
    }

    fn check(s: &str) -> Result<(), TestCaseError> {
        let out = sanitize_relative(s);
        prop_assert!(!out.as_os_str().is_empty());
        prop_assert!(
            out.components().all(|c| matches!(c, Component::Normal(_))),
            "{:?}",
            out
        );
        let base = Path::new("base");
        prop_assert!(base.join(&out).starts_with(base));
        Ok(())
    }
}
