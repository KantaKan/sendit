use crate::{
    error::{Error, Result},
    events::Device,
    node::Inner,
    protocol::*,
    tls::PeerClient,
};
use std::{net::SocketAddr, time::Duration};

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
