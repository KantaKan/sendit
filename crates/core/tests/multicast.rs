mod common;
use common::*;
use sendit_core::Event;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a multicast-capable network interface"]
async fn peers_find_each_other_by_multicast() {
    let mport = 53399;
    let a = peer_with("alice", |c| {
        c.multicast = true;
        c.multicast_port = mport;
    })
    .await;
    let mut rx = a.node.subscribe();
    let _b = peer_with("bob", |c| {
        c.multicast = true;
        c.multicast_port = mport;
    })
    .await;
    wait_for(
        &mut rx,
        |e| matches!(e, Event::DeviceUpdated { device } if device.alias == "bob"),
    )
    .await;
}
