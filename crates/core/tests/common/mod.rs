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
