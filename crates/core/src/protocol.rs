//! LocalSend protocol v2 wire types. https://github.com/localsend/protocol
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

pub const PROTOCOL_VERSION: &str = "2.1";
pub const DEFAULT_PORT: u16 = 53317;
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
pub const API: &str = "/api/localsend/v2";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Http,
    #[default]
    Https,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

/// Sent in announcements, /register bodies and responses, and prepare-upload `info`.
/// Fields LocalSend omits in some messages (port, protocol) get defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub alias: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_model: Option<String>,
    /// Kept as a string so unknown future types still parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    pub fingerprint: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub protocol: Scheme,
    #[serde(default)]
    pub download: bool,
    /// sendit extension: our app version. LocalSend ignores unknown fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sendit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Announcement {
    #[serde(flatten)]
    pub info: DeviceInfo,
    #[serde(default)]
    pub announce: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    #[serde(default)]
    pub file_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PrepareUploadRequest {
    pub info: DeviceInfo,
    pub files: HashMap<String, FileMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadResponse {
    pub session_id: String,
    /// fileId -> token
    pub files: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UploadQuery {
    pub session_id: String,
    pub file_id: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionQuery {
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PinQuery {
    pub pin: Option<String>,
}
