//! cargo run --release -p sendit-core --example loopback_bench -- 1024
use sendit_core::{Config, Decision, Event, Node};
use std::{io::Write, net::Ipv4Addr, time::Instant};

#[tokio::main]
async fn main() {
    let mb: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    let root = std::env::temp_dir().join(format!("sendit-bench-{}", std::process::id()));
    let cfg = |name: &str| {
        let mut c = Config::new(
            name,
            root.join(name).join("data"),
            root.join(name).join("down"),
        );
        c.bind = Ipv4Addr::LOCALHOST;
        c.port = 0;
        c.multicast = false;
        c
    };
    let a = Node::start(cfg("a")).await.unwrap();
    let b = Node::start(cfg("b")).await.unwrap();
    let rb = b.clone();
    let mut rx = b.subscribe();
    tokio::spawn(async move {
        while let Ok(e) = rx.recv().await {
            if let Event::IncomingRequest { request } = e {
                let _ = rb.respond(&request.request_id, Decision::Accept { dest: None });
            }
        }
    });
    let file = root.join("payload.bin");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&file).unwrap());
        let chunk = vec![7u8; 1 << 20];
        for _ in 0..mb {
            f.write_all(&chunk).unwrap();
        }
    }
    let dev = a
        .add_peer(std::net::SocketAddr::from(([127, 0, 0, 1], b.port())))
        .await
        .unwrap();
    let t = Instant::now();
    a.send(&dev.fingerprint, vec![file], None).await.unwrap();
    let secs = t.elapsed().as_secs_f64();
    println!(
        "{mb} MB in {secs:.2}s = {:.0} MB/s (loopback, TLS included)",
        mb as f64 / secs
    );
    let _ = std::fs::remove_dir_all(&root);
}
