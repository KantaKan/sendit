use clap::{Parser, Subcommand};
use sendit_core::{
    protocol::DEFAULT_PORT, trust::TrustLevel, Config, Decision, Device, Direction, Event, Node,
};
use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::broadcast::error::RecvError;

type BoxResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(name = "sendit", version, about = "LocalSend-compatible file transfer")]
struct Cli {
    /// Name other devices see (default: hostname)
    #[arg(long, global = true)]
    alias: Option<String>,
    #[arg(long, global = true, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// Interface to listen on
    #[arg(long, global = true, default_value_t = Ipv4Addr::UNSPECIFIED, hide = true)]
    bind: Ipv4Addr,
    /// Where identity and trust data are stored
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Skip multicast discovery (use --addr)
    #[arg(long, global = true)]
    no_multicast: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List devices on the network
    Devices {
        #[arg(long, default_value_t = 3)]
        wait: u64,
        /// Also probe every address on the subnet (slow, for networks that block multicast)
        #[arg(long)]
        scan: bool,
    },
    /// Send files or folders
    Send {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Device name or fingerprint prefix
        #[arg(long)]
        to: Option<String>,
        /// Send to this ip:port directly
        #[arg(long)]
        addr: Option<SocketAddr>,
        #[arg(long, default_value_t = 3)]
        wait: u64,
        #[arg(long)]
        scan: bool,
        #[arg(long)]
        pin: Option<String>,
        /// Accept a changed key for this device (e.g. after a reinstall)
        #[arg(long)]
        trust_new_key: bool,
    },
    /// Wait for incoming files
    Receive {
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Accept everything without asking
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        pin: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> BoxResult<()> {
    let data_dir = match &cli.data_dir {
        Some(d) => d.clone(),
        None => directories::ProjectDirs::from("", "", "sendit")
            .map(|d| d.data_dir().to_path_buf())
            .ok_or("cannot find a data directory; pass --data-dir")?,
    };
    let download_dir = match &cli.cmd {
        Cmd::Receive { dir: Some(d), .. } => d.clone(),
        _ => directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(|d| d.join("sendit")))
            .unwrap_or_else(|| data_dir.join("downloads")),
    };
    let alias = cli
        .alias
        .clone()
        .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned());
    let mut cfg = Config::new(alias, data_dir, download_dir);
    cfg.bind = cli.bind;
    cfg.port = cli.port;
    cfg.multicast = !cli.no_multicast;
    if let Cmd::Receive { pin, .. } = &cli.cmd {
        cfg.pin = pin.clone();
    }
    let node = Node::start(cfg).await?;
    if node.port_fallback() {
        eprintln!(
            "port {} is busy (is LocalSend running?), using {}",
            cli.port,
            node.port()
        );
    }
    match cli.cmd {
        Cmd::Devices { wait, scan } => {
            discover(&node, wait, scan).await;
            print_devices(&node.devices());
        }
        Cmd::Send {
            paths,
            to,
            addr,
            wait,
            scan,
            pin,
            trust_new_key,
        } => {
            let device = match addr {
                Some(a) => node.add_peer(a).await?,
                None => {
                    let to = to.ok_or("pass --to <name> or --addr <ip:port>")?;
                    discover(&node, wait, scan).await;
                    pick(&node.devices(), &to)?
                }
            };
            send(&node, device, paths, pin, trust_new_key).await?;
        }
        Cmd::Receive { yes, .. } => receive(&node, yes).await?,
    }
    Ok(())
}

async fn discover(node: &Node, wait: u64, scan: bool) {
    if scan {
        node.scan().await;
    }
    tokio::time::sleep(Duration::from_secs(wait)).await;
}

fn print_devices(devices: &[Device]) {
    if devices.is_empty() {
        println!("no devices found. Same Wi-Fi? Try --scan");
        return;
    }
    for d in devices {
        println!(
            "{:<24} {:<9} {:<22} {:?}",
            clean(&d.alias),
            &d.fingerprint[..8.min(d.fingerprint.len())],
            d.addr,
            d.trust
        );
    }
}

fn pick(devices: &[Device], to: &str) -> BoxResult<Device> {
    let upper = to.to_uppercase();
    let matches: Vec<&Device> = devices
        .iter()
        .filter(|d| {
            d.alias.eq_ignore_ascii_case(to) || (to.len() >= 6 && d.fingerprint.starts_with(&upper))
        })
        .collect();
    match matches.as_slice() {
        [d] => Ok((*d).clone()),
        [] => Err(format!("no device named {to:?}. Run `sendit devices`").into()),
        _ => Err(format!(
            "{to:?} matches {} devices; use a fingerprint prefix",
            matches.len()
        )
        .into()),
    }
}

