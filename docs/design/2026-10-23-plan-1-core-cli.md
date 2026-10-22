# sendit Plan 1: Rust core + CLI

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust library (`sendit-core`) that speaks LocalSend protocol v2 with sendit's extra identity checks, plus a `sendit` CLI that can discover, send to and receive from official LocalSend apps.

**Architecture:** A Cargo workspace. `crates/core` owns everything protocol-related: an HTTPS server (axum + axum-server + rustls), outgoing transfers (reqwest with a fingerprint-pinning rustls verifier), multicast discovery (socket2), a TOFU trust store, and a broadcast event stream. `crates/cli` is a thin consumer of the `Node` API. Plan 2 (Tauri + Svelte app) will consume the same API.

**Tech Stack:** Rust stable (2021 edition), tokio, axum 0.8, axum-server 0.7, rustls 0.23 (ring provider), rcgen 0.13, reqwest 0.12 (rustls), socket2 0.5, serde, thiserror 2, proptest, cargo-fuzz, clap 4.

**Spec:** `docs/design/2026-10-23-sendit-v1-design.md`

## Global Constraints

- Protocol: LocalSend v2. Multicast `224.0.0.167:53317`, HTTP API under `/api/localsend/v2`, default TCP port `53317`.
- Fingerprint is the SHA-256 of the cert DER. Store it as uppercase hex and always compare it case-insensitively.
- rustls must use the **ring** provider. Never call `ClientConfig::builder()` or `ServerConfig::builder()`. Always use `builder_with_provider(tls::provider())`, because aws-lc-rs does not build on Windows without extra toolchains.
- Commits use the author's git identity only. **No `Co-Authored-By` or other tool-attribution trailers.**
- `.superpowers/` stays gitignored.
- License: Apache-2.0.
- UI-facing strings in the CLI are English.
- CI must pass on Windows, macOS and Linux. Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` before every commit.

## Review Focus

1. **The sender disappears right after `prepare-upload`.** The receiver must not stay "busy" (409) forever. The idle watchdog test lives in Task 8.
2. **Two files in one batch share a name, or the name already exists on disk.** Nothing gets overwritten: you get `same.txt`, `same (1).txt`, `same (2).txt`. Test in Task 8.
3. **LocalSend sends fingerprints in a different letter case.** Pinning still succeeds. Test in Task 4.
4. **Port 53317 is already in use (LocalSend is open on the same PC).** The node falls back to a free port and reports it. Test in Task 7.
5. **A newer LocalSend version adds JSON fields or new `deviceType` values.** Parsing still succeeds. Test in Task 1.

---

## File map

```
Cargo.toml                         workspace
crates/core/Cargo.toml
crates/core/src/lib.rs             module list + re-exports
crates/core/src/protocol.rs        DTOs + constants (Task 1)
crates/core/src/sanitize.rs        peer filename -> safe relative path (Task 2)
crates/core/src/identity.rs        cert + fingerprint (Task 3)
crates/core/src/tls.rs             pinned verifier, PeerClient, server config (Task 4)
crates/core/src/trust.rs           TOFU store (Task 5)
crates/core/src/error.rs           Error (Task 6)
crates/core/src/events.rs          Device, Event, FailReason, Meter, stalled() (Task 6)
crates/core/src/node.rs            Config, Node, Inner (Task 7, extended in 8-10)
crates/core/src/server.rs          axum router + handlers (Task 7, extended in 8)
crates/core/src/discovery.rs       register_with (Task 7), multicast + scan (Task 10)
crates/core/src/receive.rs         incoming sessions + write_file (Task 8)
crates/core/src/send.rs            outgoing transfers (Task 9)
crates/core/tests/common/mod.rs    test helpers (grown per task)
crates/core/tests/fixtures/*.json  protocol fixtures
crates/core/tests/{protocol_fixtures,peers,receive,transfer,chaos,multicast}.rs
crates/core/examples/loopback_bench.rs
crates/cli/Cargo.toml, crates/cli/src/main.rs, crates/cli/tests/cli.rs
fuzz/                              cargo-fuzz targets
.github/workflows/{ci,fuzz}.yml
docs/{interop-checklist,benchmark}.md, README.md
```

---

### Task 1: Workspace + protocol DTOs

**Files:**
- Create: `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/protocol.rs`
- Create: `crates/core/tests/fixtures/{announce,announce_future,register_response,prepare_upload,prepare_upload_response}.json`
- Test: `crates/core/tests/protocol_fixtures.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `protocol::{PROTOCOL_VERSION, DEFAULT_PORT, MULTICAST_ADDR, API, Scheme, DeviceInfo, Announcement, FileMeta, PrepareUploadRequest, PrepareUploadResponse, UploadQuery, SessionQuery, PinQuery}`

- [ ] **Step 1: Workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/core"]
exclude = ["fuzz"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
```

`crates/core/Cargo.toml`:
```toml
[package]
name = "sendit-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "sendit_core"

[dependencies]
axum = "0.8"
axum-server = { version = "0.7", features = ["tls-rustls-no-provider"] }
fs2 = "0.4"
futures = "0.3"
hex = "0.4"
if-addrs = "0.13"
mime_guess = "2"
rcgen = "0.13"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream"] }
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12", "logging"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
socket2 = { version = "0.5", features = ["all"] }
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["io"] }
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
walkdir = "2"

[dev-dependencies]
proptest = "1"
tempfile = "3"
tokio = { version = "1", features = ["full", "test-util"] }
```

`crates/core/src/lib.rs`:
```rust
pub mod protocol;
```

Append to `.gitignore`:
```
fuzz/target/
fuzz/corpus/
fuzz/artifacts/
```

- [ ] **Step 2: Fixtures** (shapes taken from https://github.com/localsend/protocol)

`crates/core/tests/fixtures/announce.json`:
```json
{"alias":"Nice Orange","version":"2.0","deviceModel":"Samsung","deviceType":"mobile","fingerprint":"random string","port":53317,"protocol":"https","download":true,"announce":true}
```
`announce_future.json`:
```json
{"alias":"Watchy","version":"2.9","deviceModel":null,"deviceType":"smartwatch","fingerprint":"abc","port":53317,"protocol":"https","download":false,"announce":true,"someNewField":{"x":1}}
```
`register_response.json`:
```json
{"alias":"Secret Banana","version":"2.0","deviceModel":"Windows","deviceType":"desktop","fingerprint":"random string","download":true}
```
`prepare_upload.json`:
```json
{
  "info": {"alias":"Nice Orange","version":"2.0","deviceModel":"Samsung","deviceType":"mobile","fingerprint":"random string","port":53317,"protocol":"https","download":true},
  "files": {
    "some file id": {"id":"some file id","fileName":"my image.png","size":324242,"fileType":"image/jpeg","sha256":"*sha256 hash*","preview":"*preview data*","metadata":{"modified":"2021-01-01T12:34:56Z"}},
    "another file id": {"id":"another file id","fileName":"another image.jpg","size":1234,"fileType":"image/jpeg","sha256":null,"preview":null}
  }
}
```
`prepare_upload_response.json`:
```json
{"sessionId":"mySessionId","files":{"someFileId":"someFileToken","someOtherFileId":"someOtherFileToken"}}
```

- [ ] **Step 3: Write the failing test** `crates/core/tests/protocol_fixtures.rs`
```rust
use sendit_core::protocol::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn parses_announcement() {
    let a: Announcement = serde_json::from_str(&fixture("announce.json")).unwrap();
    assert_eq!(a.info.alias, "Nice Orange");
    assert_eq!(a.info.port, 53317);
    assert_eq!(a.info.protocol, Scheme::Https);
    assert!(a.info.download);
    assert!(a.announce);
}

#[test]
fn tolerates_unknown_fields_and_device_types() {
    let a: Announcement = serde_json::from_str(&fixture("announce_future.json")).unwrap();
    assert_eq!(a.info.device_type.as_deref(), Some("smartwatch"));
    assert_eq!(a.info.device_model, None);
}

#[test]
fn register_response_defaults_port_and_protocol() {
    let i: DeviceInfo = serde_json::from_str(&fixture("register_response.json")).unwrap();
    assert_eq!(i.port, DEFAULT_PORT);
    assert_eq!(i.protocol, Scheme::Https);
    assert_eq!(i.sendit, None);
}

#[test]
fn prepare_upload_round_trips() {
    let r: PrepareUploadRequest = serde_json::from_str(&fixture("prepare_upload.json")).unwrap();
    assert_eq!(r.files.len(), 2);
    let f = &r.files["some file id"];
    assert_eq!(f.file_name, "my image.png");
    assert_eq!(f.size, 324242);
    assert_eq!(r.files["another file id"].sha256, None);
    let again: PrepareUploadRequest = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(r, again);
}

#[test]
fn prepare_upload_response_parses() {
    let r: PrepareUploadResponse = serde_json::from_str(&fixture("prepare_upload_response.json")).unwrap();
    assert_eq!(r.session_id, "mySessionId");
    assert_eq!(r.files["someFileId"], "someFileToken");
}

#[test]
fn serializes_camel_case_without_nulls() {
    let info = DeviceInfo {
        alias: "pc".into(),
        version: PROTOCOL_VERSION.into(),
        device_model: Some("windows".into()),
        device_type: None,
        fingerprint: "AB".into(),
        port: 1234,
        protocol: Scheme::Https,
        download: false,
        sendit: Some("0.1.0".into()),
    };
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["deviceModel"], "windows");
    assert_eq!(v["protocol"], "https");
    assert_eq!(v["sendit"], "0.1.0");
    assert!(v.get("deviceType").is_none());
    assert!(v.get("device_model").is_none());
}
```

- [ ] **Step 4: Run it to verify it fails**

Run: `cargo test -p sendit-core --test protocol_fixtures`
Expected: compile error, `protocol` items not found.

- [ ] **Step 5: Implement** `crates/core/src/protocol.rs`
```rust
//! LocalSend protocol v2 wire types. https://github.com/localsend/protocol
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

pub const PROTOCOL_VERSION: &str = "2.1";
pub const DEFAULT_PORT: u16 = 53317;
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const API: &str = "/api/localsend/v2";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Http,
    #[default]
    Https,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

/// Sent in announcements, /register bodies and responses, and prepare-upload `info`.
/// Fields LocalSend omits in some messages (port, protocol) get defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub alias: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    /// Kept as a string so unknown future types still parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    pub fingerprint: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub protocol: Scheme,
    #[serde(default)]
    pub download: bool,
    /// sendit extension: our app version. LocalSend ignores unknown fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sendit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Announcement {
    #[serde(flatten)]
    pub info: DeviceInfo,
    #[serde(default)]
    pub announce: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    #[serde(default)]
    pub file_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PrepareUploadRequest {
    pub info: DeviceInfo,
    pub files: HashMap<String, FileMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadResponse {
    pub session_id: String,
    /// fileId -> token
    pub files: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UploadQuery {
    pub session_id: String,
    pub file_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionQuery {
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PinQuery {
    pub pin: Option<String>,
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p sendit-core --test protocol_fixtures`
Expected: 6 passed.

- [ ] **Step 7: Commit**
```bash
git add Cargo.toml crates .gitignore
git commit -m "feat(core): LocalSend v2 protocol types with fixtures"
```

---

### Task 2: Filename sanitizer

**Files:**
- Create: `crates/core/src/sanitize.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod sanitize;`)

**Interfaces:**
- Produces: `sanitize::sanitize_relative(name: &str) -> PathBuf` (always a non-empty relative path made only of `Component::Normal`), `sanitize::unique_path(dest: &Path, rel: &Path) -> PathBuf`

- [ ] **Step 1: Write the failing tests** at the bottom of `crates/core/src/sanitize.rs` (create the file with only this module and stub signatures that `todo!()`)
```rust
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
        assert_eq!(sanitize_relative("album/sub/b.jpg"), p(&["album", "sub", "b.jpg"]));
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
        assert_eq!(sanitize_relative("C:\\Windows\\x"), p(&["C_", "Windows", "x"]));
        assert_eq!(sanitize_relative("CON.txt"), p(&["_CON.txt"]));
        assert_eq!(sanitize_relative("lpt1"), p(&["_lpt1"]));
        assert_eq!(sanitize_relative("file. . "), p(&["file"]));
        assert_eq!(sanitize_relative("a?b*c|d\"e<f>g"), p(&["a_b_c_d_e_f_g"]));
    }

    #[test]
    fn control_and_bidi_chars_replaced() {
        assert_eq!(sanitize_relative("a\u{0}b\nc"), p(&["a_b_c"]));
        assert_eq!(sanitize_relative("invoice\u{202E}fdp.exe"), p(&["invoice_fdp.exe"]));
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
        assert_eq!(unique_path(dir.path(), rel), dir.path().join("same (1).txt"));
        std::fs::write(dir.path().join("same (1).txt"), "x").unwrap();
        assert_eq!(unique_path(dir.path(), rel), dir.path().join("same (2).txt"));
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
        prop_assert!(out.components().all(|c| matches!(c, Component::Normal(_))), "{:?}", out);
        let base = Path::new("base");
        prop_assert!(base.join(&out).starts_with(base));
        Ok(())
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core sanitize`
Expected: tests panic with `not yet implemented`.

- [ ] **Step 3: Implement** (top of `crates/core/src/sanitize.rs`)
```rust
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
    let stem = first.file_stem().and_then(|s| s.to_str()).unwrap_or("file").to_string();
    let ext = first.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (1..)
        .map(|n| parent.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("unbounded range")
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sendit-core sanitize`
Expected: all pass, including both proptests.

- [ ] **Step 5: Commit**
```bash
git add crates/core/src
git commit -m "feat(core): sanitize peer-supplied file names"
```

---

### Task 3: Identity (cert + fingerprint)

**Files:**
- Create: `crates/core/src/identity.rs`, `crates/core/src/error.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `identity::{Identity { cert_der: Vec<u8>, key_der: Vec<u8>, fingerprint: String }, fingerprint_of(&[u8]) -> String}`, `Identity::generate() -> Result<Identity>`, `Identity::load_or_create(dir: &Path) -> Result<Identity>`
- Produces: `error::{Error, Result}` (the whole enum is defined here, so later tasks only use it)

- [ ] **Step 1: Create `crates/core/src/error.rs`**
```rust
use crate::events::FailReason;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls: {0}")]
    Tls(#[from] rustls::Error),
    #[error("certificate: {0}")]
    Cert(#[from] rcgen::Error),
    #[error("network: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unknown device {0}")]
    UnknownDevice(String),
    #[error("that address is this device")]
    SelfDevice,
    #[error("this device's key changed; confirm it or block it first")]
    Suspect,
    #[error("device is blocked")]
    Blocked,
    #[error("peer's certificate does not match its fingerprint")]
    FingerprintMismatch,
    #[error("no pending request {0}")]
    NoSuchRequest(String),
    #[error("not enough disk space")]
    DiskFull,
    #[error("transfer failed: {0:?}")]
    Transfer(FailReason),
}

pub type Result<T> = std::result::Result<T, Error>;
```
`FailReason` arrives in Task 6. Until then, put a temporary stub in `lib.rs` so this compiles:
```rust
pub mod error;
pub mod identity;
pub mod protocol;
pub mod sanitize;

// Replaced by the real events module in Task 6.
pub mod events {
    #[derive(Clone, Debug, PartialEq)]
    pub enum FailReason {}
}

pub use error::{Error, Result};
```

- [ ] **Step 2: Write failing tests** at the bottom of `crates/core/src/identity.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_uppercase_sha256_hex() {
        let id = Identity::generate().unwrap();
        assert_eq!(id.fingerprint.len(), 64);
        assert!(id.fingerprint.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)));
        assert_eq!(id.fingerprint, fingerprint_of(&id.cert_der));
    }

    #[test]
    fn each_generate_is_unique() {
        assert_ne!(Identity::generate().unwrap().fingerprint, Identity::generate().unwrap().fingerprint);
    }

    #[test]
    fn load_or_create_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(dir.path()).unwrap();
        let b = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.key_der, b.key_der);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p sendit-core identity`
Expected: compile error, `Identity` not found.

- [ ] **Step 4: Implement** (top of `crates/core/src/identity.rs`)
```rust
//! Our TLS identity. LocalSend defines a device's fingerprint as SHA-256 of its cert.
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub struct Identity {
    pub cert_der: Vec<u8>,
    /// PKCS#8
    pub key_der: Vec<u8>,
    /// Uppercase hex SHA-256 of `cert_der`.
    pub fingerprint: String,
}

pub fn fingerprint_of(cert_der: &[u8]) -> String {
    hex::encode_upper(Sha256::digest(cert_der))
}

impl Identity {
    pub fn generate() -> Result<Self> {
        let ck = rcgen::generate_simple_self_signed(vec!["sendit".to_string()])?;
        let cert_der = ck.cert.der().to_vec();
        Ok(Self {
            fingerprint: fingerprint_of(&cert_der),
            key_der: ck.key_pair.serialize_der(),
            cert_der,
        })
    }

    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let cert_path = dir.join("cert.der");
        let key_path = dir.join("key.der");
        if cert_path.exists() && key_path.exists() {
            let cert_der = fs::read(cert_path)?;
            return Ok(Self { fingerprint: fingerprint_of(&cert_der), key_der: fs::read(key_path)?, cert_der });
        }
        fs::create_dir_all(dir)?;
        let id = Self::generate()?;
        fs::write(&cert_path, &id.cert_der)?;
        fs::write(&key_path, &id.key_der)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(id)
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sendit-core identity`
Expected: 3 passed.

- [ ] **Step 6: Commit**
```bash
git add crates/core/src
git commit -m "feat(core): persistent self-signed identity and fingerprint"
```

---

### Task 4: Pinned TLS client + server config

**Files:**
- Create: `crates/core/src/tls.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod tls;`)

**Interfaces:**
- Consumes: `identity::{Identity, fingerprint_of}`, `error::Result`
- Produces: `tls::provider() -> Arc<CryptoProvider>`, `tls::server_config(&Identity) -> Result<Arc<rustls::ServerConfig>>`, `tls::PeerClient { pub http: reqwest::Client }`, `PeerClient::new(expected: Option<&str>) -> Result<PeerClient>`, `PeerClient::seen_fingerprint(&self) -> Option<String>`

- [ ] **Step 1: Write failing tests** at the bottom of `crates/core/src/tls.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use std::net::SocketAddr;

    async fn serve(id: &Identity) -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
        let tls = axum_server::tls_rustls::RustlsConfig::from_config(server_config(id).unwrap());
        tokio::spawn(axum_server::from_tcp_rustls(listener, tls).serve(app.into_make_service()));
        addr
    }

    #[tokio::test]
    async fn accepts_matching_fingerprint_in_any_case() {
        let id = Identity::generate().unwrap();
        let addr = serve(&id).await;
        for fp in [id.fingerprint.clone(), id.fingerprint.to_lowercase()] {
            let c = PeerClient::new(Some(&fp)).unwrap();
            let body = c.http.get(format!("https://{addr}/")).send().await.unwrap().text().await.unwrap();
            assert_eq!(body, "ok");
        }
    }

    #[tokio::test]
    async fn rejects_wrong_fingerprint_and_records_what_it_saw() {
        let id = Identity::generate().unwrap();
        let other = Identity::generate().unwrap();
        let addr = serve(&id).await;
        let c = PeerClient::new(Some(&other.fingerprint)).unwrap();
        assert!(c.http.get(format!("https://{addr}/")).send().await.is_err());
        assert_eq!(c.seen_fingerprint().as_deref(), Some(id.fingerprint.as_str()));
    }

    #[tokio::test]
    async fn unpinned_client_accepts_and_records() {
        let id = Identity::generate().unwrap();
        let addr = serve(&id).await;
        let c = PeerClient::new(None).unwrap();
        c.http.get(format!("https://{addr}/")).send().await.unwrap();
        assert_eq!(c.seen_fingerprint().as_deref(), Some(id.fingerprint.as_str()));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core tls`
Expected: compile error.

- [ ] **Step 3: Implement** (top of `crates/core/src/tls.rs`)
```rust
//! TLS with self-signed certs. Instead of trusting a CA, we pin a peer's
//! certificate to the fingerprint it announced. This is sendit's main
//! defence against MITM that LocalSend itself does not do.
use crate::{error::Result, identity::{fingerprint_of, Identity}};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{ring, CryptoProvider},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

pub fn server_config(id: &Identity) -> Result<Arc<rustls::ServerConfig>> {
    let cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(id.cert_der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key_der.clone())),
        )?;
    Ok(Arc::new(cfg))
}

#[derive(Debug)]
struct PinnedVerifier {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint_of(end_entity.as_ref());
        *self.seen.lock().unwrap() = Some(fp.clone());
        match &self.expected {
            Some(want) if !want.eq_ignore_ascii_case(&fp) => {
                Err(rustls::Error::General("certificate fingerprint mismatch".into()))
            }
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// An HTTPS client for exactly one peer. With `expected = None` it accepts
/// any cert, so the caller must compare `seen_fingerprint()` to what the peer claims.
pub struct PeerClient {
    pub http: reqwest::Client,
    seen: Arc<Mutex<Option<String>>>,
}

impl PeerClient {
    pub fn new(expected: Option<&str>) -> Result<Self> {
        let seen = Arc::new(Mutex::new(None));
        let verifier = PinnedVerifier { expected: expected.map(str::to_string), seen: seen.clone(), provider: provider() };
        let tls = rustls::ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let http = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .build()?;
        Ok(Self { http, seen })
    }

    pub fn seen_fingerprint(&self) -> Option<String> {
        self.seen.lock().unwrap().clone()
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sendit-core tls`
Expected: 3 passed.

- [ ] **Step 5: Commit**
```bash
git add crates/core/src
git commit -m "feat(core): fingerprint-pinned TLS client and server config"
```

---

### Task 5: TOFU trust store

**Files:**
- Create: `crates/core/src/trust.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod trust;`)

**Interfaces:**
- Produces: `trust::{TrustLevel { New, Known, Verified, Suspect, Blocked }, Observation { Seen, New, IdentityChanged { old_fingerprint } }, TrustStore}`. Methods: `TrustStore::load(PathBuf)`, `in_memory()`, `observe(&mut self, fp, alias) -> Observation`, `level(&self, fp) -> TrustLevel`, `auto_accept(&self, fp) -> bool`, `accept_new_key(&mut self, fp, alias)`, `set_verified(&mut self, fp, bool)`, `set_auto_accept(&mut self, fp, bool)`, `block(&mut self, fp, alias)`. Every `fp` argument must already be uppercase. `Node` takes care of that.

- [ ] **Step 1: Write failing tests** at the bottom of `crates/core/src/trust.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sight_pins_then_known() {
        let mut t = TrustStore::in_memory();
        assert_eq!(t.level("AA"), TrustLevel::New);
        assert_eq!(t.observe("AA", "bob"), Observation::New);
        assert_eq!(t.observe("AA", "bob"), Observation::Seen);
        assert_eq!(t.level("AA"), TrustLevel::Known);
    }

    #[test]
    fn same_alias_new_key_is_suspect_until_accepted() {
        let mut t = TrustStore::in_memory();
        t.observe("AA", "bob");
        assert_eq!(t.observe("BB", "bob"), Observation::IdentityChanged { old_fingerprint: "AA".into() });
        assert_eq!(t.level("BB"), TrustLevel::Suspect);
        // no repeated event for the same suspect
        assert_eq!(t.observe("BB", "bob"), Observation::Seen);
        t.accept_new_key("BB", "bob");
        assert_eq!(t.level("BB"), TrustLevel::Known);
        assert_eq!(t.level("AA"), TrustLevel::Known);
    }

    #[test]
    fn verified_blocked_auto_accept() {
        let mut t = TrustStore::in_memory();
        t.observe("AA", "bob");
        t.set_verified("AA", true);
        assert_eq!(t.level("AA"), TrustLevel::Verified);
        assert!(!t.auto_accept("AA"));
        t.set_auto_accept("AA", true);
        assert!(t.auto_accept("AA"));
        t.block("CC", "mallory");
        assert_eq!(t.level("CC"), TrustLevel::Blocked);
        assert_eq!(t.observe("CC", "mallory"), Observation::Seen);
    }

    #[test]
    fn persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trust.json");
        {
            let mut t = TrustStore::load(path.clone());
            t.observe("AA", "bob");
            t.set_verified("AA", true);
        }
        let t = TrustStore::load(path);
        assert_eq!(t.level("AA"), TrustLevel::Verified);
    }

    #[test]
    fn corrupt_file_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trust.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(TrustStore::load(path).level("AA"), TrustLevel::New);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core trust`
Expected: compile error.

- [ ] **Step 3: Implement** (top of `crates/core/src/trust.rs`)
```rust
//! Trust-on-first-use. A device's key is pinned the first time we see it. If its
//! alias then shows up with a different key, that key is "suspect" until the user decides.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    New,
    Known,
    Verified,
    Suspect,
    Blocked,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Observation {
    Seen,
    New,
    IdentityChanged { old_fingerprint: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Record {
    alias: String,
    verified: bool,
    blocked: bool,
    auto_accept: bool,
}

pub struct TrustStore {
    path: Option<PathBuf>,
    records: HashMap<String, Record>,
    suspects: HashSet<String>,
}

impl TrustStore {
    pub fn load(path: PathBuf) -> Self {
        let records = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path: Some(path), records, suspects: HashSet::new() }
    }

    pub fn in_memory() -> Self {
        Self { path: None, records: HashMap::new(), suspects: HashSet::new() }
    }

    // ponytail: alias is the identity key for TOFU; two real devices with the same
    // name will trip it once each until the user accepts both keys.
    pub fn observe(&mut self, fp: &str, alias: &str) -> Observation {
        if let Some(r) = self.records.get_mut(fp) {
            if r.alias != alias {
                r.alias = alias.to_string();
                self.save();
            }
            return Observation::Seen;
        }
        if self.suspects.contains(fp) {
            return Observation::Seen;
        }
        if let Some(old) = self.records.iter().find(|(_, r)| r.alias == alias && !r.blocked).map(|(k, _)| k.clone()) {
            self.suspects.insert(fp.to_string());
            return Observation::IdentityChanged { old_fingerprint: old };
        }
        self.records.insert(fp.to_string(), Record { alias: alias.to_string(), ..Default::default() });
        self.save();
        Observation::New
    }

    pub fn level(&self, fp: &str) -> TrustLevel {
        match self.records.get(fp) {
            Some(r) if r.blocked => TrustLevel::Blocked,
            _ if self.suspects.contains(fp) => TrustLevel::Suspect,
            Some(r) if r.verified => TrustLevel::Verified,
            Some(_) => TrustLevel::Known,
            None => TrustLevel::New,
        }
    }

    pub fn auto_accept(&self, fp: &str) -> bool {
        self.records.get(fp).is_some_and(|r| r.auto_accept && !r.blocked)
    }

    /// "It's a reinstall": trust this key. The old key stays pinned too.
    pub fn accept_new_key(&mut self, fp: &str, alias: &str) {
        self.suspects.remove(fp);
        self.records.entry(fp.to_string()).or_insert_with(|| Record { alias: alias.to_string(), ..Default::default() });
        self.save();
    }

    pub fn set_verified(&mut self, fp: &str, verified: bool) {
        if let Some(r) = self.records.get_mut(fp) {
            r.verified = verified;
            self.save();
        }
    }

    pub fn set_auto_accept(&mut self, fp: &str, on: bool) {
        if let Some(r) = self.records.get_mut(fp) {
            r.auto_accept = on;
            self.save();
        }
    }

    pub fn block(&mut self, fp: &str, alias: &str) {
        self.suspects.remove(fp);
        let r = self.records.entry(fp.to_string()).or_insert_with(|| Record { alias: alias.to_string(), ..Default::default() });
        r.blocked = true;
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        match serde_json::to_vec_pretty(&self.records) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(path, bytes) {
                    tracing::warn!("saving trust store: {e}");
                }
            }
            Err(e) => tracing::warn!("encoding trust store: {e}"),
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sendit-core trust`
Expected: 5 passed.

- [ ] **Step 5: Commit**
```bash
git add crates/core/src
git commit -m "feat(core): TOFU trust store"
```

---

### Task 6: Events, FailReason, progress Meter

**Files:**
- Create: `crates/core/src/events.rs`
- Modify: `crates/core/src/lib.rs` (remove the stub `events` module, add `pub mod events;` and re-exports)

**Interfaces:**
- Consumes: `protocol::{FileMeta, API}`, `trust::TrustLevel`
- Produces: `Device { fingerprint, alias, device_model, device_type, addr: SocketAddr, trust: TrustLevel, is_sendit: bool }` with `base_url()`, `Direction`, `FailReason` (+ `From<io::Error>`), `IncomingRequest`, `Event` (`DeviceUpdated{device}`, `IdentityChanged{alias, old_fingerprint, device}`, `IncomingRequest{request}`, `TransferStarted{transfer_id, direction, peer, files, total}`, `Progress{transfer_id, direction, bytes, total}`, `TransferDone{transfer_id, direction, paths}`, `TransferFailed{transfer_id, direction, reason}`), `pub(crate) Meter::new(tx, id, dir, total) -> Arc<Meter>`, `Meter::add(u64)`, `Meter::done() -> u64`, `pub(crate) async fn stalled(&Meter, Duration)`

- [ ] **Step 1: Write failing tests** at the bottom of `crates/core/src/events.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_throttles_but_always_reports_completion() {
        let (tx, mut rx) = broadcast::channel(4096);
        let m = Meter::new(tx, "t".into(), Direction::Outgoing, 1000);
        for _ in 0..1000 {
            m.add(1);
        }
        let mut events = vec![];
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        assert!(events.len() < 5, "{} events", events.len());
        let Some(Event::Progress { bytes, total, .. }) = events.last() else { panic!("no progress") };
        assert_eq!((*bytes, *total), (1000, 1000));
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_waits_for_a_quiet_period() {
        let (tx, _rx) = broadcast::channel(16);
        let m = Meter::new(tx, "t".into(), Direction::Incoming, 100);
        let feeder = {
            let m = m.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                for _ in 0..3 {
                    m.add(1);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
        };
        let start = tokio::time::Instant::now();
        stalled(&m, Duration::from_secs(1)).await;
        let waited = start.elapsed();
        assert!(waited >= Duration::from_secs(4) && waited < Duration::from_secs(5), "{waited:?}");
        feeder.await.unwrap();
    }

    #[test]
    fn io_storage_full_maps_to_disk_full() {
        let e = std::io::Error::from(std::io::ErrorKind::StorageFull);
        assert_eq!(FailReason::from(e), FailReason::DiskFull);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core events`
Expected: compile error.

- [ ] **Step 3: Implement** (top of `crates/core/src/events.rs`)
```rust
//! Everything the UI/CLI learns about comes through `Event`.
use crate::{protocol::{FileMeta, API}, trust::TrustLevel};
use serde::Serialize;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Uppercase hex. The stable id of a device.
    pub fingerprint: String,
    pub alias: String,
    pub device_model: Option<String>,
    pub device_type: Option<String>,
    pub addr: SocketAddr,
    pub trust: TrustLevel,
    pub is_sendit: bool,
}

impl Device {
    pub fn base_url(&self) -> String {
        format!("https://{}{}", self.addr, API)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum FailReason {
    Declined,
    Cancelled,
    PeerGone,
    Busy,
    PinRequired,
    Blocked,
    Checksum,
    DiskFull,
    IdentityMismatch,
    Protocol(String),
    Io(String),
}

impl From<std::io::Error> for FailReason {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::StorageFull {
            Self::DiskFull
        } else {
            Self::Io(e.to_string())
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequest {
    pub request_id: String,
    pub from: Device,
    pub files: Vec<FileMeta>,
    pub total_size: u64,
    /// Whether the default download dir has room. The UI warns if not.
    pub fits_on_disk: bool,
    /// The sender proved it owns the key it claims (we called back, pinned).
    pub sender_verified: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    DeviceUpdated { device: Device },
    IdentityChanged { alias: String, old_fingerprint: String, device: Device },
    IncomingRequest { request: IncomingRequest },
    TransferStarted { transfer_id: String, direction: Direction, peer: String, files: Vec<FileMeta>, total: u64 },
    Progress { transfer_id: String, direction: Direction, bytes: u64, total: u64 },
    TransferDone { transfer_id: String, direction: Direction, paths: Vec<PathBuf> },
    TransferFailed { transfer_id: String, direction: Direction, reason: FailReason },
}

const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// Counts bytes of one transfer and emits throttled `Progress` events.
pub(crate) struct Meter {
    tx: broadcast::Sender<Event>,
    id: String,
    dir: Direction,
    total: u64,
    done: AtomicU64,
    last: Mutex<Instant>,
}

impl Meter {
    pub(crate) fn new(tx: broadcast::Sender<Event>, id: String, dir: Direction, total: u64) -> Arc<Self> {
        Arc::new(Self { tx, id, dir, total, done: AtomicU64::new(0), last: Mutex::new(Instant::now()) })
    }

    pub(crate) fn add(&self, n: u64) {
        let done = self.done.fetch_add(n, Ordering::Relaxed) + n;
        let mut last = self.last.lock().unwrap();
        if done >= self.total || last.elapsed() >= PROGRESS_EVERY {
            *last = Instant::now();
            let _ = self.tx.send(Event::Progress { transfer_id: self.id.clone(), direction: self.dir, bytes: done, total: self.total });
        }
    }

    pub(crate) fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }
}

/// Resolves once `meter` makes no progress for a whole `idle` period.
pub(crate) async fn stalled(meter: &Meter, idle: Duration) {
    let mut last = meter.done();
    loop {
        tokio::time::sleep(idle).await;
        let now = meter.done();
        if now == last {
            return;
        }
        last = now;
    }
}
```

`crates/core/src/lib.rs` becomes:
```rust
pub mod error;
pub mod events;
pub mod identity;
pub mod protocol;
pub mod sanitize;
pub mod tls;
pub mod trust;

pub use error::{Error, Result};
pub use events::{Device, Direction, Event, FailReason, IncomingRequest};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sendit-core`
Expected: all tests so far pass.

- [ ] **Step 5: Commit**
```bash
git add crates/core/src
git commit -m "feat(core): event stream, fail reasons, progress meter"
```

---

### Task 7: Node + HTTPS server + /register + add_peer

**Files:**
- Create: `crates/core/src/node.rs`, `crates/core/src/server.rs`, `crates/core/src/discovery.rs`
- Create: `crates/core/tests/common/mod.rs`, `crates/core/tests/peers.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: everything from Tasks 1-6
- Produces: `Config { alias, port, multicast_port, data_dir, download_dir, pin, multicast, request_timeout, idle_timeout }` with `Config::new(alias, data_dir, download_dir)`. `Node` with `start(Config) -> Result<Node>`, `port()`, `port_fallback()`, `fingerprint()`, `info()`, `subscribe()`, `devices()`, `device(fp)`, `add_peer(SocketAddr) -> Result<Device>`, `accept_new_key(fp)`, `set_verified(fp, bool)`, `set_auto_accept(fp, bool)`, `block(fp)`, `shutdown()`. Crate-internal: `Inner { cfg, identity, port, trust, devices, events }`, `Inner::emit`, `Inner::my_info`, `Inner::see_device(&DeviceInfo, IpAddr) -> Option<Device>`, `discovery::register_with(&Inner, SocketAddr, Option<&str>) -> Result<Device>`
- Test helpers: `common::{Peer { node, dir }, peer(alias), peer_with(alias, f), Peer::addr(), Peer::down(), wait_for(rx, pred)}`

- [ ] **Step 1: Test helpers** `crates/core/tests/common/mod.rs`
```rust
#![allow(dead_code)]
use sendit_core::{Config, Event, Node};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tempfile::TempDir;
use tokio::sync::broadcast;

pub struct Peer {
    pub node: Node,
    pub dir: TempDir,
}

impl Peer {
    pub fn addr(&self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.node.port()))
    }
    pub fn down(&self) -> PathBuf {
        self.dir.path().join("down")
    }
}

pub async fn peer(alias: &str) -> Peer {
    peer_with(alias, |_| {}).await
}

pub async fn peer_with(alias: &str, f: impl FnOnce(&mut Config)) -> Peer {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new(alias, dir.path().join("data"), dir.path().join("down"));
    cfg.port = 0;
    cfg.multicast = false;
    cfg.request_timeout = Duration::from_secs(5);
    cfg.idle_timeout = Duration::from_secs(1);
    f(&mut cfg);
    Peer { node: Node::start(cfg).await.unwrap(), dir }
}

pub async fn wait_for(rx: &mut broadcast::Receiver<Event>, mut pred: impl FnMut(&Event) -> bool) -> Event {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match rx.recv().await {
                Ok(e) if pred(&e) => return e,
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(e) => panic!("event stream closed: {e}"),
            }
        }
    })
    .await
    .expect("timed out waiting for event")
}
```

- [ ] **Step 2: Write failing tests** `crates/core/tests/peers.rs`
```rust
mod common;
use common::*;
use sendit_core::{trust::TrustLevel, Config, Event, Node};

#[tokio::test(flavor = "multi_thread")]
async fn peers_register_each_other() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    let dev = a.node.add_peer(b.addr()).await.unwrap();
    assert_eq!(dev.alias, "bob");
    assert_eq!(dev.fingerprint, b.node.fingerprint());
    assert_eq!(dev.trust, TrustLevel::Known);
    assert!(dev.is_sendit);
    assert!(b.node.devices().iter().any(|d| d.fingerprint == a.node.fingerprint()));
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_self_is_rejected() {
    let a = peer("alice").await;
    assert!(a.node.add_peer(a.addr()).await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn same_alias_with_new_key_is_suspect() {
    let a = peer("alice").await;
    let bob1 = peer("bob").await;
    let bob2 = peer("bob").await;
    a.node.add_peer(bob1.addr()).await.unwrap();
    let mut rx = a.node.subscribe();
    let d = a.node.add_peer(bob2.addr()).await.unwrap();
    assert_eq!(d.trust, TrustLevel::Suspect);
    let Event::IdentityChanged { old_fingerprint, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::IdentityChanged { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(old_fingerprint, bob1.node.fingerprint());
    a.node.accept_new_key(bob2.node.fingerprint());
    assert_eq!(a.node.device(bob2.node.fingerprint()).unwrap().trust, TrustLevel::Known);
}

#[tokio::test(flavor = "multi_thread")]
async fn busy_port_falls_back() {
    let blocker = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let taken = blocker.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new("x", dir.path().join("d"), dir.path().join("down"));
    cfg.port = taken;
    cfg.multicast = false;
    let node = Node::start(cfg).await.unwrap();
    assert!(node.port_fallback());
    assert_ne!(node.port(), taken);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p sendit-core --test peers`
Expected: compile error, `Config`/`Node` missing.

- [ ] **Step 4: Implement `crates/core/src/node.rs`**
```rust
use crate::{
    discovery,
    error::Result,
    events::{Device, Event},
    identity::Identity,
    protocol::*,
    server, tls,
    trust::{Observation, TrustStore},
};
use axum_server::tls_rustls::RustlsConfig;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct Config {
    pub alias: String,
    /// TCP port for the HTTPS server. 0 = any free port.
    pub port: u16,
    pub multicast_port: u16,
    pub data_dir: PathBuf,
    pub download_dir: PathBuf,
    pub pin: Option<String>,
    pub multicast: bool,
    /// How long an incoming request waits for the user.
    pub request_timeout: Duration,
    /// A transfer with no bytes moving for this long fails.
    pub idle_timeout: Duration,
}

impl Config {
    pub fn new(alias: impl Into<String>, data_dir: impl Into<PathBuf>, download_dir: impl Into<PathBuf>) -> Self {
        Self {
            alias: alias.into(),
            port: DEFAULT_PORT,
            multicast_port: DEFAULT_PORT,
            data_dir: data_dir.into(),
            download_dir: download_dir.into(),
            pin: None,
            multicast: true,
            request_timeout: Duration::from_secs(120),
            idle_timeout: Duration::from_secs(30),
        }
    }
}

pub(crate) struct Inner {
    pub cfg: Config,
    pub identity: Identity,
    pub port: u16,
    pub trust: Mutex<TrustStore>,
    /// Keyed by uppercase fingerprint.
    pub devices: Mutex<HashMap<String, Device>>,
    pub events: broadcast::Sender<Event>,
}

impl Inner {
    pub(crate) fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    pub(crate) fn my_info(&self) -> DeviceInfo {
        DeviceInfo {
            alias: self.cfg.alias.clone(),
            version: PROTOCOL_VERSION.into(),
            device_model: Some(std::env::consts::OS.into()),
            device_type: Some("desktop".into()),
            fingerprint: self.identity.fingerprint.clone(),
            port: self.port,
            protocol: Scheme::Https,
            download: false,
            sendit: Some(env!("CARGO_PKG_VERSION").into()),
        }
    }

    /// Records a peer we heard from. Returns None if `info` is ourselves.
    pub(crate) fn see_device(&self, info: &DeviceInfo, ip: IpAddr) -> Option<Device> {
        if info.fingerprint.eq_ignore_ascii_case(&self.identity.fingerprint) {
            return None;
        }
        let fp = info.fingerprint.to_uppercase();
        let (obs, trust) = {
            let mut t = self.trust.lock().unwrap();
            let obs = t.observe(&fp, &info.alias);
            (obs, t.level(&fp))
        };
        let device = Device {
            fingerprint: fp.clone(),
            alias: info.alias.clone(),
            device_model: info.device_model.clone(),
            device_type: info.device_type.clone(),
            addr: SocketAddr::new(ip, info.port),
            trust,
            is_sendit: info.sendit.is_some(),
        };
        self.devices.lock().unwrap().insert(fp, device.clone());
        match obs {
            Observation::IdentityChanged { old_fingerprint } => self.emit(Event::IdentityChanged {
                alias: info.alias.clone(),
                old_fingerprint,
                device: device.clone(),
            }),
            _ => self.emit(Event::DeviceUpdated { device: device.clone() }),
        }
        Some(device)
    }
}

#[derive(Clone)]
pub struct Node {
    pub(crate) inner: Arc<Inner>,
    handle: axum_server::Handle,
    port_fallback: bool,
}

impl Node {
    pub async fn start(cfg: Config) -> Result<Node> {
        std::fs::create_dir_all(&cfg.data_dir)?;
        std::fs::create_dir_all(&cfg.download_dir)?;
        let identity = Identity::load_or_create(&cfg.data_dir.join("identity"))?;
        let trust = TrustStore::load(cfg.data_dir.join("trust.json"));
        let (listener, port_fallback) = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, cfg.port)) {
            Ok(l) => (l, false),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && cfg.port != 0 => {
                (TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))?, true)
            }
            Err(e) => return Err(e.into()),
        };
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (events, _) = broadcast::channel(1024);
        let inner = Arc::new(Inner {
            cfg,
            identity,
            port,
            trust: Mutex::new(trust),
            devices: Mutex::new(HashMap::new()),
            events,
        });
        let tls = RustlsConfig::from_config(tls::server_config(&inner.identity)?);
        let app = server::router(inner.clone()).into_make_service_with_connect_info::<SocketAddr>();
        let handle = axum_server::Handle::new();
        let server = axum_server::from_tcp_rustls(listener, tls).handle(handle.clone()).serve(app);
        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("https server stopped: {e}");
            }
        });
        Ok(Node { inner, handle, port_fallback })
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    /// True if the configured port was busy and a random one is used instead.
    pub fn port_fallback(&self) -> bool {
        self.port_fallback
    }

    pub fn fingerprint(&self) -> &str {
        &self.inner.identity.fingerprint
    }

    pub fn info(&self) -> DeviceInfo {
        self.inner.my_info()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    pub fn devices(&self) -> Vec<Device> {
        let mut v: Vec<Device> = self.inner.devices.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.alias.to_lowercase().cmp(&b.alias.to_lowercase()));
        v
    }

    pub fn device(&self, fingerprint: &str) -> Option<Device> {
        self.inner.devices.lock().unwrap().get(&fingerprint.to_uppercase()).cloned()
    }

    /// Contact a peer directly by address (no multicast needed).
    pub async fn add_peer(&self, addr: SocketAddr) -> Result<Device> {
        discovery::register_with(&self.inner, addr, None).await
    }

    pub fn accept_new_key(&self, fingerprint: &str) {
        let fp = fingerprint.to_uppercase();
        let alias = self.device(&fp).map(|d| d.alias).unwrap_or_default();
        self.inner.trust.lock().unwrap().accept_new_key(&fp, &alias);
        self.refresh(&fp);
    }

    pub fn set_verified(&self, fingerprint: &str, verified: bool) {
        let fp = fingerprint.to_uppercase();
        self.inner.trust.lock().unwrap().set_verified(&fp, verified);
        self.refresh(&fp);
    }

    pub fn set_auto_accept(&self, fingerprint: &str, on: bool) {
        let fp = fingerprint.to_uppercase();
        self.inner.trust.lock().unwrap().set_auto_accept(&fp, on);
        self.refresh(&fp);
    }

    pub fn block(&self, fingerprint: &str) {
        let fp = fingerprint.to_uppercase();
        let alias = self.device(&fp).map(|d| d.alias).unwrap_or_default();
        self.inner.trust.lock().unwrap().block(&fp, &alias);
        self.refresh(&fp);
    }

    pub fn shutdown(&self) {
        self.handle.shutdown();
    }

    fn refresh(&self, fp: &str) {
        let level = self.inner.trust.lock().unwrap().level(fp);
        let updated = self.inner.devices.lock().unwrap().get_mut(fp).map(|d| {
            d.trust = level;
            d.clone()
        });
        if let Some(device) = updated {
            self.inner.emit(Event::DeviceUpdated { device });
        }
    }
}
```

- [ ] **Step 5: Implement `crates/core/src/server.rs`**
```rust
use crate::{node::Inner, protocol::*};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    routing::post,
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};

pub(crate) fn router(n: Arc<Inner>) -> Router {
    Router::new()
        .route(&format!("{API}/register"), post(register))
        // JSON bodies only; /upload streams its raw body and is not affected.
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(n)
}

async fn register(
    State(n): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Json(info): Json<DeviceInfo>,
) -> Json<DeviceInfo> {
    if info.protocol == Scheme::Https {
        n.see_device(&info, remote.ip());
    }
    Json(n.my_info())
}
```

- [ ] **Step 6: Implement `crates/core/src/discovery.rs`**
```rust
use crate::{
    error::{Error, Result},
    events::Device,
    node::Inner,
    protocol::*,
    tls::PeerClient,
};
use std::{net::SocketAddr, time::Duration};

/// POST our info to `addr`/register. If `expected` is set, the TLS cert must
/// match it. Either way, the cert must match the fingerprint in the reply.
pub(crate) async fn register_with(n: &Inner, addr: SocketAddr, expected: Option<&str>) -> Result<Device> {
    let client = PeerClient::new(expected)?;
    let resp = client
        .http
        .post(format!("https://{addr}{API}/register"))
        .json(&n.my_info())
        .timeout(Duration::from_secs(3))
        .send()
        .await?
        .error_for_status()?;
    let mut info: DeviceInfo = resp.json().await?;
    let seen = client.seen_fingerprint().ok_or(Error::FingerprintMismatch)?;
    if !seen.eq_ignore_ascii_case(&info.fingerprint) {
        return Err(Error::FingerprintMismatch);
    }
    // /register replies omit port and protocol.
    info.port = addr.port();
    info.protocol = Scheme::Https;
    n.see_device(&info, addr.ip()).ok_or(Error::SelfDevice)
}
```

- [ ] **Step 7: Update `crates/core/src/lib.rs`**
```rust
pub mod error;
pub mod events;
pub mod identity;
pub mod protocol;
pub mod sanitize;
pub mod tls;
pub mod trust;

mod discovery;
mod node;
mod server;

pub use error::{Error, Result};
pub use events::{Device, Direction, Event, FailReason, IncomingRequest};
pub use node::{Config, Node};
```

- [ ] **Step 8: Run tests**

Run: `cargo test -p sendit-core --test peers`
Expected: 4 passed.

- [ ] **Step 9: Commit**
```bash
git add crates/core
git commit -m "feat(core): node with HTTPS server, register handshake and TOFU"
```

---

### Task 8: Receiving (prepare-upload, upload, cancel)

**Files:**
- Create: `crates/core/src/receive.rs`, `crates/core/tests/receive.rs`
- Modify: `crates/core/src/node.rs` (fields `pending`, `session`; `Decision`; `respond`), `crates/core/src/server.rs` (3 routes), `crates/core/src/lib.rs`, `crates/core/tests/common/mod.rs` (helpers)

**Interfaces:**
- Consumes: `Inner::see_device`, `discovery::register_with`, `Meter`, `stalled`, `sanitize::{sanitize_relative, unique_path}`
- Produces: `Decision { Accept { dest: Option<PathBuf> }, Decline }` (re-exported). `Node::respond(&self, request_id: &str, Decision) -> Result<()>`. Crate-internal: `Inner.pending: Mutex<HashMap<String, Pending>>`, `Inner.session: Mutex<Option<Session>>`, `Inner::open_session`, `claim_file`, `file_finished`, `fail_incoming(&self, session_id, FailReason) -> bool`, `receive::write_file`, `receive::has_space`, `receive::PART_SUFFIX = ".sendit-part"`, `FailReason::status()`

- [ ] **Step 1: Add test helpers** (append to `crates/core/tests/common/mod.rs`)
```rust
use sendit_core::{protocol::*, tls::PeerClient, Decision};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn auto_accept(node: &Node) {
    respond_all(node, || Decision::Accept { dest: None });
}

pub fn auto_decline(node: &Node) {
    respond_all(node, || Decision::Decline);
}

fn respond_all(node: &Node, d: impl Fn() -> Decision + Send + 'static) {
    let n = node.clone();
    let mut rx = node.subscribe();
    tokio::spawn(async move {
        while let Ok(e) = rx.recv().await {
            if let Event::IncomingRequest { request } = e {
                let _ = n.respond(&request.request_id, d());
            }
        }
    });
}

pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i.wrapping_mul(31) % 251) as u8).collect()
}

pub fn write_file(dir: &Path, rel: &str, len: usize) -> PathBuf {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, pattern(len)).unwrap();
    p
}

pub fn sha(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

/// True if no partial file is left anywhere under `dir`.
pub fn no_part_files(dir: &Path) -> bool {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .all(|e| !e.file_name().to_string_lossy().ends_with(".sendit-part"))
}

pub async fn eventually(mut f: impl FnMut() -> bool) {
    for _ in 0..50 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("condition never became true");
}

pub fn fake_info() -> DeviceInfo {
    DeviceInfo {
        alias: "mallory".into(),
        version: "2.1".into(),
        device_model: None,
        device_type: Some("desktop".into()),
        fingerprint: "F".repeat(64),
        // nothing listens here, so the call-back verification fails fast
        port: 1,
        protocol: Scheme::Https,
        download: false,
        sendit: None,
    }
}

pub fn meta(id: &str, name: &str, size: u64, sha256: Option<String>) -> FileMeta {
    FileMeta { id: id.into(), file_name: name.into(), size, file_type: "application/octet-stream".into(), sha256, preview: None, metadata: None }
}

pub async fn prepare(to: &Peer, files: Vec<FileMeta>, pin: Option<&str>) -> reqwest::Response {
    let c = PeerClient::new(Some(to.node.fingerprint())).unwrap();
    let req = PrepareUploadRequest { info: fake_info(), files: files.into_iter().map(|f| (f.id.clone(), f)).collect() };
    let mut r = c.http.post(format!("https://{}{API}/prepare-upload", to.addr())).json(&req);
    if let Some(p) = pin {
        r = r.query(&[("pin", p)]);
    }
    r.send().await.unwrap()
}

pub async fn upload(to: &Peer, session: &str, file_id: &str, token: &str, body: Vec<u8>) -> u16 {
    let c = PeerClient::new(Some(to.node.fingerprint())).unwrap();
    c.http
        .post(format!("https://{}{API}/upload", to.addr()))
        .query(&[("sessionId", session), ("fileId", file_id), ("token", token)])
        .body(body)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}
```
Add to `crates/core/Cargo.toml` `[dev-dependencies]`: `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }`, `hex = "0.4"`, `sha2 = "0.10"`, `walkdir = "2"`.

- [ ] **Step 2: Write failing tests** `crates/core/tests/receive.rs`
```rust
mod common;
use common::*;
use sendit_core::{protocol::*, tls::PeerClient, Event, FailReason};

async fn accepted(to: &Peer, files: Vec<FileMeta>) -> PrepareUploadResponse {
    let r = prepare(to, files, None).await;
    assert_eq!(r.status(), 200);
    r.json().await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn receives_a_file() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut rx = b.node.subscribe();
    let r = accepted(&b, vec![meta("1", "hello.txt", 5, None)]).await;
    assert_eq!(upload(&b, &r.session_id, "1", &r.files["1"], b"hello".to_vec()).await, 200);
    let Event::TransferDone { paths, .. } = wait_for(&mut rx, |e| matches!(e, Event::TransferDone { .. })).await else { unreachable!() };
    assert_eq!(paths, vec![b.down().join("hello.txt")]);
    assert_eq!(std::fs::read(b.down().join("hello.txt")).unwrap(), b"hello");
}

#[tokio::test(flavor = "multi_thread")]
async fn hostile_names_stay_inside_download_dir() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let names = ["../../escape.txt", "..\\..\\win.txt", "/abs/x.txt"];
    let files = names.iter().enumerate().map(|(i, n)| meta(&i.to_string(), n, 1, None)).collect();
    let r = accepted(&b, files).await;
    for i in 0..names.len() {
        let id = i.to_string();
        assert_eq!(upload(&b, &r.session_id, &id, &r.files[&id], b"x".to_vec()).await, 200);
    }
    assert!(b.down().join("escape.txt").exists());
    assert!(b.down().join("win.txt").exists());
    assert!(b.down().join("abs").join("x.txt").exists());
    assert!(!b.dir.path().join("escape.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn never_overwrites() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    std::fs::write(b.down().join("same.txt"), "old").unwrap();
    let r = accepted(&b, vec![meta("1", "same.txt", 1, None), meta("2", "same.txt", 1, None)]).await;
    assert_eq!(upload(&b, &r.session_id, "1", &r.files["1"], b"a".to_vec()).await, 200);
    assert_eq!(upload(&b, &r.session_id, "2", &r.files["2"], b"b".to_vec()).await, 200);
    assert_eq!(std::fs::read_to_string(b.down().join("same.txt")).unwrap(), "old");
    assert!(b.down().join("same (1).txt").exists());
    assert!(b.down().join("same (2).txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn checksum_mismatch_is_422_and_leaves_nothing() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "c.txt", 5, Some("00".repeat(32)))]).await;
    assert_eq!(upload(&b, &r.session_id, "1", &r.files["1"], b"hello".to_vec()).await, 422);
    assert!(!b.down().join("c.txt").exists());
    assert!(no_part_files(&b.down()));
}

#[tokio::test(flavor = "multi_thread")]
async fn more_bytes_than_declared_is_400() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "big.txt", 10, None)]).await;
    assert_eq!(upload(&b, &r.session_id, "1", &r.files["1"], vec![0; 20]).await, 400);
    assert!(!b.down().join("big.txt").exists());
    assert!(no_part_files(&b.down()));
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_token_is_403() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    assert_eq!(upload(&b, &r.session_id, "1", "nope", b"a".to_vec()).await, 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn pin_is_enforced() {
    let b = peer_with("bob", |c| c.pin = Some("1234".into())).await;
    auto_accept(&b.node);
    let f = || vec![meta("1", "a.txt", 1, None)];
    assert_eq!(prepare(&b, f(), None).await.status(), 401);
    assert_eq!(prepare(&b, f(), Some("9999")).await.status(), 401);
    assert_eq!(prepare(&b, f(), Some("1234")).await.status(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn second_sender_gets_409_while_busy() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    assert_eq!(prepare(&b, vec![meta("2", "b.txt", 1, None)], None).await.status(), 409);
}

#[tokio::test(flavor = "multi_thread")]
async fn vanished_sender_frees_the_session() {
    let b = peer("bob").await; // idle_timeout = 1s
    auto_accept(&b.node);
    let mut rx = b.node.subscribe();
    accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    let Event::TransferFailed { reason, .. } = wait_for(&mut rx, |e| matches!(e, Event::TransferFailed { .. })).await else { unreachable!() };
    assert_eq!(reason, FailReason::PeerGone);
    assert_eq!(prepare(&b, vec![meta("2", "b.txt", 1, None)], None).await.status(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_request_is_204_and_blocked_is_403() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    assert_eq!(prepare(&b, vec![], None).await.status(), 204);
    b.node.block(&"F".repeat(64));
    assert_eq!(prepare(&b, vec![meta("1", "a.txt", 1, None)], None).await.status(), 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn decline_is_403() {
    let b = peer("bob").await;
    auto_decline(&b.node);
    assert_eq!(prepare(&b, vec![meta("1", "a.txt", 1, None)], None).await.status(), 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_sender_is_flagged() {
    let b = peer("bob").await;
    let mut rx = b.node.subscribe();
    let bb = b.node.clone();
    let task = tokio::spawn(async move {
        let Event::IncomingRequest { request } = wait_for(&mut rx, |e| matches!(e, Event::IncomingRequest { .. })).await else { unreachable!() };
        let _ = bb.respond(&request.request_id, sendit_core::Decision::Decline);
        request
    });
    prepare(&b, vec![meta("1", "a.txt", 1, None)], None).await;
    let req = task.await.unwrap();
    assert!(!req.sender_verified);
    assert_eq!(req.total_size, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_endpoint_fails_the_session() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut rx = b.node.subscribe();
    let r = accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    let c = PeerClient::new(Some(b.node.fingerprint())).unwrap();
    let s = c.http.post(format!("https://{}{API}/cancel", b.addr())).query(&[("sessionId", &r.session_id)]).send().await.unwrap();
    assert_eq!(s.status(), 200);
    let Event::TransferFailed { reason, .. } = wait_for(&mut rx, |e| matches!(e, Event::TransferFailed { .. })).await else { unreachable!() };
    assert_eq!(reason, FailReason::Cancelled);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p sendit-core --test receive`
Expected: compile error, `Decision` missing.

- [ ] **Step 4: Extend `node.rs`**

Add the imports `use crate::{events::FailReason, receive::{has_space, Pending, Session}, error::Error};` and `use std::path::PathBuf;` (if it isn't already imported). Then add to `Inner`:
```rust
    pub pending: Mutex<HashMap<String, Pending>>,
    /// LocalSend allows one incoming session at a time; others get 409.
    pub session: Mutex<Option<Session>>,
```
Initialize both in `Node::start` (`pending: Mutex::new(HashMap::new()), session: Mutex::new(None),`). Then add:
```rust
#[derive(Clone, Debug)]
pub enum Decision {
    /// `dest = None` uses `Config::download_dir`.
    Accept { dest: Option<PathBuf> },
    Decline,
}
```
and this method on `Node`:
```rust
    /// Answer an `Event::IncomingRequest`. If there isn't enough space in the
    /// destination, returns `Error::DiskFull` and the request stays pending,
    /// so the caller can pick another folder or decline.
    pub fn respond(&self, request_id: &str, decision: Decision) -> Result<()> {
        let mut pending = self.inner.pending.lock().unwrap();
        let p = pending.remove(request_id).ok_or_else(|| Error::NoSuchRequest(request_id.into()))?;
        if let Decision::Accept { dest } = &decision {
            let dir = dest.clone().unwrap_or_else(|| self.inner.cfg.download_dir.clone());
            if !has_space(&dir, p.total) {
                pending.insert(request_id.to_string(), p);
                return Err(Error::DiskFull);
            }
        }
        let _ = p.tx.send(decision);
        Ok(())
    }
```
Do not import `FailReason` unless the compiler asks for it. Task 9 uses it.

- [ ] **Step 5: Implement `crates/core/src/receive.rs`**
```rust
//! Incoming side: one session at a time, files streamed to `.sendit-part`, then renamed.
use crate::{
    events::{stalled, Device, Direction, Event, FailReason, Meter},
    node::{Decision, Inner},
    protocol::*,
    sanitize::{sanitize_relative, unique_path},
};
use axum::{body::Body, http::StatusCode};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) const PART_SUFFIX: &str = ".sendit-part";

pub(crate) struct Pending {
    pub tx: oneshot::Sender<Decision>,
    pub total: u64,
}

pub(crate) struct Session {
    pub id: String,
    pub dest: PathBuf,
    pub files: HashMap<String, Slot>,
    pub meter: Arc<Meter>,
    pub cancel: CancellationToken,
    pub paths: Vec<PathBuf>,
}

pub(crate) struct Slot {
    pub meta: FileMeta,
    pub token: String,
    pub state: SlotState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotState {
    Waiting,
    Receiving,
    Done,
}

pub(crate) fn has_space(dir: &Path, need: u64) -> bool {
    fs2::available_space(dir).map(|free| free >= need).unwrap_or(true)
}

impl FailReason {
    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::Checksum => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Protocol(_) | Self::PeerGone => StatusCode::BAD_REQUEST,
            Self::Cancelled => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Inner {
    /// Returns None if another session is already active.
    pub(crate) fn open_session(self: &Arc<Self>, from: Device, files: Vec<FileMeta>, total: u64, dest: PathBuf) -> Option<PrepareUploadResponse> {
        let id = Uuid::new_v4().to_string();
        let meter = Meter::new(self.events.clone(), id.clone(), Direction::Incoming, total);
        let cancel = CancellationToken::new();
        let mut tokens = HashMap::new();
        let mut slots = HashMap::new();
        for meta in &files {
            let token = Uuid::new_v4().to_string();
            tokens.insert(meta.id.clone(), token.clone());
            slots.insert(meta.id.clone(), Slot { meta: meta.clone(), token, state: SlotState::Waiting });
        }
        {
            let mut guard = self.session.lock().unwrap();
            if guard.is_some() {
                return None;
            }
            *guard = Some(Session { id: id.clone(), dest, files: slots, meter: meter.clone(), cancel: cancel.clone(), paths: vec![] });
        }
        self.emit(Event::TransferStarted { transfer_id: id.clone(), direction: Direction::Incoming, peer: from.fingerprint, files, total });
        let n = self.clone();
        let (idle, sid) = (self.cfg.idle_timeout, id.clone());
        tokio::spawn(async move {
            tokio::select! {
                _ = stalled(&meter, idle) => { n.fail_incoming(&sid, FailReason::PeerGone); }
                _ = cancel.cancelled() => {}
            }
        });
        Some(PrepareUploadResponse { session_id: id, files: tokens })
    }

    pub(crate) fn claim_file(&self, q: &UploadQuery) -> Result<(FileMeta, PathBuf, Arc<Meter>, CancellationToken), StatusCode> {
        let mut guard = self.session.lock().unwrap();
        let s = guard.as_mut().filter(|s| s.id == q.session_id).ok_or(StatusCode::FORBIDDEN)?;
        let slot = s.files.get_mut(&q.file_id).ok_or(StatusCode::FORBIDDEN)?;
        if slot.token != q.token || slot.state != SlotState::Waiting {
            return Err(StatusCode::FORBIDDEN);
        }
        slot.state = SlotState::Receiving;
        Ok((slot.meta.clone(), s.dest.clone(), s.meter.clone(), s.cancel.clone()))
    }

    pub(crate) fn file_finished(&self, session_id: &str, file_id: &str, path: PathBuf) {
        let finished = {
            let mut guard = self.session.lock().unwrap();
            let Some(s) = guard.as_mut().filter(|s| s.id == session_id) else { return };
            if let Some(slot) = s.files.get_mut(file_id) {
                slot.state = SlotState::Done;
            }
            s.paths.push(path);
            if s.files.values().all(|f| f.state == SlotState::Done) { guard.take() } else { None }
        };
        if let Some(s) = finished {
            s.cancel.cancel(); // stops the watchdog
            self.emit(Event::TransferDone { transfer_id: s.id, direction: Direction::Incoming, paths: s.paths });
        }
    }

    /// Ends the session `session_id` if it is still active. Returns false if it wasn't.
    pub(crate) fn fail_incoming(&self, session_id: &str, reason: FailReason) -> bool {
        let taken = {
            let mut guard = self.session.lock().unwrap();
            if guard.as_ref().is_some_and(|s| s.id == session_id) { guard.take() } else { None }
        };
        let Some(s) = taken else { return false };
        s.cancel.cancel();
        self.emit(Event::TransferFailed { transfer_id: s.id, direction: Direction::Incoming, reason });
        true
    }
}

pub(crate) async fn write_file(meta: &FileMeta, dest: &Path, body: Body, meter: &Meter, cancel: &CancellationToken) -> Result<PathBuf, FailReason> {
    let rel = sanitize_relative(&meta.file_name);
    let target = dest.join(&rel);
    let dir = target.parent().unwrap_or(dest).to_path_buf();
    tokio::fs::create_dir_all(&dir).await?;
    let part = dir.join(format!(".{}{PART_SUFFIX}", Uuid::new_v4()));
    let result: Result<PathBuf, FailReason> = async {
        stream_into(&part, meta, body, meter, cancel).await?;
        let final_path = unique_path(dest, &rel);
        tokio::fs::rename(&part, &final_path).await?;
        Ok(final_path)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    result
}

async fn stream_into(part: &Path, meta: &FileMeta, body: Body, meter: &Meter, cancel: &CancellationToken) -> Result<(), FailReason> {
    let mut file = tokio::fs::File::create(part).await?;
    let mut hasher = meta.sha256.as_ref().map(|_| Sha256::new());
    let mut stream = body.into_data_stream();
    let mut written = 0u64;
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => return Err(FailReason::Cancelled),
            next = stream.next() => next,
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|_| FailReason::PeerGone)?;
        written += chunk.len() as u64;
        if written > meta.size {
            return Err(FailReason::Protocol("peer sent more bytes than it declared".into()));
        }
        if let Some(h) = hasher.as_mut() {
            h.update(&chunk);
        }
        file.write_all(&chunk).await?;
        meter.add(chunk.len() as u64);
    }
    if written != meta.size {
        return Err(FailReason::PeerGone);
    }
    file.flush().await?;
    if let (Some(h), Some(expected)) = (hasher, &meta.sha256) {
        if !hex::encode(h.finalize()).eq_ignore_ascii_case(expected) {
            return Err(FailReason::Checksum);
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Add the handlers to `server.rs`**

Extend the imports:
```rust
use crate::{
    discovery,
    events::{Event, IncomingRequest},
    node::{Decision, Inner},
    protocol::*,
    receive::{self, has_space, Pending},
    trust::TrustLevel,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use tokio::sync::oneshot;
use uuid::Uuid;
```
Router:
```rust
pub(crate) fn router(n: Arc<Inner>) -> Router {
    Router::new()
        .route(&format!("{API}/register"), post(register))
        .route(&format!("{API}/prepare-upload"), post(prepare_upload))
        .route(&format!("{API}/upload"), post(upload))
        .route(&format!("{API}/cancel"), post(cancel))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(n)
}
```
Handlers:
```rust
async fn prepare_upload(
    State(n): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Query(q): Query<PinQuery>,
    Json(req): Json<PrepareUploadRequest>,
) -> Response {
    if let Some(pin) = &n.cfg.pin {
        if q.pin.as_deref() != Some(pin.as_str()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    let Some(from) = n.see_device(&req.info, remote.ip()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if from.trust == TrustLevel::Blocked {
        return StatusCode::FORBIDDEN.into_response();
    }
    if n.session.lock().unwrap().is_some() {
        return StatusCode::CONFLICT.into_response();
    }
    if req.files.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    // Prove the sender owns the key it claims: call it back, pinned to that key.
    let sender_verified = req.info.protocol == Scheme::Https
        && discovery::register_with(&n, SocketAddr::new(remote.ip(), req.info.port), Some(&req.info.fingerprint)).await.is_ok();
    let files: Vec<FileMeta> = req.files.into_values().collect();
    let total: u64 = files.iter().map(|f| f.size).sum();
    let auto = sender_verified && from.trust == TrustLevel::Verified && n.trust.lock().unwrap().auto_accept(&from.fingerprint);
    let decision = if auto {
        Decision::Accept { dest: None }
    } else {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        n.pending.lock().unwrap().insert(request_id.clone(), Pending { tx, total });
        n.emit(Event::IncomingRequest {
            request: IncomingRequest {
                request_id: request_id.clone(),
                from: from.clone(),
                files: files.clone(),
                total_size: total,
                fits_on_disk: has_space(&n.cfg.download_dir, total),
                sender_verified,
            },
        });
        let answer = tokio::time::timeout(n.cfg.request_timeout, rx).await;
        n.pending.lock().unwrap().remove(&request_id);
        match answer {
            Ok(Ok(d)) => d,
            _ => Decision::Decline,
        }
    };
    let Decision::Accept { dest } = decision else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let dest = dest.unwrap_or_else(|| n.cfg.download_dir.clone());
    match n.open_session(from, files, total, dest) {
        Some(resp) => Json(resp).into_response(),
        None => StatusCode::CONFLICT.into_response(),
    }
}

async fn upload(State(n): State<Arc<Inner>>, Query(q): Query<UploadQuery>, body: Body) -> StatusCode {
    let (meta, dest, meter, cancel) = match n.claim_file(&q) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match receive::write_file(&meta, &dest, body, &meter, &cancel).await {
        Ok(path) => {
            n.file_finished(&q.session_id, &q.file_id, path);
            StatusCode::OK
        }
        Err(reason) => {
            let code = reason.status();
            n.fail_incoming(&q.session_id, reason);
            code
        }
    }
}

async fn cancel(State(n): State<Arc<Inner>>, Query(q): Query<SessionQuery>) -> StatusCode {
    n.fail_incoming(&q.session_id, crate::events::FailReason::Cancelled);
    StatusCode::OK
}
```

- [ ] **Step 7: `lib.rs`**: add `mod receive;` and change the re-export to `pub use node::{Config, Decision, Node};`.

- [ ] **Step 8: Run tests**

Run: `cargo test -p sendit-core`
Expected: all pass (13 new in `receive`).

- [ ] **Step 9: Commit**
```bash
git add crates/core
git commit -m "feat(core): receive files with accept flow, PIN, watchdog and safe writes"
```

---

### Task 9: Sending

**Files:**
- Create: `crates/core/src/send.rs`, `crates/core/tests/transfer.rs`
- Modify: `crates/core/src/node.rs` (field `outgoing`; `send`, `cancel`), `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `PeerClient`, `Meter`, `stalled`, `Inner::fail_incoming`
- Produces: `Node::send(&self, fingerprint: &str, paths: Vec<PathBuf>, pin: Option<String>) -> Result<String /* transfer_id */>`, `Node::cancel(&self, transfer_id: &str) -> bool`. Crate-internal: `Inner.outgoing: Mutex<HashMap<String, CancellationToken>>`, `send::collect_files(&[PathBuf]) -> io::Result<Vec<(FileMeta, PathBuf)>>`

- [ ] **Step 1: Write failing tests** `crates/core/tests/transfer.rs`
```rust
mod common;
use common::*;
use sendit_core::{Error, Event, FailReason};

