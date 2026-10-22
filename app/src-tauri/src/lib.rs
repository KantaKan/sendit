//! The desktop shell: one sendit `Node`, exposed to the webview as commands,
//! with every core event forwarded as `core-event`.
use sendit_core::{
    protocol::DEFAULT_PORT, trust::KnownDevice, Config, Decision, Device, Error, Node,
};
use serde::Serialize;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
use tauri::{Emitter, Manager, State};
use tokio::sync::broadcast::error::RecvError;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub alias: String,
    pub fingerprint: String,
    pub port: u16,
    pub port_fallback: bool,
    pub download_dir: PathBuf,
}

pub fn app_info(node: &Node) -> AppInfo {
    AppInfo {
        alias: node.info().alias,
        fingerprint: node.fingerprint().to_string(),
        port: node.port(),
        port_fallback: node.port_fallback(),
        download_dir: node.download_dir().to_path_buf(),
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
            set_verified
        ])
        .run(tauri::generate_context!())
        .expect("error while running sendit");
}

#[cfg(test)]
mod tests {
    use super::*;
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
