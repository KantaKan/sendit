//! TLS with self-signed certs. Instead of trusting a CA, we pin a peer's
//! certificate to the fingerprint it announced. This is sendit's main
//! defence against MITM that LocalSend itself does not do.
use crate::{
    error::Result,
    identity::{fingerprint_of, Identity},
};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{ring, CryptoProvider},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

pub fn server_config(id: &Identity) -> Result<Arc<rustls::ServerConfig>> {
    let cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(id.cert_der.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key_der.clone())),
        )?;
    Ok(Arc::new(cfg))
}

#[derive(Debug)]
struct PinnedVerifier {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint_of(end_entity.as_ref());
        *self.seen.lock().unwrap() = Some(fp.clone());
        match &self.expected {
            Some(want) if !want.eq_ignore_ascii_case(&fp) => Err(rustls::Error::General(
                "certificate fingerprint mismatch".into(),
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// An HTTPS client for exactly one peer. With `expected = None` it accepts
/// any cert, so the caller must compare `seen_fingerprint()` to what the peer claims.
pub struct PeerClient {
    pub http: reqwest::Client,
    seen: Arc<Mutex<Option<String>>>,
}

impl PeerClient {
    pub fn new(expected: Option<&str>) -> Result<Self> {
        let seen = Arc::new(Mutex::new(None));
        let verifier = PinnedVerifier {
            expected: expected.map(str::to_string),
            seen: seen.clone(),
            provider: provider(),
        };
        let tls = rustls::ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let http = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .build()?;
        Ok(Self { http, seen })
    }

    pub fn seen_fingerprint(&self) -> Option<String> {
        self.seen.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use std::net::SocketAddr;

    async fn serve(id: &Identity) -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
        let tls = axum_server::tls_rustls::RustlsConfig::from_config(server_config(id).unwrap());
        tokio::spawn(axum_server::from_tcp_rustls(listener, tls).serve(app.into_make_service()));
        addr
    }

    #[tokio::test]
    async fn accepts_matching_fingerprint_in_any_case() {
        let id = Identity::generate().unwrap();
        let addr = serve(&id).await;
        for fp in [id.fingerprint.clone(), id.fingerprint.to_lowercase()] {
            let c = PeerClient::new(Some(&fp)).unwrap();
            let body = c
                .http
                .get(format!("https://{addr}/"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert_eq!(body, "ok");
        }
    }

    #[tokio::test]
    async fn rejects_wrong_fingerprint_and_records_what_it_saw() {
        let id = Identity::generate().unwrap();
        let other = Identity::generate().unwrap();
        let addr = serve(&id).await;
        let c = PeerClient::new(Some(&other.fingerprint)).unwrap();
        assert!(c.http.get(format!("https://{addr}/")).send().await.is_err());
        assert_eq!(
            c.seen_fingerprint().as_deref(),
            Some(id.fingerprint.as_str())
        );
    }

    #[tokio::test]
    async fn unpinned_client_accepts_and_records() {
        let id = Identity::generate().unwrap();
        let addr = serve(&id).await;
        let c = PeerClient::new(None).unwrap();
        c.http.get(format!("https://{addr}/")).send().await.unwrap();
        assert_eq!(
            c.seen_fingerprint().as_deref(),
            Some(id.fingerprint.as_str())
        );
    }
}
