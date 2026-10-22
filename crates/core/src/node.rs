use crate::{
    discovery,
    error::{Error, Result},
    events::{Device, Event, FailReason},
    identity::Identity,
    protocol::*,
    receive::{has_space, Pending, Session},
    send, server, tls,
    trust::{Observation, TrustStore},
};
use axum_server::tls_rustls::RustlsConfig;
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct Config {
    pub alias: String,
    /// Interface the HTTPS server listens on. Tests use loopback so Windows
    /// Firewall doesn't prompt for every rebuilt test binary.
    pub bind: Ipv4Addr,
    /// TCP port for the HTTPS server. 0 = any free port.
    pub port: u16,
    pub multicast_port: u16,
    pub data_dir: PathBuf,
    pub download_dir: PathBuf,
    pub pin: Option<String>,
    pub multicast: bool,
    /// How long an incoming request waits for the user.
    pub request_timeout: Duration,
    /// A transfer with no bytes moving for this long fails.
    pub idle_timeout: Duration,
}

impl Config {
    pub fn new(
        alias: impl Into<String>,
        data_dir: impl Into<PathBuf>,
        download_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            alias: alias.into(),
            bind: Ipv4Addr::UNSPECIFIED,
            port: DEFAULT_PORT,
            multicast_port: DEFAULT_PORT,
            data_dir: data_dir.into(),
            download_dir: download_dir.into(),
            pin: None,
            multicast: true,
            request_timeout: Duration::from_secs(120),
            idle_timeout: Duration::from_secs(30),
        }
    }
}

pub(crate) struct Inner {
    pub cfg: Config,
    pub identity: Identity,
    pub port: u16,
    pub trust: Mutex<TrustStore>,
    /// Keyed by uppercase fingerprint.
    pub devices: Mutex<HashMap<String, Device>>,
    pub events: broadcast::Sender<Event>,
    pub pending: Mutex<HashMap<String, Pending>>,
    /// LocalSend allows one incoming session at a time; others get 409.
    pub session: Mutex<Option<Session>>,
    pub outgoing: Mutex<HashMap<String, CancellationToken>>,
    /// Fingerprints we are currently proving via `discovery::verify_claim`.
    pub verifying: Mutex<HashSet<String>>,
    pub pin_failures: Mutex<HashMap<IpAddr, u32>>,
}

#[derive(Clone, Debug)]
pub enum Decision {
    /// `dest = None` uses `Config::download_dir`.
    Accept {
        dest: Option<PathBuf>,
    },
    Decline,
}

impl Inner {
    pub(crate) fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    pub(crate) fn my_info(&self) -> DeviceInfo {
        DeviceInfo {
            alias: self.cfg.alias.clone(),
            version: PROTOCOL_VERSION.into(),
            device_model: Some(std::env::consts::OS.into()),
            device_type: Some("desktop".into()),
            fingerprint: self.identity.fingerprint.clone(),
            port: self.port,
            protocol: Scheme::Https,
            download: false,
            sendit: Some(env!("CARGO_PKG_VERSION").into()),
        }
    }

    /// A peer's unproven claim, for display only: nothing is stored or pinned.
    pub(crate) fn unverified_device(&self, info: &DeviceInfo, ip: IpAddr) -> Device {
        let fp = info.fingerprint.to_uppercase();
        let trust = self.trust.lock().unwrap().peek(&fp, &info.alias);
        Device {
            fingerprint: fp,
            alias: info.alias.clone(),
            device_model: info.device_model.clone(),
            device_type: info.device_type.clone(),
            addr: SocketAddr::new(ip, info.port),
            trust,
            is_sendit: info.sendit.is_some(),
        }
    }

