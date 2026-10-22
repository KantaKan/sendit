use crate::events::FailReason;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls: {0}")]
    Tls(#[from] rustls::Error),
    #[error("certificate: {0}")]
    Cert(#[from] rcgen::Error),
    #[error("network: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unknown device {0}")]
    UnknownDevice(String),
    #[error("that address is this device")]
    SelfDevice,
    #[error("this device's key changed; confirm it or block it first")]
    Suspect,
    #[error("device is blocked")]
    Blocked,
    #[error("peer's certificate does not match its fingerprint")]
    FingerprintMismatch,
    #[error("no pending request {0}")]
    NoSuchRequest(String),
    #[error("not enough disk space")]
    DiskFull,
    #[error("transfer failed: {0:?}")]
    Transfer(FailReason),
}

pub type Result<T> = std::result::Result<T, Error>;