async fn pair() -> (Peer, Peer) {
    let a = peer("alice").await;
    let b = peer("bob").await;
    a.node.add_peer(b.addr()).await.unwrap();
    (a, b)
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_files_of_various_sizes() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    for (i, len) in [0usize, 1, 10 * 1024 * 1024].into_iter().enumerate() {
        let src = write_file(a.dir.path(), &format!("f{i}.bin"), len);
        a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap();
        assert_eq!(sha(&b.down().join(format!("f{i}.bin"))), sha(&src));
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: 1 GB"]
async fn sends_one_gigabyte() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    let src = write_file(a.dir.path(), "big.bin", 1 << 30);
    a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap();
    assert_eq!(sha(&b.down().join("big.bin")), sha(&src));
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_folders_with_structure() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    write_file(a.dir.path(), "album/a.jpg", 100);
    write_file(a.dir.path(), "album/sub/b.jpg", 200);
    let notes = write_file(a.dir.path(), "notes.txt", 3);
    a.node.send(b.node.fingerprint(), vec![a.dir.path().join("album"), notes], None).await.unwrap();
    assert!(b.down().join("album/a.jpg").exists());
    assert!(b.down().join("album/sub/b.jpg").exists());
    assert!(b.down().join("notes.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn decline_reported() {
    let (a, b) = pair().await;
    auto_decline(&b.node);
    let src = write_file(a.dir.path(), "x.txt", 3);
    let err = a.node.send(b.node.fingerprint(), vec![src], None).await.unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::Declined)), "{err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn pin_required_then_ok() {
    let a = peer("alice").await;
    let b = peer_with("bob", |c| c.pin = Some("4321".into())).await;
    a.node.add_peer(b.addr()).await.unwrap();
    auto_accept(&b.node);
    let src = write_file(a.dir.path(), "x.txt", 3);
    let err = a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::PinRequired)));
    a.node.send(b.node.fingerprint(), vec![src], Some("4321".into())).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn refuses_suspect_until_accepted() {
    let a = peer("alice").await;
    let bob1 = peer("bob").await;
    let bob2 = peer("bob").await;
    auto_accept(&bob2.node);
    a.node.add_peer(bob1.addr()).await.unwrap();
    a.node.add_peer(bob2.addr()).await.unwrap();
    let src = write_file(a.dir.path(), "x.txt", 3);
    let err = a.node.send(bob2.node.fingerprint(), vec![src.clone()], None).await.unwrap_err();
    assert!(matches!(err, Error::Suspect));
    a.node.accept_new_key(bob2.node.fingerprint());
    a.node.send(bob2.node.fingerprint(), vec![src], None).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn emits_started_progress_done() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    let mut rx = a.node.subscribe();
    let src = write_file(a.dir.path(), "x.bin", 1024 * 1024);
    let id = a.node.send(b.node.fingerprint(), vec![src], None).await.unwrap();
    let mut saw = (false, false, false);
    while let Ok(e) = rx.try_recv() {
        match e {
            Event::TransferStarted { transfer_id, .. } if transfer_id == id => saw.0 = true,
            Event::Progress { transfer_id, .. } if transfer_id == id => saw.1 = true,
            Event::TransferDone { transfer_id, .. } if transfer_id == id => saw.2 = true,
            _ => {}
        }
    }
    assert_eq!(saw, (true, true, true));
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_device_errors() {
    let a = peer("alice").await;
    let err = a.node.send("ABCDEF", vec![], None).await.unwrap_err();
    assert!(matches!(err, Error::UnknownDevice(_)));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core --test transfer`
Expected: compile error, `send` missing on `Node`.

- [ ] **Step 3: Implement `crates/core/src/send.rs`**
```rust
//! Outgoing side: prepare-upload, then parallel streaming uploads.
use crate::{
    error::{Error, Result},
    events::{stalled, Device, Direction, Event, FailReason, Meter},
    node::Inner,
    protocol::*,
    tls::PeerClient,
    trust::TrustLevel,
};
use futures::{StreamExt, TryStreamExt};
use reqwest::header::CONTENT_LENGTH;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

const PARALLEL_UPLOADS: usize = 3;

pub(crate) async fn send(n: &Arc<Inner>, fingerprint: &str, paths: Vec<PathBuf>, pin: Option<String>) -> Result<String> {
    let device = n
        .devices
        .lock()
        .unwrap()
        .get(&fingerprint.to_uppercase())
        .cloned()
        .ok_or_else(|| Error::UnknownDevice(fingerprint.into()))?;
    match device.trust {
        TrustLevel::Suspect => return Err(Error::Suspect),
        TrustLevel::Blocked => return Err(Error::Blocked),
        _ => {}
    }
    let files = collect_files(&paths)?;
    let total = files.iter().map(|(m, _)| m.size).sum();
    let id = Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    n.outgoing.lock().unwrap().insert(id.clone(), cancel.clone());
    n.emit(Event::TransferStarted {
        transfer_id: id.clone(),
        direction: Direction::Outgoing,
        peer: device.fingerprint.clone(),
        files: files.iter().map(|(m, _)| m.clone()).collect(),
        total,
    });
    let meter = Meter::new(n.events.clone(), id.clone(), Direction::Outgoing, total);
    let result = run(n, &device, files, pin, &meter, &cancel).await;
    n.outgoing.lock().unwrap().remove(&id);
    match result {
        Ok(()) => {
            n.emit(Event::TransferDone { transfer_id: id.clone(), direction: Direction::Outgoing, paths });
            Ok(id)
        }
        Err(reason) => {
            n.emit(Event::TransferFailed { transfer_id: id, direction: Direction::Outgoing, reason: reason.clone() });
            Err(Error::Transfer(reason))
        }
    }
}

async fn run(
    n: &Inner,
    device: &Device,
    files: Vec<(FileMeta, PathBuf)>,
    pin: Option<String>,
    meter: &Arc<Meter>,
    cancel: &CancellationToken,
) -> std::result::Result<(), FailReason> {
    let client = PeerClient::new(Some(&device.fingerprint)).map_err(|e| FailReason::Protocol(e.to_string()))?;
    let base = device.base_url();
    let request = PrepareUploadRequest {
        info: n.my_info(),
        files: files.iter().map(|(m, _)| (m.id.clone(), m.clone())).collect(),
    };
    let mut prepare = client
        .http
        .post(format!("{base}/prepare-upload"))
        .json(&request)
        .timeout(n.cfg.request_timeout + Duration::from_secs(10));
    if let Some(pin) = &pin {
        prepare = prepare.query(&[("pin", pin)]);
    }
    let resp = tokio::select! {
        r = prepare.send() => r.map_err(|_| connect_failure(&client, device))?,
        _ = cancel.cancelled() => return Err(FailReason::Cancelled),
    };
    match resp.status().as_u16() {
        200 => {}
        204 => return Ok(()),
        401 => return Err(FailReason::PinRequired),
        403 => return Err(FailReason::Declined),
        409 | 429 => return Err(FailReason::Busy),
        s => return Err(FailReason::Protocol(format!("prepare-upload returned {s}"))),
    }
    let prep: PrepareUploadResponse = resp.json().await.map_err(|e| FailReason::Protocol(e.to_string()))?;
    // Files missing from the reply were skipped by the receiver.
    let jobs = files.into_iter().filter_map(|(meta, path)| {
        let token = prep.files.get(&meta.id)?.clone();
        Some((meta, path, token))
    });
    let uploads = futures::stream::iter(jobs)
        .map(|(meta, path, token)| upload_one(&client, &base, &prep.session_id, meta, path, token, meter.clone()))
        .buffer_unordered(PARALLEL_UPLOADS)
        .try_collect::<Vec<()>>();
    let outcome = tokio::select! {
        r = uploads => r.map(|_| ()),
        _ = stalled(meter, n.cfg.idle_timeout) => Err(FailReason::PeerGone),
        _ = cancel.cancelled() => Err(FailReason::Cancelled),
    };
    if outcome.is_err() {
        let _ = client
            .http
            .post(format!("{base}/cancel"))
            .query(&[("sessionId", &prep.session_id)])
            .timeout(Duration::from_secs(3))
            .send()
            .await;
    }
    outcome
}

async fn upload_one(
    client: &PeerClient,
    base: &str,
    session_id: &str,
    meta: FileMeta,
    path: PathBuf,
    token: String,
    meter: Arc<Meter>,
) -> std::result::Result<(), FailReason> {
    let file = tokio::fs::File::open(&path).await?;
    let body = ReaderStream::with_capacity(file, 256 * 1024).inspect_ok(move |chunk| meter.add(chunk.len() as u64));
    let resp = client
        .http
        .post(format!("{base}/upload"))
        .query(&[("sessionId", session_id), ("fileId", meta.id.as_str()), ("token", token.as_str())])
        .header(CONTENT_LENGTH, meta.size)
        .body(reqwest::Body::wrap_stream(body))
        .send()
        .await
        .map_err(|_| FailReason::PeerGone)?;
    match resp.status().as_u16() {
        200 => Ok(()),
        422 => Err(FailReason::Checksum),
        403 | 409 => Err(FailReason::Cancelled),
        s => Err(FailReason::Protocol(format!("upload returned {s}"))),
    }
}

fn connect_failure(client: &PeerClient, device: &Device) -> FailReason {
    match client.seen_fingerprint() {
        Some(seen) if !seen.eq_ignore_ascii_case(&device.fingerprint) => FailReason::IdentityMismatch,
        _ => FailReason::PeerGone,
    }
}

/// Files become `name`; folders become `folder/sub/name` entries.
pub(crate) fn collect_files(paths: &[PathBuf]) -> std::io::Result<Vec<(FileMeta, PathBuf)>> {
    let mut out = Vec::new();
    for root in paths {
        if root.is_dir() {
            let base = root.parent().unwrap_or(root);
            for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
                let entry = entry.map_err(std::io::Error::other)?;
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(base).unwrap_or(entry.path());
                    out.push(meta_for(entry.path(), &rel_name(rel))?);
                }
            }
        } else {
            let name = root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            out.push(meta_for(root, &name)?);
        }
    }
    Ok(out)
}

