use sendit_core::protocol::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn parses_announcement() {
    let a: Announcement = serde_json::from_str(&fixture("announce.json")).unwrap();
    assert_eq!(a.info.alias, "Nice Orange");
    assert_eq!(a.info.port, 53317);
    assert_eq!(a.info.protocol, Scheme::Https);
    assert!(a.info.download);
    assert!(a.announce);
}

#[test]
fn tolerates_unknown_fields_and_device_types() {
    let a: Announcement = serde_json::from_str(&fixture("announce_future.json")).unwrap();
    assert_eq!(a.info.device_type.as_deref(), Some("smartwatch"));
    assert_eq!(a.info.device_model, None);
}

#[test]
fn register_response_defaults_port_and_protocol() {
    let i: DeviceInfo = serde_json::from_str(&fixture("register_response.json")).unwrap();
    assert_eq!(i.port, DEFAULT_PORT);
    assert_eq!(i.protocol, Scheme::Https);
    assert_eq!(i.sendit, None);
}

#[test]
fn prepare_upload_round_trips() {
    let r: PrepareUploadRequest = serde_json::from_str(&fixture("prepare_upload.json")).unwrap();
    assert_eq!(r.files.len(), 2);
    let f = &r.files["some file id"];
    assert_eq!(f.file_name, "my image.png");
    assert_eq!(f.size, 324242);
    assert_eq!(r.files["another file id"].sha256, None);
    let again: PrepareUploadRequest =
        serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(r, again);
}

#[test]
fn prepare_upload_response_parses() {
    let r: PrepareUploadResponse =
        serde_json::from_str(&fixture("prepare_upload_response.json")).unwrap();
    assert_eq!(r.session_id, "mySessionId");
    assert_eq!(r.files["someFileId"], "someFileToken");
}

#[test]
fn serializes_camel_case_without_nulls() {
    let info = DeviceInfo {
        alias: "pc".into(),
        version: PROTOCOL_VERSION.into(),
        device_model: Some("windows".into()),
        device_type: None,
        fingerprint: "AB".into(),
        port: 1234,
        protocol: Scheme::Https,
        download: false,
        sendit: Some("0.1.0".into()),
    };
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["deviceModel"], "windows");
    assert_eq!(v["protocol"], "https");
    assert_eq!(v["sendit"], "0.1.0");
    assert!(v.get("deviceType").is_none());
    assert!(v.get("device_model").is_none());
}
