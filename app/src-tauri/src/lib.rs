//! The desktop shell: one sendit `Node`, exposed to the webview as commands,
//! with every core event forwarded as `core-event`.
use clipboard_rs::{common::RustImage, Clipboard, ClipboardContext, ContentFormat};
use sendit_core::{
    protocol::DEFAULT_PORT, trust::KnownDevice, Config, Decision, Device, Error, Node,
};
use serde::Serialize;
use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast::error::RecvError;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub alias: String,
    pub fingerprint: String,
    pub port: u16,
    pub port_fallback: bool,
    pub download_dir: PathBuf,
    /// LAN IPv4 addresses; the UI shows the most likely one.
    pub addresses: Vec<String>,
}

pub fn app_info(node: &Node) -> AppInfo {
    AppInfo {
        alias: node.info().alias,
        fingerprint: node.fingerprint().to_string(),
        port: node.port(),
        port_fallback: node.port_fallback(),
        download_dir: node.download_dir().to_path_buf(),
        addresses: node.local_ips().iter().map(|ip| ip.to_string()).collect(),
    }
}

/// "192.168.1.5" or "192.168.1.5:53317".
pub fn parse_addr(s: &str) -> Result<SocketAddr, String> {
    let s = s.trim();
    s.parse::<SocketAddr>()
        .or_else(|_| {
            s.parse::<IpAddr>()
                .map(|ip| SocketAddr::new(ip, DEFAULT_PORT))
        })
        .map_err(|_| format!("{s:?} is not an IP address"))
}