fn rel_name(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn meta_for(path: &Path, name: &str) -> std::io::Result<(FileMeta, PathBuf)> {
    let size = std::fs::metadata(path)?.len();
    let meta = FileMeta {
        id: Uuid::new_v4().to_string(),
        file_name: name.to_string(),
        size,
        file_type: mime_guess::from_path(path).first_or_octet_stream().essence_str().to_string(),
        sha256: None,
        preview: None,
        metadata: None,
    };
    Ok((meta, path.to_path_buf()))
}
```

- [ ] **Step 4: Extend `node.rs`**

Add `use tokio_util::sync::CancellationToken;`, plus `use crate::{send, events::FailReason};` and `use std::path::PathBuf;` if they are missing. Add the field `pub outgoing: Mutex<HashMap<String, CancellationToken>>,` to `Inner` and initialize it with `outgoing: Mutex::new(HashMap::new()),`. Add to `Node`:
```rust
    /// Runs the whole transfer. Emits TransferStarted first, so a UI can grab
    /// the id for `cancel`. Returns that id on success.
    pub async fn send(&self, fingerprint: &str, paths: Vec<PathBuf>, pin: Option<String>) -> Result<String> {
        send::send(&self.inner, fingerprint, paths, pin).await
    }

    /// Cancels an outgoing or incoming transfer. Returns false if it was unknown.
    pub fn cancel(&self, transfer_id: &str) -> bool {
        if let Some(token) = self.inner.outgoing.lock().unwrap().get(transfer_id) {
            token.cancel();
            return true;
        }
        self.inner.fail_incoming(transfer_id, FailReason::Cancelled)
    }
```

- [ ] **Step 5: `lib.rs`**: add `mod send;`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p sendit-core`
Expected: all pass, with 1 ignored (the 1 GB test).

- [ ] **Step 7: Commit**
```bash
git add crates/core
git commit -m "feat(core): send files and folders with progress, PIN and cancel"
```

---

### Task 10: Multicast discovery + subnet scan

**Files:**
- Modify: `crates/core/src/discovery.rs`, `crates/core/src/node.rs`
- Create: `crates/core/tests/multicast.rs`

**Interfaces:**
- Produces: `Node::announce(&self)`, `Node::scan(&self) -> Vec<Device>`. `Node::start` spawns the multicast listener and announces when `cfg.multicast` is set. Crate-internal: `discovery::{spawn_listener(Arc<Inner>) -> io::Result<()>, send_announcement(&Inner, announce: bool), scan(&Arc<Inner>) -> Vec<Device>, accept_announcement(&Announcement, own_fp: &str) -> bool, ipv4_interfaces() -> Vec<Ipv4Addr>}`

- [ ] **Step 1: Write the failing unit test** (append to `discovery.rs`)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ann(fp: &str, protocol: Scheme) -> Announcement {
        Announcement {
            info: DeviceInfo {
                alias: "x".into(), version: "2.1".into(), device_model: None, device_type: None,
                fingerprint: fp.into(), port: 53317, protocol, download: false, sendit: None,
            },
            announce: true,
        }
    }

    #[test]
    fn ignores_self_and_plain_http_peers() {
        assert!(accept_announcement(&ann("AA", Scheme::Https), "BB"));
        assert!(!accept_announcement(&ann("aa", Scheme::Https), "AA"));
        assert!(!accept_announcement(&ann("CC", Scheme::Http), "AA"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p sendit-core discovery`
Expected: compile error, `accept_announcement` missing.

- [ ] **Step 3: Implement** (add to `discovery.rs`)
```rust
use futures::StreamExt;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

pub(crate) fn accept_announcement(a: &Announcement, own_fp: &str) -> bool {
    // ponytail: LocalSend's "encryption off" mode has no cert to pin; ignored in v1.
    !a.info.fingerprint.eq_ignore_ascii_case(own_fp) && a.info.protocol == Scheme::Https
}

pub(crate) fn ipv4_interfaces() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .collect()
}

fn interfaces_or_any() -> Vec<Ipv4Addr> {
    let v = ipv4_interfaces();
    if v.is_empty() { vec![Ipv4Addr::UNSPECIFIED] } else { v }
}

fn multicast_socket(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(unix)]
    s.set_reuse_port(true)?;
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    for ip in interfaces_or_any() {
        if let Err(e) = s.join_multicast_v4(&MULTICAST_ADDR, &ip) {
            tracing::debug!("join multicast on {ip}: {e}");
        }
    }
    s.set_multicast_loop_v4(true)?;
    s.set_nonblocking(true)?;
    tokio::net::UdpSocket::from_std(s.into())
}

pub(crate) fn spawn_listener(n: Arc<Inner>) -> std::io::Result<()> {
    let sock = multicast_socket(n.cfg.multicast_port)?;
    tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let Ok((len, src)) = sock.recv_from(&mut buf).await else { continue };
            let Ok(a) = serde_json::from_slice::<Announcement>(&buf[..len]) else { continue };
            if !accept_announcement(&a, &n.identity.fingerprint) {
                continue;
            }
            let n = n.clone();
            tokio::spawn(async move { handle_announcement(&n, a, src.ip()).await });
        }
    });
    Ok(())
}

