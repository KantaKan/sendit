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
        a.node
            .send(b.node.fingerprint(), vec![src.clone()], None)
            .await
            .unwrap();
        assert_eq!(sha(&b.down().join(format!("f{i}.bin"))), sha(&src));
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "slow: 1 GB"]
async fn sends_one_gigabyte() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    let src = write_file(a.dir.path(), "big.bin", 1 << 30);
    a.node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap();
    assert_eq!(sha(&b.down().join("big.bin")), sha(&src));
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_folders_with_structure() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    write_file(a.dir.path(), "album/a.jpg", 100);
    write_file(a.dir.path(), "album/sub/b.jpg", 200);
    let notes = write_file(a.dir.path(), "notes.txt", 3);
    a.node
        .send(
            b.node.fingerprint(),
            vec![a.dir.path().join("album"), notes],
            None,
        )
        .await
        .unwrap();
    assert!(b.down().join("album/a.jpg").exists());
    assert!(b.down().join("album/sub/b.jpg").exists());
    assert!(b.down().join("notes.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn decline_reported() {
    let (a, b) = pair().await;
    auto_decline(&b.node);
    let src = write_file(a.dir.path(), "x.txt", 3);
    let err = a
        .node
        .send(b.node.fingerprint(), vec![src], None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Transfer(FailReason::Declined)),
        "{err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pin_required_then_ok() {
    let a = peer("alice").await;
    let b = peer_with("bob", |c| c.pin = Some("4321".into())).await;
    a.node.add_peer(b.addr()).await.unwrap();
    auto_accept(&b.node);
    let src = write_file(a.dir.path(), "x.txt", 3);
    let err = a
        .node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Transfer(FailReason::PinRequired)));
    a.node
        .send(b.node.fingerprint(), vec![src], Some("4321".into()))
        .await
        .unwrap();
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
    let err = a
        .node
        .send(bob2.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Suspect));
    a.node.accept_new_key(bob2.node.fingerprint());
    a.node
        .send(bob2.node.fingerprint(), vec![src], None)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn emits_started_progress_done() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    let mut rx = a.node.subscribe();
    let src = write_file(a.dir.path(), "x.bin", 1024 * 1024);
    let id = a
        .node
        .send(b.node.fingerprint(), vec![src], None)
        .await
        .unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn started_event_carries_sources() {
    let (a, b) = pair().await;
    auto_accept(&b.node);
    let mut rx = a.node.subscribe();
    let src = write_file(a.dir.path(), "s.txt", 3);
    a.node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap();
    let Event::TransferStarted { sources, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::TransferStarted { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(sources, vec![src]);
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_text_messages() {
    let (a, b) = pair().await;
    let mut brx = b.node.subscribe();
    let mut arx = a.node.subscribe();
    let msg = "hello ไทย
line 2";
    a.node.send_text(b.node.fingerprint(), msg).await.unwrap();
    let Event::MessageReceived {
        text,
        sender_verified,
        ..
    } = wait_for(&mut brx, |e| matches!(e, Event::MessageReceived { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(text, msg);
    assert!(sender_verified);
    let Event::MessageSending {
        id: started,
        peer,
        text,
    } = wait_for(&mut arx, |e| matches!(e, Event::MessageSending { .. })).await
    else {
        unreachable!()
    };
    assert_eq!((peer.as_str(), text.as_str()), (b.node.fingerprint(), msg));
    let Event::MessageSent { id } =
        wait_for(&mut arx, |e| matches!(e, Event::MessageSent { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(id, started);
    assert_eq!(std::fs::read_dir(b.down()).unwrap().count(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn text_to_unknown_device_errors() {
    let a = peer("alice").await;
    assert!(matches!(
        a.node.send_text("ABCDEF", "hi").await.unwrap_err(),
        Error::UnknownDevice(_)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_text_reports_why() {
    let a = peer("alice").await;
    let b = peer_with("bob", |c| c.pin = Some("1".into())).await;
    a.node.add_peer(b.addr()).await.unwrap();
    let mut rx = a.node.subscribe();
    assert!(a.node.send_text(b.node.fingerprint(), "hi").await.is_err());
    let Event::MessageFailed { reason, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::MessageFailed { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(reason, FailReason::PinRequired);
}