/// Transfer failures already reach the UI as events; only setup errors
/// (unknown device, changed key, unreadable file) come back here.
pub async fn send_files_impl(
    node: &Node,
    fingerprint: &str,
    paths: Vec<PathBuf>,
) -> Result<(), String> {
    match node.send(fingerprint, paths, None).await {
        Ok(_) | Err(Error::Transfer(_)) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Like `send_files_impl`: delivery failures arrive as `MessageFailed` events.
pub async fn send_text_impl(node: &Node, fingerprint: &str, text: &str) -> Result<(), String> {
    match node.send_text(fingerprint, text).await {
        Ok(()) | Err(Error::Transfer(_)) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn send_text(node: State<'_, Node>, fingerprint: String, text: String) -> Result<(), String> {
    send_text_impl(&node, &fingerprint, &text).await
}

#[tauri::command]
fn info(node: State<'_, Node>) -> AppInfo {
    app_info(&node)
}

#[tauri::command]
fn devices(node: State<'_, Node>) -> Vec<Device> {
    node.devices()
}

#[tauri::command]
fn known_devices(node: State<'_, Node>) -> Vec<KnownDevice> {
    node.known_devices()
}

#[tauri::command]
async fn send_files(
    node: State<'_, Node>,
    fingerprint: String,
    paths: Vec<PathBuf>,
) -> Result<(), String> {
    send_files_impl(&node, &fingerprint, paths).await
}

#[tauri::command]
fn respond(
    node: State<'_, Node>,
    request_id: String,
    accept: bool,
    dest: Option<PathBuf>,
) -> Result<(), String> {
    let decision = if accept {
        Decision::Accept { dest }
    } else {
        Decision::Decline
    };
    node.respond(&request_id, decision)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn cancel(node: State<'_, Node>, transfer_id: String) -> bool {
    node.cancel(&transfer_id)
}

#[tauri::command]
fn announce(node: State<'_, Node>) {
    node.announce();
}

#[tauri::command]
async fn scan(node: State<'_, Node>) -> Result<Vec<Device>, String> {
    Ok(node.scan().await)
}

#[tauri::command]
async fn add_peer(node: State<'_, Node>, addr: String) -> Result<Device, String> {
    let addr = parse_addr(&addr)?;
    node.add_peer(addr).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn trust_new_key(node: State<'_, Node>, fingerprint: String) {
    node.accept_new_key(&fingerprint);
}

#[tauri::command]
fn block(node: State<'_, Node>, fingerprint: String) {
    node.block(&fingerprint);
}

#[tauri::command]
fn set_verified(node: State<'_, Node>, fingerprint: String, verified: bool) {
    node.set_verified(&fingerprint, verified);
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let down = app
                .path()
                .download_dir()
                .map(|d| d.join("sendit"))
                .unwrap_or_else(|_| data.join("downloads"));
            let alias = gethostname::gethostname().to_string_lossy().into_owned();
            let node = tauri::async_runtime::block_on(Node::start(Config::new(alias, data, down)))?;
            let handle = app.handle().clone();
            let mut rx = node.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(e) => {
                            let _ = handle.emit("core-event", e);
                        }
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => break,
                    }
                }
            });
            app.manage(node);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            info,
            devices,
            known_devices,
            send_files,
            respond,
            cancel,
            announce,
            scan,
            add_peer,
            trust_new_key,
            block,
            set_verified,
            paste,
            send_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running sendit");
}

/// What the clipboard held, checked in this order.
pub enum Clip {
    Files(Vec<PathBuf>),
    Image(PathBuf),
    Text(String),
    Empty,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pasted {
    Files { count: usize, name: String },
    Image { name: String },
    Text { preview: String },
    Empty,
}

/// A short summary for the "Pasted ... to X" toast.
pub fn describe(c: &Clip) -> Pasted {
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match c {
        Clip::Files(paths) => Pasted::Files {
            count: paths.len(),
            name: paths.first().map(|p| name(p)).unwrap_or_default(),
        },
        Clip::Image(p) => Pasted::Image { name: name(p) },
        Clip::Text(t) => {
            let flat: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
            let preview = if flat.chars().count() > 60 {
                flat.chars().take(60).chain(['…']).collect()
            } else {
                flat
            };
            Pasted::Text { preview }
        }
        Clip::Empty => Pasted::Empty,
    }
}

/// Clipboard file lists are plain paths on Windows and file:// URLs elsewhere.
pub fn file_path(s: &str) -> PathBuf {
    let Some(rest) = s.strip_prefix("file://") else {
        return PathBuf::from(s);
    };
    let mut bytes = Vec::with_capacity(rest.len());
    let raw = rest.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        let hex = raw
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (raw[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(b)) => {
                bytes.push(b);
                i += 3;
            }
            (c, _) => {
                bytes.push(c);
                i += 1;
            }
        }
    }
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

/// Reads the OS clipboard: copied files first, then an image (saved as PNG in
/// `tmp`), then text.
// ponytail: pasted screenshots pile up in the app cache dir; clean on startup if it grows.
fn read_clipboard(tmp: &Path) -> Clip {
    let Ok(ctx) = ClipboardContext::new() else {
        return Clip::Empty;
    };
    if let Ok(files) = ctx.get_files() {
        let paths: Vec<PathBuf> = files.iter().map(|f| file_path(f)).collect();
        if !paths.is_empty() {
            return Clip::Files(paths);
        }
    }
    if ctx.has(ContentFormat::Image) {
        if let Ok(img) = ctx.get_image() {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            let path = tmp.join(format!("screenshot-{stamp}.png"));
            if std::fs::create_dir_all(tmp).is_ok()
                && img.save_to_path(&path.to_string_lossy()).is_ok()
            {
                return Clip::Image(path);
            }
        }
    }
    match ctx.get_text() {
        Ok(t) if !t.trim().is_empty() => Clip::Text(t),
        _ => Clip::Empty,
    }
}

/// Ctrl/Cmd+V: send whatever is on the clipboard to `fingerprint`. Returns
/// at once with a summary; failures arrive later as an `app-error` event.
#[tauri::command]
async fn paste(
    app: AppHandle,
    node: State<'_, Node>,
    fingerprint: String,
) -> Result<Pasted, String> {
    let tmp = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("clipboard");
    let clip = read_clipboard(&tmp);
    let pasted = describe(&clip);
    let node = node.inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = match clip {
            Clip::Files(paths) => send_files_impl(&node, &fingerprint, paths).await,
            Clip::Image(path) => send_files_impl(&node, &fingerprint, vec![path]).await,
            Clip::Text(text) => send_text_impl(&node, &fingerprint, &text).await,
            Clip::Empty => Ok(()),
        };
        if let Err(e) = result {
            let _ = app.emit("app-error", e);
        }
    });
    Ok(pasted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_what_was_pasted() {
        let files = Clip::Files(vec!["C:/a/report.pdf".into(), "C:/a/b.png".into()]);
        assert_eq!(
            describe(&files),
            Pasted::Files {
                count: 2,
                name: "report.pdf".into()
            }
        );
        let img = Clip::Image("C:/tmp/screenshot-1.png".into());
        assert_eq!(
            describe(&img),
            Pasted::Image {
                name: "screenshot-1.png".into()
            }
        );
        let long = "x".repeat(200);
        let Pasted::Text { preview } = describe(&Clip::Text(long)) else {
            panic!()
        };
        assert_eq!(preview.chars().count(), 61);
        assert!(preview.ends_with('…'));
        assert_eq!(
            describe(&Clip::Text(
                "hi
there"
                    .into()
            )),
            Pasted::Text {
                preview: "hi there".into()
            }
        );
        assert_eq!(describe(&Clip::Empty), Pasted::Empty);
    }

    #[test]
    fn file_urls_become_paths() {
        assert_eq!(file_path(r"C:\x\a b.txt"), PathBuf::from(r"C:\x\a b.txt"));
        assert_eq!(
            file_path("file:///home/me/a%20b.txt"),
            PathBuf::from("/home/me/a b.txt")
        );
        assert_eq!(
            file_path("file:///Users/me/%E0%B8%81.txt"),
            PathBuf::from("/Users/me/ก.txt")
        );
    }
    use sendit_core::{Config, Decision, Event, Node};
    use std::{net::Ipv4Addr, path::Path, time::Duration};

    async fn node(name: &str, dir: &Path) -> Node {
        let mut c = Config::new(
            name,
            dir.join(name).join("data"),
            dir.join(name).join("down"),
        );
        c.bind = Ipv4Addr::LOCALHOST;
        c.port = 0;
        c.multicast = false;
        c.request_timeout = Duration::from_secs(5);
        Node::start(c).await.unwrap()
    }

    #[test]
    fn parse_addr_defaults_the_port() {
        assert_eq!(
            parse_addr("192.168.1.5").unwrap(),
            "192.168.1.5:53317".parse().unwrap()
        );
        assert_eq!(
            parse_addr(" 10.0.0.2:4000 ").unwrap(),
            "10.0.0.2:4000".parse().unwrap()
        );
        assert!(parse_addr("nope").is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn app_info_reports_port_and_dir() {
        let d = tempfile::tempdir().unwrap();
        let a = node("a", d.path()).await;
        let i = app_info(&a);
        assert_eq!(i.port, a.port());
        assert!(!i.port_fallback);
        assert!(i.download_dir.ends_with("down"));
        assert_eq!(i.fingerprint, a.fingerprint());
        assert!(i.addresses.iter().all(|x| x.parse::<IpAddr>().is_ok()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn text_to_unknown_device_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let a = node("a", d.path()).await;
        assert!(send_text_impl(&a, "ABCDEF", "hi").await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_to_unknown_device_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let a = node("a", d.path()).await;
        assert!(send_files_impl(&a, "ABCDEF", vec![]).await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn transfer_failures_are_left_to_events() {
        let d = tempfile::tempdir().unwrap();
        let a = node("a", d.path()).await;
        let b = node("b", d.path()).await;
        let (bb, mut rx) = (b.clone(), b.subscribe());
        tokio::spawn(async move {
            while let Ok(e) = rx.recv().await {
                if let Event::IncomingRequest { request } = e {
                    let _ = bb.respond(&request.request_id, Decision::Decline);
                }
            }
        });
        let dev = a
            .add_peer(SocketAddr::from(([127, 0, 0, 1], b.port())))
            .await
            .unwrap();
        let f = d.path().join("x.txt");
        std::fs::write(&f, "x").unwrap();
        assert_eq!(send_files_impl(&a, &dev.fingerprint, vec![f]).await, Ok(()));
    }
}