async fn handle_announcement(n: &Arc<Inner>, a: Announcement, ip: IpAddr) {
    if !a.announce {
        n.see_device(&a.info, ip);
        return;
    }
    // Answer like LocalSend does: HTTP /register first, UDP reply if that fails.
    let addr = SocketAddr::new(ip, a.info.port);
    if register_with(n, addr, Some(&a.info.fingerprint)).await.is_err() {
        n.see_device(&a.info, ip);
        send_announcement(n, false);
    }
}

pub(crate) fn send_announcement(n: &Inner, announce: bool) {
    let Ok(msg) = serde_json::to_vec(&Announcement { info: n.my_info(), announce }) else { return };
    let target = SocketAddrV4::new(MULTICAST_ADDR, n.cfg.multicast_port);
    for ip in interfaces_or_any() {
        let sent = (|| -> std::io::Result<()> {
            let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            s.bind(&SocketAddrV4::new(ip, 0).into())?;
            s.set_multicast_if_v4(&ip)?;
            s.set_multicast_loop_v4(true)?;
            s.send_to(&msg, &target.into())?;
            Ok(())
        })();
        if let Err(e) = sent {
            tracing::debug!("announce on {ip}: {e}");
        }
    }
}

/// Fallback when multicast is blocked: try /register on every host of our /24s.
// ponytail: assumes /24 subnets; read real netmasks if users report misses.
pub(crate) async fn scan(n: &Arc<Inner>) -> Vec<Device> {
    let own: HashSet<Ipv4Addr> = ipv4_interfaces().into_iter().collect();
    let targets: Vec<SocketAddr> = own
        .iter()
        .flat_map(|ip| {
            let o = ip.octets();
            (1..=254u8).map(move |h| Ipv4Addr::new(o[0], o[1], o[2], h))
        })
        .filter(|ip| !own.contains(ip))
        .map(|ip| SocketAddr::new(IpAddr::V4(ip), DEFAULT_PORT))
        .collect();
    futures::stream::iter(targets)
        .map(|addr| {
            let n = n.clone();
            async move { register_with(&n, addr, None).await.ok() }
        })
        .buffer_unordered(64)
        .filter_map(|d| async { d })
        .collect()
        .await
}
```
Merge these `use` lines with the existing imports at the top of the file.

- [ ] **Step 4: Wire into `node.rs`**

In `Node::start`, just before `Ok(Node { .. })`:
```rust
        if inner.cfg.multicast {
            discovery::spawn_listener(inner.clone())?;
            discovery::send_announcement(&inner, true);
        }