    /// Records a peer whose key was proven over TLS (only `register_with` calls
    /// this). Returns None if `info` is ourselves.
    pub(crate) fn see_device(&self, info: &DeviceInfo, ip: IpAddr) -> Option<Device> {
        if info
            .fingerprint
            .eq_ignore_ascii_case(&self.identity.fingerprint)
        {
            return None;
        }
        let fp = info.fingerprint.to_uppercase();
        let (obs, trust) = {
            let mut t = self.trust.lock().unwrap();
            let obs = t.observe(&fp, &info.alias);
            (obs, t.level(&fp))
        };
        let device = Device {
            fingerprint: fp.clone(),
            alias: info.alias.clone(),
            device_model: info.device_model.clone(),
            device_type: info.device_type.clone(),
            addr: SocketAddr::new(ip, info.port),
            trust,
            is_sendit: info.sendit.is_some(),
        };
        self.devices.lock().unwrap().insert(fp, device.clone());
        match obs {
            Observation::IdentityChanged { old_fingerprint } => self.emit(Event::IdentityChanged {
                alias: info.alias.clone(),
                old_fingerprint,
                device: device.clone(),
            }),
            _ => self.emit(Event::DeviceUpdated {
                device: device.clone(),
            }),
        }
        Some(device)
    }
}

#[derive(Clone)]
pub struct Node {
    pub(crate) inner: Arc<Inner>,
    handle: axum_server::Handle,
    port_fallback: bool,
}

