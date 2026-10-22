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
    assert_eq!(
        upload(&b, &r.session_id, "1", &r.files["1"], b"hello".to_vec()).await,
        200
    );
    let Event::TransferDone { paths, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::TransferDone { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(paths, vec![b.down().join("hello.txt")]);
    assert_eq!(std::fs::read(b.down().join("hello.txt")).unwrap(), b"hello");
}

#[tokio::test(flavor = "multi_thread")]
async fn hostile_names_stay_inside_download_dir() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let names = ["../../escape.txt", "..\\..\\win.txt", "/abs/x.txt"];
    let files = names
        .iter()
        .enumerate()
        .map(|(i, n)| meta(&i.to_string(), n, 1, None))
        .collect();
    let r = accepted(&b, files).await;
    for i in 0..names.len() {
        let id = i.to_string();
        assert_eq!(
            upload(&b, &r.session_id, &id, &r.files[&id], b"x".to_vec()).await,
            200
        );
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
    let r = accepted(
        &b,
        vec![
            meta("1", "same.txt", 1, None),
            meta("2", "same.txt", 1, None),
        ],
    )
    .await;
    assert_eq!(
        upload(&b, &r.session_id, "1", &r.files["1"], b"a".to_vec()).await,
        200
    );
    assert_eq!(
        upload(&b, &r.session_id, "2", &r.files["2"], b"b".to_vec()).await,
        200
    );
    assert_eq!(
        std::fs::read_to_string(b.down().join("same.txt")).unwrap(),
        "old"
    );
    assert!(b.down().join("same (1).txt").exists());
    assert!(b.down().join("same (2).txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn checksum_mismatch_is_422_and_leaves_nothing() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "c.txt", 5, Some("00".repeat(32)))]).await;
    assert_eq!(
        upload(&b, &r.session_id, "1", &r.files["1"], b"hello".to_vec()).await,
        422
    );
    assert!(!b.down().join("c.txt").exists());
    assert!(no_part_files(&b.down()));
}

#[tokio::test(flavor = "multi_thread")]
async fn more_bytes_than_declared_is_400() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "big.txt", 10, None)]).await;
    assert_eq!(
        upload(&b, &r.session_id, "1", &r.files["1"], vec![0; 20]).await,
        400
    );
    assert!(!b.down().join("big.txt").exists());
    assert!(no_part_files(&b.down()));
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_token_is_403() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let r = accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    assert_eq!(
        upload(&b, &r.session_id, "1", "nope", b"a".to_vec()).await,
        403
    );
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
    assert_eq!(
        prepare(&b, vec![meta("2", "b.txt", 1, None)], None)
            .await
            .status(),
        409
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn vanished_sender_frees_the_session() {
    let b = peer("bob").await; // idle_timeout = 1s
    auto_accept(&b.node);
    let mut rx = b.node.subscribe();
    accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    let Event::TransferFailed { reason, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::TransferFailed { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(reason, FailReason::PeerGone);
    assert_eq!(
        prepare(&b, vec![meta("2", "b.txt", 1, None)], None)
            .await
            .status(),
        200
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_request_is_204_and_blocked_is_403() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    assert_eq!(prepare(&b, vec![], None).await.status(), 204);
    b.node.block(&"F".repeat(64));
    assert_eq!(
        prepare(&b, vec![meta("1", "a.txt", 1, None)], None)
            .await
            .status(),
        403
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn decline_is_403() {
    let b = peer("bob").await;
    auto_decline(&b.node);
    assert_eq!(
        prepare(&b, vec![meta("1", "a.txt", 1, None)], None)
            .await
            .status(),
        403
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_sender_is_flagged() {
    let b = peer("bob").await;
    let mut rx = b.node.subscribe();
    let bb = b.node.clone();
    let task = tokio::spawn(async move {
        let Event::IncomingRequest { request } =
            wait_for(&mut rx, |e| matches!(e, Event::IncomingRequest { .. })).await
        else {
            unreachable!()
        };
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
    let c = PeerClient::new(
        &sendit_core::identity::Identity::generate().unwrap(),
        Some(b.node.fingerprint()),
    )
    .unwrap();
    let s = c
        .http
        .post(format!("https://{}{API}/cancel", b.addr()))
        .query(&[("sessionId", &r.session_id)])
        .send()
        .await
        .unwrap();
    assert_eq!(s.status(), 200);
    let Event::TransferFailed { reason, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::TransferFailed { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(reason, FailReason::Cancelled);
}

/// A sender that hangs up while we wait for the user must not leave the
/// request (or the receiver's "busy" state) behind.
#[tokio::test(flavor = "multi_thread")]
async fn abandoned_request_expires_and_frees_the_slot() {
    let b = peer("bob").await; // nobody answers; request_timeout = 5s
    let mut rx = b.node.subscribe();
    let c = PeerClient::new(
        &sendit_core::identity::Identity::generate().unwrap(),
        Some(b.node.fingerprint()),
    )
    .unwrap();
    let req = PrepareUploadRequest {
        info: fake_info(),
        files: [("1".to_string(), meta("1", "a.txt", 1, None))].into(),
    };
    let url = format!("https://{}{API}/prepare-upload", b.addr());
    let sender = tokio::spawn(async move { c.http.post(url).json(&req).send().await });
    let Event::IncomingRequest { request } =
        wait_for(&mut rx, |e| matches!(e, Event::IncomingRequest { .. })).await
    else {
        unreachable!()
    };
    sender.abort(); // the sender hangs up while we wait for the user
    let Event::RequestExpired { request_id } =
        wait_for(&mut rx, |e| matches!(e, Event::RequestExpired { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(request_id, request.request_id);
    assert!(b
        .node
        .respond(&request_id, sendit_core::Decision::Accept { dest: None })
        .is_err());
    assert_eq!(prepare(&b, vec![], None).await.status(), 204);
}

/// The incoming transfer names the request it came from, so a UI can match them.
#[tokio::test(flavor = "multi_thread")]
async fn accepted_transfer_names_its_request() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    let mut rx = b.node.subscribe();
    accepted(&b, vec![meta("1", "a.txt", 1, None)]).await;
    let Event::IncomingRequest { request } =
        wait_for(&mut rx, |e| matches!(e, Event::IncomingRequest { .. })).await
    else {
        unreachable!()
    };
    let Event::TransferStarted { request_id, .. } =
        wait_for(&mut rx, |e| matches!(e, Event::TransferStarted { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(request_id, Some(request.request_id));
}

fn text_meta(text: Option<&str>) -> FileMeta {
    let mut m = meta("1", "note.txt", 5, None);
    m.file_type = "text/plain".into();
    m.preview = text.map(str::to_string);
    m
}

/// LocalSend sends a text message as one text file whose content is in
/// `preview`, and expects 204 ("no transfer needed") instead of an upload.
#[tokio::test(flavor = "multi_thread")]
async fn text_message_is_delivered_without_upload() {
    let b = peer("bob").await;
    let mut rx = b.node.subscribe();
    assert_eq!(
        prepare(
            &b,
            vec![text_meta(Some(
                "hello
world"
            ))],
            None
        )
        .await
        .status(),
        204
    );
    let Event::MessageReceived {
        from,
        text,
        sender_verified,
    } = wait_for(&mut rx, |e| matches!(e, Event::MessageReceived { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(
        text,
        "hello
world"
    );
    assert_eq!(from.alias, "mallory");
    assert!(!sender_verified);
    assert_eq!(std::fs::read_dir(b.down()).unwrap().count(), 0);
}

/// A real .txt file (no preview) is still a file transfer.
#[tokio::test(flavor = "multi_thread")]
async fn text_file_without_preview_is_still_a_file() {
    let b = peer("bob").await;
    auto_accept(&b.node);
    assert_eq!(prepare(&b, vec![text_meta(None)], None).await.status(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn blocked_sender_cannot_message() {
    let b = peer("bob").await;
    b.node.block(&"F".repeat(64));
    assert_eq!(
        prepare(&b, vec![text_meta(Some("hi"))], None)
            .await
            .status(),
        403
    );
}