```
Add to `Node`:
```rust
    /// Shout "I'm here" on the LAN. Peers answer via /register.
    pub fn announce(&self) {
        discovery::send_announcement(&self.inner, true);
    }

    /// Slow fallback for networks that block multicast.
    pub async fn scan(&self) -> Vec<Device> {
        discovery::scan(&self.inner).await
    }
```

- [ ] **Step 5: Multicast integration test** `crates/core/tests/multicast.rs`
```rust
mod common;
use common::*;
use sendit_core::Event;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a multicast-capable network interface"]
async fn peers_find_each_other_by_multicast() {
    let mport = 53399;
    let a = peer_with("alice", |c| { c.multicast = true; c.multicast_port = mport; }).await;
    let mut rx = a.node.subscribe();
    let _b = peer_with("bob", |c| { c.multicast = true; c.multicast_port = mport; }).await;
    wait_for(&mut rx, |e| matches!(e, Event::DeviceUpdated { device } if device.alias == "bob")).await;
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p sendit-core`, then `cargo test -p sendit-core --test multicast -- --ignored`
Expected: everything passes. Run the ignored test on a dev machine. If it fails there, note which OS in the PR.

- [ ] **Step 7: Commit**
```bash
git add crates/core
git commit -m "feat(core): multicast discovery and subnet scan fallback"
```

---

### Task 11: Network chaos tests

**Files:**
- Modify: `crates/core/tests/common/mod.rs` (proxy)
- Create: `crates/core/tests/chaos.rs`

**Interfaces:**
- Consumes: `Node::{add_peer, send, cancel}`, events
- Produces: test helper `proxy(target: SocketAddr, mode: Chaos) -> SocketAddr` with `Chaos::{Cut(u64), Throttle(u64), Stall(u64)}`

- [ ] **Step 1: Add the proxy helper** (append to `common/mod.rs`)
```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Copy)]
pub enum Chaos {
    /// Drop the connection after this many client->server bytes.
    Cut(u64),
    /// Forward client->server at this many bytes per second.
    Throttle(u64),
    /// Stop forwarding (but keep the socket open) after this many bytes.
    Stall(u64),
}