impl Node {
    pub async fn start(cfg: Config) -> Result<Node> {
        std::fs::create_dir_all(&cfg.data_dir)?;
        std::fs::create_dir_all(&cfg.download_dir)?;
        let identity = Identity::load_or_create(&cfg.data_dir.join("identity"))?;
        let trust = TrustStore::load(cfg.data_dir.join("trust.json"));
        let (listener, port_fallback) = match TcpListener::bind((cfg.bind, cfg.port)) {
            Ok(l) => (l, false),
            // Windows reports ports inside Hyper-V/WinNAT reserved ranges as PermissionDenied.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::AddrInUse | std::io::ErrorKind::PermissionDenied
                ) && cfg.port != 0 =>
            {
                (TcpListener::bind((cfg.bind, 0))?, true)
            }
            Err(e) => return Err(e.into()),
        };
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (events, _) = broadcast::channel(1024);
        let inner = Arc::new(Inner {
            cfg,
            identity,
            port,
            trust: Mutex::new(trust),
            devices: Mutex::new(HashMap::new()),
            events,
            pending: Mutex::new(HashMap::new()),
            session: Mutex::new(None),
            outgoing: Mutex::new(HashMap::new()),
            verifying: Mutex::new(HashSet::new()),
            pin_failures: Mutex::new(HashMap::new()),
        });
        let tls = RustlsConfig::from_config(tls::server_config(&inner.identity)?);
        let app = server::router(inner.clone()).into_make_service_with_connect_info::<SocketAddr>();
        let handle = axum_server::Handle::new();
        let server = axum_server::from_tcp_rustls(listener, tls)
            .handle(handle.clone())
            .serve(app);
        tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("https server stopped: {e}");
            }
        });
        if inner.cfg.multicast {
            discovery::spawn_listener(inner.clone())?;
            discovery::send_announcement(&inner, true);
        }
        Ok(Node {
            inner,
            handle,
            port_fallback,
        })
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    /// True if the configured port was busy and a random one is used instead.
    pub fn port_fallback(&self) -> bool {
        self.port_fallback
    }

    pub fn fingerprint(&self) -> &str {
        &self.inner.identity.fingerprint
    }

    pub fn info(&self) -> DeviceInfo {
        self.inner.my_info()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.events.subscribe()
    }

    pub fn devices(&self) -> Vec<Device> {
        let mut v: Vec<Device> = self
            .inner
            .devices
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        v.sort_by(|a, b| a.alias.to_lowercase().cmp(&b.alias.to_lowercase()));
        v
    }

    pub fn device(&self, fingerprint: &str) -> Option<Device> {
        self.inner
            .devices
            .lock()
            .unwrap()
            .get(&fingerprint.to_uppercase())
            .cloned()
    }

    /// Contact a peer directly by address (no multicast needed).
    pub async fn add_peer(&self, addr: SocketAddr) -> Result<Device> {
        discovery::register_with(&self.inner, addr, None).await
    }

    pub fn accept_new_key(&self, fingerprint: &str) {
        let fp = fingerprint.to_uppercase();
        let alias = self.device(&fp).map(|d| d.alias).unwrap_or_default();
        self.inner.trust.lock().unwrap().accept_new_key(&fp, &alias);
        self.refresh(&fp);
    }

    pub fn set_verified(&self, fingerprint: &str, verified: bool) {
        let fp = fingerprint.to_uppercase();
        self.inner.trust.lock().unwrap().set_verified(&fp, verified);
        self.refresh(&fp);
    }

    pub fn set_auto_accept(&self, fingerprint: &str, on: bool) {
        let fp = fingerprint.to_uppercase();
        self.inner.trust.lock().unwrap().set_auto_accept(&fp, on);
        self.refresh(&fp);
    }

    pub fn block(&self, fingerprint: &str) {
        let fp = fingerprint.to_uppercase();
        let alias = self.device(&fp).map(|d| d.alias).unwrap_or_default();
        self.inner.trust.lock().unwrap().block(&fp, &alias);
        self.refresh(&fp);
    }

    /// Answer an `Event::IncomingRequest`. If there is not enough space in the
    /// destination, returns `Error::DiskFull` and the request stays pending,
    /// so the caller can pick another folder or decline.
    pub fn respond(&self, request_id: &str, decision: Decision) -> Result<()> {
        let mut pending = self.inner.pending.lock().unwrap();
        let p = pending
            .remove(request_id)
            .ok_or_else(|| Error::NoSuchRequest(request_id.into()))?;
        if let Decision::Accept { dest } = &decision {
            let dir = dest
                .clone()
                .unwrap_or_else(|| self.inner.cfg.download_dir.clone());
            if !has_space(&dir, p.total) {
                pending.insert(request_id.to_string(), p);
                return Err(Error::DiskFull);
            }
        }
        let _ = p.tx.send(decision);
        Ok(())
    }

    /// Runs the whole transfer. Emits TransferStarted first, so a UI can grab
    /// the id for `cancel`. Returns that id on success.
    pub async fn send(
        &self,
        fingerprint: &str,
        paths: Vec<PathBuf>,
        pin: Option<String>,
    ) -> Result<String> {
        send::send(&self.inner, fingerprint, paths, pin).await
    }

    /// Cancels an outgoing or incoming transfer. Returns false if it was unknown.
    pub fn cancel(&self, transfer_id: &str) -> bool {
        if let Some(token) = self.inner.outgoing.lock().unwrap().get(transfer_id) {
            token.cancel();
            return true;
        }
        self.inner.fail_incoming(transfer_id, FailReason::Cancelled)
    }

    /// Shout "I'm here" on the LAN. Peers answer via /register.
    pub fn announce(&self) {
        discovery::send_announcement(&self.inner, true);
    }

    /// Slow fallback for networks that block multicast.
    pub async fn scan(&self) -> Vec<Device> {
        discovery::scan(&self.inner).await
    }

    /// Every device in the trust store (for "seen before").
    pub fn known_devices(&self) -> Vec<crate::trust::KnownDevice> {
        self.inner.trust.lock().unwrap().known()
    }

    pub fn download_dir(&self) -> &Path {
        &self.inner.cfg.download_dir
    }

    pub fn shutdown(&self) {
        self.handle.shutdown();
    }

    fn refresh(&self, fp: &str) {
        let level = self.inner.trust.lock().unwrap().level(fp);
        let updated = self.inner.devices.lock().unwrap().get_mut(fp).map(|d| {
            d.trust = level;
            d.clone()
        });
        if let Some(device) = updated {
            self.inner.emit(Event::DeviceUpdated { device });
        }
    }
}
