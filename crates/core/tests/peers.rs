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
    assert!(b
        .node
        .devices()
        .iter()
        .any(|d| d.fingerprint == a.node.fingerprint()));
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
    let Event::IdentityChanged {
        old_fingerprint, ..
    } = wait_for(&mut rx, |e| matches!(e, Event::IdentityChanged { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(old_fingerprint, bob1.node.fingerprint());
    a.node.accept_new_key(bob2.node.fingerprint());
    assert_eq!(
        a.node.device(bob2.node.fingerprint()).unwrap().trust,
        TrustLevel::Known
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn busy_port_falls_back() {
    let blocker = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = blocker.local_addr().unwrap().port();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = Config::new("x", dir.path().join("d"), dir.path().join("down"));
    cfg.bind = std::net::Ipv4Addr::LOCALHOST;
    cfg.port = taken;
    cfg.multicast = false;
    let node = Node::start(cfg).await.unwrap();
    assert!(node.port_fallback());
    assert_ne!(node.port(), taken);
}