pub async fn proxy(target: SocketAddr, mode: Chaos) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((inbound, _)) = listener.accept().await {
            tokio::spawn(async move {
                let Ok(outbound) = TcpStream::connect(target).await else { return };
                let (mut ir, mut iw) = inbound.into_split();
                let (mut or, mut ow) = outbound.into_split();
                let up = async {
                    let mut buf = [0u8; 16 * 1024];
                    let mut total = 0u64;
                    loop {
                        let n = ir.read(&mut buf).await?;
                        if n == 0 {
                            return Ok::<_, std::io::Error>(());
                        }
                        total += n as u64;
                        match mode {
                            Chaos::Cut(limit) if total > limit => return Ok(()),
                            Chaos::Stall(limit) if total > limit => std::future::pending::<()>().await,
                            Chaos::Throttle(bps) => {
                                tokio::time::sleep(Duration::from_millis(n as u64 * 1000 / bps)).await
                            }
                            _ => {}
                        }
                        ow.write_all(&buf[..n]).await?;
                    }
                };
                let down = tokio::io::copy(&mut or, &mut iw);
                // Whichever side ends first tears down both sockets.
                tokio::select! { _ = up => {}, _ = down => {} }
            });
        }
    });
    addr
}
```

- [ ] **Step 2: Write the tests** `crates/core/tests/chaos.rs`
```rust
mod common;
use common::*;
use sendit_core::{Error, Event, FailReason};