async fn send(
    node: &Node,
    device: Device,
    paths: Vec<PathBuf>,
    pin: Option<String>,
    trust_new_key: bool,
) -> BoxResult<()> {
    if device.trust == TrustLevel::Suspect {
        if !trust_new_key {
            return Err(format!(
                "{} is using a different key than before. If it was reinstalled, rerun with --trust-new-key",
                clean(&device.alias)
            )
            .into());
        }
        node.accept_new_key(&device.fingerprint);
    }
    let mut rx = node.subscribe();
    let progress = tokio::spawn(async move {
        let mut first: Option<Instant> = None;
        while let Ok(e) = rx.recv().await {
            if let Event::Progress {
                bytes,
                total,
                direction: Direction::Outgoing,
                ..
            } = e
            {
                let start = *first.get_or_insert_with(Instant::now);
                let secs = start.elapsed().as_secs_f64().max(0.001);
                let pct = if total == 0 {
                    100.0
                } else {
                    bytes as f64 * 100.0 / total as f64
                };
                eprint!("\r{pct:>5.1}%  {:>8.1} MB/s", bytes as f64 / 1e6 / secs);
                let _ = std::io::stderr().flush();
            }
        }
    });
    eprintln!("waiting for {} to accept...", clean(&device.alias));
    let result = node.send(&device.fingerprint, paths, pin).await;
    progress.abort();
    eprintln!();
    result?;
    println!("sent to {}", clean(&device.alias));
    Ok(())
}

async fn receive(node: &Node, yes: bool) -> BoxResult<()> {
    println!(
        "listening as {:?} on port {} (fingerprint {})",
        node.info().alias,
        node.port(),
        &node.fingerprint()[..8]
    );
    let mut rx = node.subscribe();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                node.shutdown();
                return Ok(());
            }
            e = rx.recv() => match e {
                Ok(Event::IncomingRequest { request }) => {
                    let note = match request.from.trust {
                        TrustLevel::Suspect => "  WARNING: this name was seen before with a different key",
                        _ if !request.sender_verified => "  (unverified sender)",
                        _ => "",
                    };
                    println!(
                        "{} wants to send {} file(s), {:.1} MB{note}",
                        clean(&request.from.alias),
                        request.files.len(),
                        request.total_size as f64 / 1e6
                    );
                    for f in &request.files {
                        println!("  {} ({} bytes)", clean(&f.file_name), f.size);
                    }
                    let accept = yes || ask("accept? [y/N] ").await;
                    let decision = if accept { Decision::Accept { dest: None } } else { Decision::Decline };
                    if let Err(e) = node.respond(&request.request_id, decision) {
                        eprintln!("{e}");
                        let _ = node.respond(&request.request_id, Decision::Decline);
                    }
                }
                Ok(Event::TransferDone { direction: Direction::Incoming, paths, .. }) => {
                    for p in paths {
                        println!("saved {}", p.display());
                    }
                }
                Ok(Event::TransferFailed { direction: Direction::Incoming, reason, .. }) => {
                    println!("transfer failed: {reason:?}")
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Peer-chosen text made safe to print: no terminal escape sequences.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

async fn ask(prompt: &str) -> bool {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    tokio::task::spawn_blocking(|| {
        let mut s = String::new();
        let _ = std::io::stdin().read_line(&mut s);
        s.trim().eq_ignore_ascii_case("y")
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(alias: &str, fp: &str) -> Device {
        Device {
            fingerprint: fp.into(),
            alias: alias.into(),
            device_model: None,
            device_type: None,
            addr: SocketAddr::from(([127, 0, 0, 1], 1)),
            trust: TrustLevel::Known,
            is_sendit: false,
        }
    }

    #[test]
    fn clean_strips_terminal_escapes() {
        assert_eq!(clean("a\x1b[1A\x1b[2Kb\n"), "a?[1A?[2Kb?");
        assert_eq!(clean("ไฟล์.txt"), "ไฟล์.txt");
    }

    #[test]
    fn pick_by_alias_or_prefix() {
        let ds = vec![
            dev("Pixel 8", "ABCDEF01"),
            dev("MacBook", "12345678"),
            dev("pixel 8", "ABCDEF99"),
        ];
        assert_eq!(pick(&ds, "macbook").unwrap().fingerprint, "12345678");
        assert_eq!(pick(&ds, "abcdef01").unwrap().alias, "Pixel 8");
        assert!(pick(&ds, "Pixel 8").is_err()); // ambiguous
        assert!(pick(&ds, "nobody").is_err());
        assert!(pick(&ds, "ABC").is_err()); // prefix too short
    }
}
