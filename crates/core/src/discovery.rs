use crate::{
    error::{Error, Result},
    events::Device,
    node::Inner,
    protocol::*,
    tls::PeerClient,
};
use futures::StreamExt;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

/// POST our info to `addr`/register. If `expected` is set, the TLS cert must
/// match it. Either way, the cert must match the fingerprint in the reply.
pub(crate) async fn register_with(
    n: &Inner,
    addr: SocketAddr,
    expected: Option<&str>,
) -> Result<Device> {
    let client = PeerClient::new(expected)?;
    let resp = client
        .http
        .post(format!("https://{addr}{API}/register"))
        .json(&n.my_info())
        .timeout(Duration::from_secs(3))
        .send()
        .await?
        .error_for_status()?;
    let mut info: DeviceInfo = resp.json().await?;
    let seen = client
        .seen_fingerprint()
        .ok_or(Error::FingerprintMismatch)?;
    if !seen.eq_ignore_ascii_case(&info.fingerprint) {
        return Err(Error::FingerprintMismatch);
    }
    // /register replies omit port and protocol.
    info.port = addr.port();
    info.protocol = Scheme::Https;
    n.see_device(&info, addr.ip()).ok_or(Error::SelfDevice)
}

/// Prove an unproven claim by calling the claimed address, pinned to the
/// claimed key. Only a proven claim is stored or allowed to touch the trust store.
pub(crate) async fn verify_claim(n: &Inner, info: &DeviceInfo, ip: IpAddr) -> Option<Device> {
    if info.protocol != Scheme::Https {
        return None;
    }
    let fp = info.fingerprint.to_uppercase();
    let addr = SocketAddr::new(ip, info.port);
    if let Some(d) = n.devices.lock().unwrap().get(&fp) {
        if d.addr == addr {
            return Some(d.clone());
        }
    }
    // Two sendit nodes verifying each other would otherwise ping-pong forever.
    if !n.verifying.lock().unwrap().insert(fp.clone()) {
        return None;
    }
    let proven = register_with(n, addr, Some(&fp)).await.ok();
    n.verifying.lock().unwrap().remove(&fp);
    proven
}

pub(crate) fn accept_announcement(a: &Announcement, own_fp: &str) -> bool {
    // ponytail: LocalSend's "encryption off" mode has no cert to pin; ignored in v1.
    !a.info.fingerprint.eq_ignore_ascii_case(own_fp) && a.info.protocol == Scheme::Https
}

pub(crate) fn ipv4_interfaces() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .collect()
}

fn interfaces_or_any() -> Vec<Ipv4Addr> {
    let v = ipv4_interfaces();
    if v.is_empty() {
        vec![Ipv4Addr::UNSPECIFIED]
    } else {
        v
    }
}

fn multicast_socket(port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(unix)]
    s.set_reuse_port(true)?;
    s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    for ip in interfaces_or_any() {
        if let Err(e) = s.join_multicast_v4(&MULTICAST_ADDR, &ip) {
            tracing::debug!("join multicast on {ip}: {e}");
        }
    }
    s.set_multicast_loop_v4(true)?;
    s.set_nonblocking(true)?;
    tokio::net::UdpSocket::from_std(s.into())
}

pub(crate) fn spawn_listener(n: Arc<Inner>) -> std::io::Result<()> {
    let sock = multicast_socket(n.cfg.multicast_port)?;
    tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let Ok((len, src)) = sock.recv_from(&mut buf).await else {
                continue;
            };
            let Ok(a) = serde_json::from_slice::<Announcement>(&buf[..len]) else {
                continue;
            };
            if !accept_announcement(&a, &n.identity.fingerprint) {
                continue;
            }
            let n = n.clone();
            tokio::spawn(async move { handle_announcement(&n, a, src.ip()).await });
        }
    });
    Ok(())
}

async fn handle_announcement(n: &Arc<Inner>, a: Announcement, ip: IpAddr) {
    if !a.announce {
        verify_claim(n, &a.info, ip).await;
        return;
    }
    // Answer like LocalSend does: HTTP /register first, UDP reply if that fails.
    let addr = SocketAddr::new(ip, a.info.port);
    if register_with(n, addr, Some(&a.info.fingerprint))
        .await
        .is_err()
    {
        send_announcement(n, false);
    }
}

pub(crate) fn send_announcement(n: &Inner, announce: bool) {
    let Ok(msg) = serde_json::to_vec(&Announcement {
        info: n.my_info(),
        announce,
    }) else {
        return;
    };
    let target = SocketAddrV4::new(MULTICAST_ADDR, n.cfg.multicast_port);
    for ip in interfaces_or_any() {
        let sent = (|| -> std::io::Result<()> {
            let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            s.bind(&SocketAddrV4::new(ip, 0).into())?;
            s.set_multicast_if_v4(&ip)?;
            s.set_multicast_loop_v4(true)?;
            s.send_to(&msg, &target.into())?;
            Ok(())
        })();
        if let Err(e) = sent {
            tracing::debug!("announce on {ip}: {e}");
        }
    }
}

/// Fallback when multicast is blocked: try /register on every host of our /24s.
// ponytail: assumes /24 subnets; read real netmasks if users report misses.
pub(crate) async fn scan(n: &Arc<Inner>) -> Vec<Device> {
    let own: HashSet<Ipv4Addr> = ipv4_interfaces().into_iter().collect();
    let targets: Vec<SocketAddr> = own
        .iter()
        .flat_map(|ip| {
            let o = ip.octets();
            (1..=254u8).map(move |h| Ipv4Addr::new(o[0], o[1], o[2], h))
        })
        .filter(|ip| !own.contains(ip))
        .map(|ip| SocketAddr::new(IpAddr::V4(ip), DEFAULT_PORT))
        .collect();
    futures::stream::iter(targets)
        .map(|addr| {
            let n = n.clone();
            async move { register_with(&n, addr, None).await.ok() }
        })
        .buffer_unordered(64)
        .filter_map(|d| async { d })
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ann(fp: &str, protocol: Scheme) -> Announcement {
        Announcement {
            info: DeviceInfo {
                alias: "x".into(),
                version: "2.1".into(),
                device_model: None,
                device_type: None,
                fingerprint: fp.into(),
                port: 53317,
                protocol,
                download: false,
                sendit: None,
            },
            announce: true,
        }
    }

    #[test]
    fn ignores_self_and_plain_http_peers() {
        assert!(accept_announcement(&ann("AA", Scheme::Https), "BB"));
        assert!(!accept_announcement(&ann("aa", Scheme::Https), "AA"));
        assert!(!accept_announcement(&ann("CC", Scheme::Http), "AA"));
    }
}