const MB: usize = 1024 * 1024;

#[tokio::test(flavor = "multi_thread")]
async fn cut_mid_transfer_fails_cleanly_and_retry_succeeds() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut brx = b.node.subscribe();
    let via = proxy(b.addr(), Chaos::Cut(2 * MB as u64)).await;
    a.node.add_peer(via).await.unwrap();
    let src = write_file(a.dir.path(), "x.bin", 10 * MB);
    let err = a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::PeerGone)), "{err:?}");
    wait_for(&mut brx, |e| matches!(e, Event::TransferFailed { .. })).await;
    let down = b.down();
    eventually(|| no_part_files(&down)).await;
    assert!(!b.down().join("x.bin").exists());
    // Retry over a healthy path.
    a.node.add_peer(b.addr()).await.unwrap();
    a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap();
    assert_eq!(sha(&b.down().join("x.bin")), sha(&src));
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_link_still_completes_with_monotonic_progress() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    auto_accept(&b.node);
    let via = proxy(b.addr(), Chaos::Throttle(2 * MB as u64)).await;
    a.node.add_peer(via).await.unwrap();
    let mut rx = a.node.subscribe();
    let src = write_file(a.dir.path(), "slow.bin", 3 * MB);
    a.node.send(b.node.fingerprint(), vec![src.clone()], None).await.unwrap();
    assert_eq!(sha(&b.down().join("slow.bin")), sha(&src));
    let mut last = 0;
    while let Ok(e) = rx.try_recv() {
        if let Event::Progress { bytes, .. } = e {
            assert!(bytes >= last);
            last = bytes;
        }
    }
    assert_eq!(last, 3 * MB as u64);
}

#[tokio::test(flavor = "multi_thread")]
async fn stalled_link_times_out_on_both_sides() {
    let a = peer("alice").await; // idle_timeout = 1s
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut brx = b.node.subscribe();
    let via = proxy(b.addr(), Chaos::Stall(MB as u64)).await;
    a.node.add_peer(via).await.unwrap();
    let src = write_file(a.dir.path(), "stall.bin", 8 * MB);
    let started = std::time::Instant::now();
    let err = a.node.send(b.node.fingerprint(), vec![src], None).await.unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::PeerGone)), "{err:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
    wait_for(&mut brx, |e| matches!(e, Event::TransferFailed { .. })).await;
    let down = b.down();
    eventually(|| no_part_files(&down)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sender_cancel_mid_transfer() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut brx = b.node.subscribe();
    let via = proxy(b.addr(), Chaos::Throttle(MB as u64)).await;
    a.node.add_peer(via).await.unwrap();
    let mut arx = a.node.subscribe();
    let src = write_file(a.dir.path(), "c.bin", 8 * MB);
    let sender = {
        let node = a.node.clone();
        let fp = b.node.fingerprint().to_string();
        tokio::spawn(async move { node.send(&fp, vec![src], None).await })
    };
    let Event::Progress { transfer_id, .. } = wait_for(&mut arx, |e| matches!(e, Event::Progress { .. })).await else { unreachable!() };
    assert!(a.node.cancel(&transfer_id));
    let err = sender.await.unwrap().unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::Cancelled)), "{err:?}");
    wait_for(&mut brx, |e| matches!(e, Event::TransferFailed { .. })).await;
    let down = b.down();
    eventually(|| no_part_files(&down)).await;
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p sendit-core --test chaos`
Expected: 4 passed. If one fails, the bug is in `send.rs` or `receive.rs`, not in the test. Use superpowers:systematic-debugging.

- [ ] **Step 4: Commit**
```bash
git add crates/core/tests
git commit -m "test(core): network chaos (cut, throttle, stall, cancel)"
```

---

### Task 12: CLI

**Files:**
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/cli/tests/cli.rs`
- Modify: `Cargo.toml` (add `"crates/cli"` to members)

**Interfaces:**
- Consumes: the `Node` public API, `Config`, `Decision`, `Event`, `Device`, `TrustLevel`
- Produces: the `sendit` binary with the commands `devices`, `send`, `receive` and the global flags `--alias --port --data-dir --no-multicast`. `receive` prints `listening ...` as its first stdout line and `saved <path>` for each received file.

- [ ] **Step 1: `crates/cli/Cargo.toml`**
```toml
[package]
name = "sendit-cli"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "sendit"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
directories = "5"
gethostname = "0.5"
sendit-core = { path = "../core" }
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
```
Change the root `Cargo.toml` to `members = ["crates/core", "crates/cli"]`.

- [ ] **Step 2: Write the failing e2e test** `crates/cli/tests/cli.rs`
```rust
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[test]
fn send_and_receive_over_loopback() {
    let tmp = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("0.0.0.0:0").unwrap().local_addr().unwrap().port();
    let bin = env!("CARGO_BIN_EXE_sendit");
    let mut rx = Command::new(bin)
        .args(["--alias", "rx", "--no-multicast", "--port", &port.to_string(), "--data-dir"])
        .arg(tmp.path().join("rx"))
        .args(["receive", "--yes", "--dir"])
        .arg(tmp.path().join("down"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(rx.stdout.take().unwrap()).lines();
    assert!(lines.next().unwrap().unwrap().starts_with("listening"));

    let file = tmp.path().join("hello.txt");
    std::fs::write(&file, "hi from cli").unwrap();
    let out = Command::new(bin)
        .args(["--alias", "tx", "--no-multicast", "--port", "0", "--data-dir"])
        .arg(tmp.path().join("tx"))
        .arg("send")
        .arg(&file)
        .args(["--addr", &format!("127.0.0.1:{port}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let saved = lines.map(|l| l.unwrap()).find(|l| l.starts_with("saved")).unwrap();
    assert!(saved.contains("hello.txt"), "{saved}");
    assert_eq!(std::fs::read_to_string(tmp.path().join("down/hello.txt")).unwrap(), "hi from cli");
    rx.kill().unwrap();
}

#[test]
fn help_lists_commands() {
    let out = Command::new(env!("CARGO_BIN_EXE_sendit")).arg("--help").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for cmd in ["devices", "send", "receive"] {
        assert!(text.contains(cmd), "{text}");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p sendit-cli`
Expected: compile error, no `main.rs`.

