//! Everything the UI/CLI learns about comes through `Event`.
use crate::{
    protocol::{FileMeta, API},
    trust::TrustLevel,
};
use serde::Serialize;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Uppercase hex. The stable id of a device.
    pub fingerprint: String,
    pub alias: String,
    pub device_model: Option<String>,
    pub device_type: Option<String>,
    pub addr: SocketAddr,
    pub trust: TrustLevel,
    pub is_sendit: bool,
}

impl Device {
    pub fn base_url(&self) -> String {
        format!("https://{}{}", self.addr, API)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum FailReason {
    Declined,
    Cancelled,
    PeerGone,
    Busy,
    PinRequired,
    Blocked,
    Checksum,
    DiskFull,
    IdentityMismatch,
    Protocol(String),
    Io(String),
}

impl From<std::io::Error> for FailReason {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::StorageFull {
            Self::DiskFull
        } else {
            Self::Io(e.to_string())
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequest {
    pub request_id: String,
    pub from: Device,
    pub files: Vec<FileMeta>,
    pub total_size: u64,
    /// Whether the default download dir has room. The UI warns if not.
    pub fits_on_disk: bool,
    /// The sender proved it owns the key it claims (we called back, pinned).
    pub sender_verified: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    DeviceUpdated {
        device: Device,
    },
    IdentityChanged {
        alias: String,
        old_fingerprint: String,
        device: Device,
    },
    IncomingRequest {
        request: IncomingRequest,
    },
    TransferStarted {
        transfer_id: String,
        direction: Direction,
        peer: String,
        files: Vec<FileMeta>,
        total: u64,
    },
    Progress {
        transfer_id: String,
        direction: Direction,
        bytes: u64,
        total: u64,
    },
    TransferDone {
        transfer_id: String,
        direction: Direction,
        paths: Vec<PathBuf>,
    },
    TransferFailed {
        transfer_id: String,
        direction: Direction,
        reason: FailReason,
    },
}

// TODO(task 8): remove once receive/send use Meter.
#[allow(dead_code)]
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

#[allow(dead_code)]
/// Counts bytes of one transfer and emits throttled `Progress` events.
pub(crate) struct Meter {
    tx: broadcast::Sender<Event>,
    id: String,
    dir: Direction,
    total: u64,
    done: AtomicU64,
    last: Mutex<Instant>,
}

#[allow(dead_code)]
impl Meter {
    pub(crate) fn new(
        tx: broadcast::Sender<Event>,
        id: String,
        dir: Direction,
        total: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            tx,
            id,
            dir,
            total,
            done: AtomicU64::new(0),
            last: Mutex::new(Instant::now()),
        })
    }

    pub(crate) fn add(&self, n: u64) {
        let done = self.done.fetch_add(n, Ordering::Relaxed) + n;
        let mut last = self.last.lock().unwrap();
        if done >= self.total || last.elapsed() >= PROGRESS_EVERY {
            *last = Instant::now();
            let _ = self.tx.send(Event::Progress {
                transfer_id: self.id.clone(),
                direction: self.dir,
                bytes: done,
                total: self.total,
            });
        }
    }

    pub(crate) fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }
}

#[allow(dead_code)]
/// Resolves once `meter` makes no progress for a whole `idle` period.
pub(crate) async fn stalled(meter: &Meter, idle: Duration) {
    let mut last = meter.done();
    loop {
        tokio::time::sleep(idle).await;
        let now = meter.done();
        if now == last {
            return;
        }
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_throttles_but_always_reports_completion() {
        let (tx, mut rx) = broadcast::channel(4096);
        let m = Meter::new(tx, "t".into(), Direction::Outgoing, 1000);
        for _ in 0..1000 {
            m.add(1);
        }
        let mut events = vec![];
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }
        assert!(events.len() < 5, "{} events", events.len());
        let Some(Event::Progress { bytes, total, .. }) = events.last() else {
            panic!("no progress")
        };
        assert_eq!((*bytes, *total), (1000, 1000));
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_waits_for_a_quiet_period() {
        let (tx, _rx) = broadcast::channel(16);
        let m = Meter::new(tx, "t".into(), Direction::Incoming, 100);
        let feeder = {
            let m = m.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                for _ in 0..3 {
                    m.add(1);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
        };
        let start = tokio::time::Instant::now();
        stalled(&m, Duration::from_secs(1)).await;
        let waited = start.elapsed();
        assert!(
            waited >= Duration::from_secs(4) && waited < Duration::from_secs(5),
            "{waited:?}"
        );
        feeder.await.unwrap();
    }

    #[test]
    fn io_storage_full_maps_to_disk_full() {
        let e = std::io::Error::from(std::io::ErrorKind::StorageFull);
        assert_eq!(FailReason::from(e), FailReason::DiskFull);
    }
}
