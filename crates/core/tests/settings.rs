mod common;
use common::*;
use sendit_core::{Error, Settings};

#[tokio::test(flavor = "multi_thread")]
async fn empty_name_is_rejected_and_nothing_changes() {
    let a = peer("alice").await;
    let r = a.node.update_settings(Settings {
        alias: "   ".into(),
        ..a.node.settings()
    });
    assert!(matches!(r, Err(Error::Invalid(_))));
    assert_eq!(a.node.settings().alias, "alice");
}

#[tokio::test(flavor = "multi_thread")]
async fn names_are_trimmed_and_capped() {
    let a = peer("alice").await;
    let s = a.node.update_settings(Settings {
        alias: format!("  {}  ", "x".repeat(100)),
        ..a.node.settings()
    });
    assert_eq!(s.unwrap().alias, "x".repeat(64));
    assert_eq!(a.node.info().alias, "x".repeat(64));
}

#[tokio::test(flavor = "multi_thread")]
async fn new_name_is_what_peers_see() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    a.node
        .update_settings(Settings {
            alias: "alice 2".into(),
            ..a.node.settings()
        })
        .unwrap();
    assert_eq!(b.node.add_peer(a.addr()).await.unwrap().alias, "alice 2");
}

#[tokio::test(flavor = "multi_thread")]
async fn new_pin_applies_to_the_next_request() {
    let b = peer("bob").await;
    auto_decline(&b.node);
    let f = || vec![meta("1", "a.txt", 1, None)];
    assert_eq!(prepare(&b, f(), None).await.status(), 403);
    b.node
        .update_settings(Settings {
            pin: Some(" 1234 ".into()),
            ..b.node.settings()
        })
        .unwrap();
    assert_eq!(b.node.settings().pin.as_deref(), Some("1234"));
    assert_eq!(prepare(&b, f(), None).await.status(), 401);
    assert_eq!(prepare(&b, f(), Some("1234")).await.status(), 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn blank_pin_means_no_pin() {
    let b = peer_with("bob", |c| c.pin = Some("1234".into())).await;
    let s = b.node.update_settings(Settings {
        pin: Some("  ".into()),
        ..b.node.settings()
    });
    assert_eq!(s.unwrap().pin, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_the_pin_clears_lockouts() {
    let b = peer_with("bob", |c| c.pin = Some("1234".into())).await;
    auto_decline(&b.node);
    let f = || vec![meta("1", "a.txt", 1, None)];
    for _ in 0..5 {
        assert_eq!(prepare(&b, f(), Some("0000")).await.status(), 401);
    }
    assert_eq!(prepare(&b, f(), Some("1234")).await.status(), 429);
    b.node
        .update_settings(Settings {
            pin: Some("9876".into()),
            ..b.node.settings()
        })
        .unwrap();
    assert_eq!(prepare(&b, f(), Some("9876")).await.status(), 403);
}

#[tokio::test(flavor = "multi_thread")]
async fn new_download_folder_applies_to_the_next_transfer() {
    let a = peer("alice").await;
    let b = peer("bob").await;
    a.node.add_peer(b.addr()).await.unwrap();
    auto_accept(&b.node);
    let elsewhere = b.dir.path().join("elsewhere");
    b.node
        .update_settings(Settings {
            download_dir: elsewhere.clone(),
            ..b.node.settings()
        })
        .unwrap();
    assert!(elsewhere.is_dir());
    assert_eq!(b.node.download_dir(), elsewhere);
    let src = write_file(a.dir.path(), "x.bin", 10);
    a.node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap();
    assert_eq!(sha(&elsewhere.join("x.bin")), sha(&src));
    assert!(!b.down().join("x.bin").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn unusable_folder_is_rejected_and_nothing_changes() {
    let a = peer("alice").await;
    let file = write_file(a.dir.path(), "plain-file", 1);
    let r = a.node.update_settings(Settings {
        download_dir: file.join("sub"),
        ..a.node.settings()
    });
    assert!(matches!(r, Err(Error::Invalid(_))));
    assert!(a.node.download_dir().ends_with("down"));
}