- [ ] **Step 4: Implement `crates/cli/src/main.rs`**
```rust
use clap::{Parser, Subcommand};
use sendit_core::{protocol::DEFAULT_PORT, trust::TrustLevel, Config, Decision, Device, Direction, Event, Node};
use std::{io::Write, net::SocketAddr, path::PathBuf, time::{Duration, Instant}};
use tokio::sync::broadcast::error::RecvError;

type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(name = "sendit", version, about = "LocalSend-compatible file transfer")]
struct Cli {
    /// Name other devices see (default: hostname)
    #[arg(long, global = true)]
    alias: Option<String>,
    #[arg(long, global = true, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// Where identity and trust data are stored
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Skip multicast discovery (use --addr)
    #[arg(long, global = true)]
    no_multicast: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List devices on the network
    Devices {
        #[arg(long, default_value_t = 3)]
        wait: u64,
        /// Also probe every address on the subnet (slow, for networks that block multicast)
        #[arg(long)]
        scan: bool,
    },
    /// Send files or folders
    Send {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Device name or fingerprint prefix
        #[arg(long)]
        to: Option<String>,
        /// Send to this ip:port directly
        #[arg(long)]
        addr: Option<SocketAddr>,
        #[arg(long, default_value_t = 3)]
        wait: u64,
        #[arg(long)]
        scan: bool,
        #[arg(long)]
        pin: Option<String>,
        /// Accept a changed key for this device (e.g. after a reinstall)
        #[arg(long)]
        trust_new_key: bool,
    },
    /// Wait for incoming files
    Receive {
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Accept everything without asking
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        pin: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> BoxResult<()> {
    let data_dir = match &cli.data_dir {
        Some(d) => d.clone(),
        None => directories::ProjectDirs::from("", "", "sendit")
            .map(|d| d.data_dir().to_path_buf())
            .ok_or("cannot find a data directory; pass --data-dir")?,
    };
    let download_dir = match &cli.cmd {
        Cmd::Receive { dir: Some(d), .. } => d.clone(),
        _ => directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(|d| d.join("sendit")))
            .unwrap_or_else(|| data_dir.join("downloads")),
    };
    let alias = cli.alias.clone().unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned());
    let mut cfg = Config::new(alias, data_dir, download_dir);
    cfg.port = cli.port;
    cfg.multicast = !cli.no_multicast;
    if let Cmd::Receive { pin, .. } = &cli.cmd {
        cfg.pin = pin.clone();
    }
    let node = Node::start(cfg).await?;
    if node.port_fallback() {
        eprintln!("port {} is busy (is LocalSend running?), using {}", cli.port, node.port());
    }
    match cli.cmd {
        Cmd::Devices { wait, scan } => {
            discover(&node, wait, scan).await;
            print_devices(&node.devices());
        }
        Cmd::Send { paths, to, addr, wait, scan, pin, trust_new_key } => {
            let device = match addr {
                Some(a) => node.add_peer(a).await?,
                None => {
                    let to = to.ok_or("pass --to <name> or --addr <ip:port>")?;
                    discover(&node, wait, scan).await;
                    pick(&node.devices(), &to)?
                }
            };
            send(&node, device, paths, pin, trust_new_key).await?;
        }
        Cmd::Receive { yes, .. } => receive(&node, yes).await?,
    }
    Ok(())
}

async fn discover(node: &Node, wait: u64, scan: bool) {
    if scan {
        node.scan().await;
    }
    tokio::time::sleep(Duration::from_secs(wait)).await;
}

fn print_devices(devices: &[Device]) {
    if devices.is_empty() {
        println!("no devices found. Same Wi-Fi? Try --scan");
        return;
    }
    for d in devices {
        println!("{:<24} {:<9} {:<22} {:?}", d.alias, &d.fingerprint[..8.min(d.fingerprint.len())], d.addr, d.trust);
    }
}

fn pick(devices: &[Device], to: &str) -> BoxResult<Device> {
    let upper = to.to_uppercase();
    let matches: Vec<&Device> = devices
        .iter()
        .filter(|d| d.alias.eq_ignore_ascii_case(to) || (to.len() >= 6 && d.fingerprint.starts_with(&upper)))
        .collect();
    match matches.as_slice() {
        [d] => Ok((*d).clone()),
        [] => Err(format!("no device named {to:?}. Run `sendit devices`").into()),
        _ => Err(format!("{to:?} matches {} devices; use a fingerprint prefix", matches.len()).into()),
    }
}

async fn send(node: &Node, device: Device, paths: Vec<PathBuf>, pin: Option<String>, trust_new_key: bool) -> BoxResult<()> {
    if device.trust == TrustLevel::Suspect {
        if !trust_new_key {
            return Err(format!(
                "{} is using a different key than before. If it was reinstalled, rerun with --trust-new-key",
                device.alias
            )
            .into());
        }
        node.accept_new_key(&device.fingerprint);
    }
    let mut rx = node.subscribe();
    let progress = tokio::spawn(async move {
        let mut first: Option<Instant> = None;
        while let Ok(e) = rx.recv().await {
            if let Event::Progress { bytes, total, direction: Direction::Outgoing, .. } = e {
                let start = *first.get_or_insert_with(Instant::now);
                let secs = start.elapsed().as_secs_f64().max(0.001);
                let pct = if total == 0 { 100.0 } else { bytes as f64 * 100.0 / total as f64 };
                eprint!("\r{pct:>5.1}%  {:>8.1} MB/s", bytes as f64 / 1e6 / secs);
                let _ = std::io::stderr().flush();
            }
        }
    });
    eprintln!("waiting for {} to accept...", device.alias);
    let result = node.send(&device.fingerprint, paths, pin).await;
    progress.abort();
    eprintln!();
    result?;
    println!("sent to {}", device.alias);
    Ok(())
}

async fn receive(node: &Node, yes: bool) -> BoxResult<()> {
    println!("listening as {:?} on port {} (fingerprint {})", node.info().alias, node.port(), &node.fingerprint()[..8]);
    let mut rx = node.subscribe();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                node.shutdown();
                return Ok(());
            }
            e = rx.recv() => match e {
                Ok(Event::IncomingRequest { request }) => {
                    let note = match request.from.trust {
                        TrustLevel::Suspect => "  WARNING: this name was seen before with a different key",
                        _ if !request.sender_verified => "  (unverified sender)",
                        _ => "",
                    };
                    println!("{} wants to send {} file(s), {:.1} MB{note}", request.from.alias, request.files.len(), request.total_size as f64 / 1e6);
                    for f in &request.files {
                        println!("  {} ({} bytes)", f.file_name, f.size);
                    }
                    let accept = yes || ask("accept? [y/N] ").await;
                    let decision = if accept { Decision::Accept { dest: None } } else { Decision::Decline };
                    if let Err(e) = node.respond(&request.request_id, decision) {
                        eprintln!("{e}");
                        let _ = node.respond(&request.request_id, Decision::Decline);
                    }
                }
                Ok(Event::TransferDone { direction: Direction::Incoming, paths, .. }) => {
                    for p in paths {
                        println!("saved {}", p.display());
                    }
                }
                Ok(Event::TransferFailed { direction: Direction::Incoming, reason, .. }) => println!("transfer failed: {reason:?}"),
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

async fn ask(prompt: &str) -> bool {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    tokio::task::spawn_blocking(|| {
        let mut s = String::new();
        let _ = std::io::stdin().read_line(&mut s);
        s.trim().eq_ignore_ascii_case("y")
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn dev(alias: &str, fp: &str) -> Device {
        Device {
            fingerprint: fp.into(), alias: alias.into(), device_model: None, device_type: None,
            addr: SocketAddr::from(([127, 0, 0, 1], 1)), trust: TrustLevel::Known, is_sendit: false,
        }
    }

    #[test]
    fn pick_by_alias_or_prefix() {
        let ds = vec![dev("Pixel 8", "ABCDEF01"), dev("MacBook", "12345678"), dev("pixel 8", "ABCDEF99")];
        assert_eq!(pick(&ds, "macbook").unwrap().fingerprint, "12345678");
        assert_eq!(pick(&ds, "abcdef01").unwrap().alias, "Pixel 8");
        assert!(pick(&ds, "Pixel 8").is_err()); // ambiguous
        assert!(pick(&ds, "nobody").is_err());
        assert!(pick(&ds, "ABC").is_err()); // prefix too short
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sendit-cli`
Expected: 3 passed (1 unit + 2 e2e).

- [ ] **Step 6: Manual smoke test against real LocalSend**

On the same Wi-Fi as a phone running LocalSend: `cargo run -p sendit-cli -- devices`. The phone should be listed. Then `cargo run -p sendit-cli -- send README.md --to "<phone name>"` and accept on the phone.

- [ ] **Step 7: Commit**
```bash
git add Cargo.toml crates/cli
git commit -m "feat(cli): devices, send and receive commands"
```

---

### Task 13: Fuzzing, CI, benchmark, docs

**Files:**
- Create: `fuzz/Cargo.toml`, `fuzz/fuzz_targets/sanitize.rs`, `fuzz/fuzz_targets/prepare_json.rs`
- Create: `.github/workflows/ci.yml`, `.github/workflows/fuzz.yml`
- Create: `crates/core/examples/loopback_bench.rs`
- Create: `docs/interop-checklist.md`, `docs/benchmark.md`, `README.md`, `LICENSE` (Apache-2.0 text from https://www.apache.org/licenses/LICENSE-2.0.txt)

**Interfaces:**
- Consumes: `sendit_core::sanitize::sanitize_relative`, `sendit_core::protocol::PrepareUploadRequest`, the public `Node` API

- [ ] **Step 1: Fuzz targets**

`fuzz/Cargo.toml`:
```toml
[package]
name = "sendit-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
serde_json = "1"
sendit-core = { path = "../crates/core" }

[[bin]]
name = "sanitize"
path = "fuzz_targets/sanitize.rs"
test = false
doc = false
bench = false

[[bin]]
name = "prepare_json"
path = "fuzz_targets/prepare_json.rs"
test = false
doc = false
bench = false
```
`fuzz/fuzz_targets/sanitize.rs`:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path};

fuzz_target!(|s: &str| {
    let p = sendit_core::sanitize::sanitize_relative(s);
    assert!(p.components().all(|c| matches!(c, Component::Normal(_))), "{p:?}");
    assert!(Path::new("/dest").join(&p).starts_with("/dest"));
});
```
`fuzz/fuzz_targets/prepare_json.rs`:
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<sendit_core::protocol::PrepareUploadRequest>(data);
});
```
Run locally (Linux, macOS or WSL): `cargo +nightly fuzz run sanitize -- -max_total_time=60`
Expected: no crash.

- [ ] **Step 2: CI** `.github/workflows/ci.yml`
```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
```
`.github/workflows/fuzz.yml`:
```yaml
name: fuzz
on:
  schedule:
    - cron: "0 3 * * *"
  workflow_dispatch:
jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [sanitize, prepare_json]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz --locked
      - run: cargo fuzz run ${{ matrix.target }} -- -max_total_time=120
```

- [ ] **Step 3: Benchmark** `crates/core/examples/loopback_bench.rs`
```rust
//! cargo run --release -p sendit-core --example loopback_bench -- 1024
use sendit_core::{Config, Decision, Event, Node};
use std::{io::Write, time::Instant};

#[tokio::main]
async fn main() {
    let mb: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1024);
    let root = std::env::temp_dir().join(format!("sendit-bench-{}", std::process::id()));
    let cfg = |name: &str| {
        let mut c = Config::new(name, root.join(name).join("data"), root.join(name).join("down"));
        c.port = 0;
        c.multicast = false;
        c
    };
    let a = Node::start(cfg("a")).await.unwrap();
    let b = Node::start(cfg("b")).await.unwrap();
    let rb = b.clone();
    let mut rx = b.subscribe();
    tokio::spawn(async move {
        while let Ok(e) = rx.recv().await {
            if let Event::IncomingRequest { request } = e {
                let _ = rb.respond(&request.request_id, Decision::Accept { dest: None });
            }
        }
    });
    let file = root.join("payload.bin");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&file).unwrap());
        let chunk = vec![7u8; 1 << 20];
        for _ in 0..mb {
            f.write_all(&chunk).unwrap();
        }
    }
    let dev = a.add_peer(std::net::SocketAddr::from(([127, 0, 0, 1], b.port()))).await.unwrap();
    let t = Instant::now();
    a.send(&dev.fingerprint, vec![file], None).await.unwrap();
    let secs = t.elapsed().as_secs_f64();
    println!("{mb} MB in {secs:.2}s = {:.0} MB/s (loopback, TLS included)", mb as f64 / secs);
    let _ = std::fs::remove_dir_all(&root);
}
```
Run: `cargo run --release -p sendit-core --example loopback_bench -- 512`
Expected: a single MB/s line. Record it in `docs/benchmark.md`.

- [ ] **Step 4: Docs**

`docs/benchmark.md`:
```markdown
# Benchmark method

1. Loopback (CPU/TLS ceiling): `cargo run --release -p sendit-core --example loopback_bench -- 1024`
2. LAN, sendit -> sendit: two machines on the same router (wired if possible).
   Receiver: `sendit receive --yes`. Sender: `sendit send <1 GB file> --to <name>`. Note the final MB/s.
3. LAN, sendit -> LocalSend and LocalSend -> sendit: same file, same machines, the LocalSend app on the other side.
   Time from accept to done with a stopwatch, 3 runs, take the median.

Record: date, OS + version, network (Wi-Fi standard or wired), and median MB/s for each direction.

| Date | Setup | Loopback | sendit->sendit | sendit->LocalSend | LocalSend->sendit |
|---|---|---|---|---|---|
```
`docs/interop-checklist.md`:
```markdown
# Interop checklist (run before every release)

Peers: LocalSend on Android, iOS, and desktop (Windows or macOS). Encryption is ON (the LocalSend default).

For each peer:
- [ ] `sendit devices` lists the peer, and the peer lists sendit
- [ ] sendit -> peer: a single file, accepted on the peer, arrives intact
- [ ] sendit -> peer: a folder with a subfolder keeps its structure
- [ ] sendit -> peer: declined on the peer gives the CLI "Declined"
- [ ] sendit -> peer with a PIN set on the peer: without --pin fails, with --pin works
- [ ] peer -> sendit: a single file and a multi-file batch arrive under the download dir
- [ ] peer -> sendit: decline in sendit is reported as declined on the peer
- [ ] peer -> sendit: cancel on the peer mid-transfer leaves no `.sendit-part` file
- [ ] sendit cancel (Ctrl+C on send) mid-transfer, then the peer accepts a new transfer right away
- [ ] The LocalSend app is open on the same PC as sendit: sendit reports the port fallback and still sends
- [ ] The extra `sendit` field in our JSON does not break the peer
```
`README.md`:
```markdown
# sendit

LocalSend-compatible file transfer for Windows, macOS and Linux. It works with the LocalSend apps on your phone, and it pins each device's key so nobody on the same Wi-Fi can pretend to be your laptop.

## CLI

    sendit devices
    sendit send photo.jpg album/ --to "Pixel 8"
    sendit receive

## Build

    cargo build --release

## Why another LocalSend client?

- A new desktop UI (coming in the app)
- Trust on first use: a device that reuses a known name with a new key gets blocked until you confirm it
- A scriptable CLI

Protocol: https://github.com/localsend/protocol. This project is not affiliated with LocalSend.

License: Apache-2.0
```

- [ ] **Step 5: Run the full suite + lint**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: clean.

- [ ] **Step 6: Commit**
```bash
git add fuzz .github crates/core/examples docs README.md LICENSE
git commit -m "chore: fuzz targets, CI on 3 OSes, benchmark and interop docs"
```

---

## Not in this plan (Plan 2: desktop app)

The Tauri 2 + Svelte app (Ticket v3 UI, DiceBear thumbs avatars, flows A/B/C), its Vitest + Playwright tests, and adding the Tauri build to CI. Plan 2 is written after this plan lands, against the real `Node` API.
