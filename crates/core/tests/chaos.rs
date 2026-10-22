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
    let err = a
        .node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Transfer(FailReason::PeerGone)),
        "{err:?}"
    );
    wait_for(&mut brx, |e| matches!(e, Event::TransferFailed { .. })).await;
    let down = b.down();
    eventually(|| no_part_files(&down)).await;
    assert!(!b.down().join("x.bin").exists());
    // Retry over a healthy path.
    a.node.add_peer(b.addr()).await.unwrap();
    a.node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap();
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
    a.node
        .send(b.node.fingerprint(), vec![src.clone()], None)
        .await
        .unwrap();
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
    let err = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        a.node.send(b.node.fingerprint(), vec![src], None),
    )
    .await
    .expect("sender hung on a stalled link")
    .unwrap_err();
    assert!(
        matches!(err, Error::Transfer(FailReason::PeerGone)),
        "{err:?}"
    );
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
    let Event::Progress { transfer_id, .. } =
        wait_for(&mut arx, |e| matches!(e, Event::Progress { .. })).await
    else {
        unreachable!()
    };
    assert!(a.node.cancel(&transfer_id));
    let err = sender.await.unwrap().unwrap_err();
    assert!(
        matches!(err, Error::Transfer(FailReason::Cancelled)),
        "{err:?}"
    );
    wait_for(&mut brx, |e| matches!(e, Event::TransferFailed { .. })).await;
    let down = b.down();
    eventually(|| no_part_files(&down)).await;
}

/// When the receiver's watchdog fires first, the sender must still see
/// "connection lost", not "cancelled".
#[tokio::test(flavor = "multi_thread")]
async fn receiver_timeout_reads_as_peer_gone_for_sender() {
    let a = peer_with("alice", |c| {
        c.idle_timeout = std::time::Duration::from_secs(10)
    })
    .await;
    let b = peer("bob").await; // idle_timeout = 1s, fires first
    auto_accept(&b.node);
    let via = proxy(b.addr(), Chaos::Stall(MB as u64)).await;
    a.node.add_peer(via).await.unwrap();
    let src = write_file(a.dir.path(), "stall2.bin", 8 * MB);
    let err = a
        .node
        .send(b.node.fingerprint(), vec![src], None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Transfer(FailReason::PeerGone)),
        "{err:?}"
    );
}
