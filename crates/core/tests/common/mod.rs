#![allow(dead_code)]
use sendit_core::{Config, Event, Node};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use tempfile::TempDir;
use tokio::sync::broadcast;

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
    FileMeta {
        id: id.into(),
        file_name: name.into(),
        size,
        file_type: "application/octet-stream".into(),
        sha256,
        preview: None,
        metadata: None,
    }
}

pub async fn prepare(to: &Peer, files: Vec<FileMeta>, pin: Option<&str>) -> reqwest::Response {
    let c = PeerClient::new(Some(to.node.fingerprint())).unwrap();
    let req = PrepareUploadRequest {
        info: fake_info(),
        files: files.into_iter().map(|f| (f.id.clone(), f)).collect(),
    };
    let mut r = c
        .http
        .post(format!("https://{}{API}/prepare-upload", to.addr()))
        .json(&req);
    if let Some(p) = pin {
        r = r.query(&[("pin", p)]);
    }
    r.send().await.unwrap()
}

pub async fn upload(to: &Peer, session: &str, file_id: &str, token: &str, body: Vec<u8>) -> u16 {
    let c = PeerClient::new(Some(to.node.fingerprint())).unwrap();
    c.http
        .post(format!("https://{}{API}/upload", to.addr()))
        .query(&[
            ("sessionId", session),
            ("fileId", file_id),
            ("token", token),
        ])
        .body(body)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

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
    cfg.bind = std::net::Ipv4Addr::LOCALHOST;
    cfg.port = 0;
    cfg.multicast = false;
    cfg.request_timeout = Duration::from_secs(5);
    cfg.idle_timeout = Duration::from_secs(1);
    f(&mut cfg);
    Peer {
        node: Node::start(cfg).await.unwrap(),
        dir,
    }
}

pub async fn wait_for(
    rx: &mut broadcast::Receiver<Event>,
    mut pred: impl FnMut(&Event) -> bool,
) -> Event {
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
                let Ok(outbound) = TcpStream::connect(target).await else {
                    return;
                };
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
                            Chaos::Stall(limit) if total > limit => {
                                std::future::pending::<()>().await
                            }
                            Chaos::Throttle(bps) => {
                                tokio::time::sleep(Duration::from_millis(n as u64 * 1000 / bps))
                                    .await
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
