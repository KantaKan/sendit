use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[test]
fn send_and_receive_over_loopback() {
    let tmp = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let bin = env!("CARGO_BIN_EXE_sendit");
    let mut rx = Command::new(bin)
        .args(["--alias", "rx", "--no-multicast", "--bind", "127.0.0.1"])
        .args(["--port", &port.to_string(), "--data-dir"])
        .arg(tmp.path().join("rx"))
        .args(["receive", "--yes", "--dir"])
        .arg(tmp.path().join("down"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(rx.stdout.take().unwrap()).lines();
    assert!(lines.next().unwrap().unwrap().starts_with("listening"));

    let file = tmp.path().join("hello.txt");
    std::fs::write(&file, "hi from cli").unwrap();
    let out = Command::new(bin)
        .args(["--alias", "tx", "--no-multicast", "--bind", "127.0.0.1"])
        .args(["--port", "0", "--data-dir"])
        .arg(tmp.path().join("tx"))
        .arg("send")
        .arg(&file)
        .args(["--addr", &format!("127.0.0.1:{port}")])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let saved = lines
        .map(|l| l.unwrap())
        .find(|l| l.starts_with("saved"))
        .unwrap();
    assert!(saved.contains("hello.txt"), "{saved}");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("down/hello.txt")).unwrap(),
        "hi from cli"
    );
    rx.kill().unwrap();
    rx.wait().unwrap();
}

#[test]
fn help_lists_commands() {
    let out = Command::new(env!("CARGO_BIN_EXE_sendit"))
        .arg("--help")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    for cmd in ["devices", "send", "receive"] {
        assert!(text.contains(cmd), "{text}");
    }
}
