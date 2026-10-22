mod common;
use common::*;
use sendit_core::{protocol::*, tls::PeerClient, trust::TrustLevel, Event};
use std::time::Duration;

/// C1: an unproven /register claim must not rename or move a pinned device.
#[tokio::test(flavor = "multi_thread")]
async fn forged_register_cannot_rename_or_move_a_known_device() {
    let a = peer("alice").await;
    let bob = peer("bob").await;
    a.node.add_peer(bob.addr()).await.unwrap();
    let mut forged = fake_info();
    forged.fingerprint = bob.node.fingerprint().to_string();
    forged.alias = "x".into();
    let c = PeerClient::new(Some(a.node.fingerprint())).unwrap();
    c.http
        .post(format!("https://{}{API}/register", a.addr()))
        .json(&forged)
        .send()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let d = a.node.device(bob.node.fingerprint()).unwrap();
    assert_eq!(d.alias, "bob");
    assert_eq!(d.addr, bob.addr());
    let fake_bob = peer("bob").await;
    let d2 = a.node.add_peer(fake_bob.addr()).await.unwrap();
    assert_eq!(d2.trust, TrustLevel::Suspect);
}

/// C1: unproven claims are not persisted as devices.
#[tokio::test(flavor = "multi_thread")]
async fn unverifiable_register_is_not_stored() {
    let a = peer("alice").await;
    let c = PeerClient::new(Some(a.node.fingerprint())).unwrap();
    c.http
        .post(format!("https://{}{API}/register", a.addr()))
        .json(&fake_info())
        .send()
        .await
        .unwrap();
    assert!(a.node.devices().is_empty());
}

/// C2: same-named files uploaded in parallel never replace each other.
#[tokio::test(flavor = "multi_thread")]
async fn parallel_same_name_uploads_all_survive() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let n = 24;
    let files = (0..n)
        .map(|i| meta(&i.to_string(), "same.txt", 1, None))
        .collect();
    let r = prepare(&b, files, None).await;
    let r: PrepareUploadResponse = r.json().await.unwrap();
    let uploads = (0..n).map(|i| {
        let id = i.to_string();
        let token = r.files[&id].clone();
        let sid = r.session_id.clone();
        let b = &b;
        async move { upload(b, &sid, &id, &token, vec![b'a']).await }
    });
    let codes = futures::future::join_all(uploads).await;
    assert!(codes.iter().all(|c| *c == 200), "{codes:?}");
    let count = std::fs::read_dir(b.down()).unwrap().count();
    assert_eq!(count, n);
}

/// I1: a link inside the download dir must not lead writes outside it.
#[tokio::test(flavor = "multi_thread")]
async fn symlinked_folder_cannot_escape() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let outside = b.dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&outside, b.down().join("link")).is_ok();
    // Junctions need no admin rights, unlike symlinks.
    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(b.down().join("link"))
        .arg(&outside)
        .output()
        .is_ok_and(|o| o.status.success());
    if !made {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let r = prepare(&b, vec![meta("1", "link/evil.txt", 1, None)], None).await;
    let r: PrepareUploadResponse = r.json().await.unwrap();
    let code = upload(&b, &r.session_id, "1", &r.files["1"], b"x".to_vec()).await;
    assert_ne!(code, 200);
    assert!(!outside.join("evil.txt").exists());
}

/// I2: while one request waits for the user, others get 409.
#[tokio::test(flavor = "multi_thread")]
async fn second_request_while_one_is_pending_is_409() {
    let b = peer("bob").await; // nobody answers; request_timeout = 5s
    let mut rx = b.node.subscribe();
    let first = {
        let fp = b.node.fingerprint().to_string();
        let addr = b.addr();
        tokio::spawn(async move {
            let c = PeerClient::new(Some(&fp)).unwrap();
            let req = PrepareUploadRequest {
                info: fake_info(),
                files: [("1".to_string(), meta("1", "a.txt", 1, None))].into(),
            };
            c.http
                .post(format!("https://{addr}{API}/prepare-upload"))
                .json(&req)
                .send()
                .await
        })
    };
    wait_for(&mut rx, |e| matches!(e, Event::IncomingRequest { .. })).await;
    assert_eq!(
        prepare(&b, vec![meta("2", "b.txt", 1, None)], None)
            .await
            .status(),
        409
    );
    first.abort();
}

/// I2: PIN guessing is rate limited.
#[tokio::test(flavor = "multi_thread")]
async fn pin_guessing_gets_429() {
    let b = peer_with("bob", |c| c.pin = Some("1234".into())).await;
    let f = || vec![meta("1", "a.txt", 1, None)];
    for _ in 0..5 {
        assert_eq!(prepare(&b, f(), Some("0000")).await.status(), 401);
    }
    assert_eq!(prepare(&b, f(), Some("0000")).await.status(), 429);
    assert_eq!(prepare(&b, f(), Some("1234")).await.status(), 429);
}

/// I4: LocalSend's add-by-IP / scan uses GET /info.
#[tokio::test(flavor = "multi_thread")]
async fn info_endpoint_returns_our_identity() {
    let b = peer("bob").await;
    let c = PeerClient::new(Some(b.node.fingerprint())).unwrap();
    let info: DeviceInfo = c
        .http
        .get(format!("https://{}{API}/info", b.addr()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info.alias, "bob");
    assert_eq!(info.fingerprint, b.node.fingerprint());
}
