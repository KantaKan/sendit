//! Outgoing side: prepare-upload, then parallel streaming uploads.
use crate::{
    error::{Error, Result},
    events::{stalled, Device, Direction, Event, FailReason, Meter},
    node::Inner,
    protocol::*,
    tls::PeerClient,
    trust::TrustLevel,
};
use futures::{StreamExt, TryStreamExt};
use reqwest::header::CONTENT_LENGTH;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

const PARALLEL_UPLOADS: usize = 3;

pub(crate) async fn send(
    n: &Arc<Inner>,
    fingerprint: &str,
    paths: Vec<PathBuf>,
    pin: Option<String>,
) -> Result<String> {
    let device = n
        .devices
        .lock()
        .unwrap()
        .get(&fingerprint.to_uppercase())
        .cloned()
        .ok_or_else(|| Error::UnknownDevice(fingerprint.into()))?;
    match device.trust {
        TrustLevel::Suspect => return Err(Error::Suspect),
        TrustLevel::Blocked => return Err(Error::Blocked),
        _ => {}
    }
    let files = collect_files(&paths)?;
    let total = files.iter().map(|(m, _)| m.size).sum();
    let id = Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    n.outgoing
        .lock()
        .unwrap()
        .insert(id.clone(), cancel.clone());
    n.emit(Event::TransferStarted {
        transfer_id: id.clone(),
        request_id: None,
        direction: Direction::Outgoing,
        peer: device.fingerprint.clone(),
        files: files.iter().map(|(m, _)| m.clone()).collect(),
        total,
        sources: paths.clone(),
    });
    let meter = Meter::new(n.events.clone(), id.clone(), Direction::Outgoing, total);
    let result = run(n, &device, files, pin, &meter, &cancel).await;
    n.outgoing.lock().unwrap().remove(&id);
    match result {
        Ok(()) => {
            n.emit(Event::TransferDone {
                transfer_id: id.clone(),
                direction: Direction::Outgoing,
                paths,
            });
            Ok(id)
        }
        Err(reason) => {
            n.emit(Event::TransferFailed {
                transfer_id: id,
                direction: Direction::Outgoing,
                reason: reason.clone(),
            });
            Err(Error::Transfer(reason))
        }
    }
}

async fn run(
    n: &Inner,
    device: &Device,
    files: Vec<(FileMeta, PathBuf)>,
    pin: Option<String>,
    meter: &Arc<Meter>,
    cancel: &CancellationToken,
) -> std::result::Result<(), FailReason> {
    let client = PeerClient::new(Some(&device.fingerprint))
        .map_err(|e| FailReason::Protocol(e.to_string()))?;
    let base = device.base_url();
    let request = PrepareUploadRequest {
        info: n.my_info(),
        files: files
            .iter()
            .map(|(m, _)| (m.id.clone(), m.clone()))
            .collect(),
    };
    let mut prepare = client
        .http
        .post(format!("{base}/prepare-upload"))
        .json(&request)
        .timeout(n.cfg.request_timeout + Duration::from_secs(10));
    if let Some(pin) = &pin {
        prepare = prepare.query(&[("pin", pin)]);
    }
    let resp = tokio::select! {
        r = prepare.send() => r.map_err(|_| connect_failure(&client, device))?,
        _ = cancel.cancelled() => return Err(FailReason::Cancelled),
    };
    match resp.status().as_u16() {
        200 => {}
        204 => return Ok(()),
        401 => return Err(FailReason::PinRequired),
        403 => return Err(FailReason::Declined),
        409 | 429 => return Err(FailReason::Busy),
        s => return Err(FailReason::Protocol(format!("prepare-upload returned {s}"))),
    }
    let prep: PrepareUploadResponse = resp
        .json()
        .await
        .map_err(|e| FailReason::Protocol(e.to_string()))?;
    // Files missing from the reply were skipped by the receiver.
    let jobs = files.into_iter().filter_map(|(meta, path)| {
        let token = prep.files.get(&meta.id)?.clone();
        Some((meta, path, token))
    });
    let uploads = futures::stream::iter(jobs)
        .map(|(meta, path, token)| {
            upload_one(
                &client,
                &base,
                &prep.session_id,
                meta,
                path,
                token,
                meter.clone(),
            )
        })
        .buffer_unordered(PARALLEL_UPLOADS)
        .try_collect::<Vec<()>>();
    let outcome = tokio::select! {
        r = uploads => r.map(|_| ()),
        _ = stalled(meter, n.cfg.idle_timeout) => Err(FailReason::PeerGone),
        _ = cancel.cancelled() => Err(FailReason::Cancelled),
    };
    if outcome.is_err() {
        let _ = client
            .http
            .post(format!("{base}/cancel"))
            .query(&[("sessionId", &prep.session_id)])
            .timeout(Duration::from_secs(3))
            .send()
            .await;
    }
    outcome
}

async fn upload_one(
    client: &PeerClient,
    base: &str,
    session_id: &str,
    meta: FileMeta,
    path: PathBuf,
    token: String,
    meter: Arc<Meter>,
) -> std::result::Result<(), FailReason> {
    let file = tokio::fs::File::open(&path).await?;
    let body = ReaderStream::with_capacity(file, 256 * 1024)
        .inspect_ok(move |chunk| meter.add(chunk.len() as u64));
    let resp = client
        .http
        .post(format!("{base}/upload"))
        .query(&[
            ("sessionId", session_id),
            ("fileId", meta.id.as_str()),
            ("token", token.as_str()),
        ])
        .header(CONTENT_LENGTH, meta.size)
        .body(reqwest::Body::wrap_stream(body))
        .send()
        .await
        .map_err(|_| FailReason::PeerGone)?;
    match resp.status().as_u16() {
        200 => Ok(()),
        422 => Err(FailReason::Checksum),
        408 => Err(FailReason::PeerGone),
        403 | 409 => Err(FailReason::Cancelled),
        s => Err(FailReason::Protocol(format!("upload returned {s}"))),
    }
}

fn connect_failure(client: &PeerClient, device: &Device) -> FailReason {
    match client.seen_fingerprint() {
        Some(seen) if !seen.eq_ignore_ascii_case(&device.fingerprint) => {
            FailReason::IdentityMismatch
        }
        _ => FailReason::PeerGone,
    }
}

/// Files become `name`; folders become `folder/sub/name` entries.
pub(crate) fn collect_files(paths: &[PathBuf]) -> std::io::Result<Vec<(FileMeta, PathBuf)>> {
    let mut out = Vec::new();
    for root in paths {
        if root.is_dir() {
            let base = root.parent().unwrap_or(root);
            for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
                let entry = entry.map_err(std::io::Error::other)?;
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(base).unwrap_or(entry.path());
                    out.push(meta_for(entry.path(), &rel_name(rel))?);
                }
            }
        } else {
            let name = root
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            out.push(meta_for(root, &name)?);
        }
    }
    Ok(out)
}

fn rel_name(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn meta_for(path: &Path, name: &str) -> std::io::Result<(FileMeta, PathBuf)> {
    let size = std::fs::metadata(path)?.len();
    let meta = FileMeta {
        id: Uuid::new_v4().to_string(),
        file_name: name.to_string(),
        size,
        file_type: mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string(),
        sha256: None,
        preview: None,
        metadata: None,
    };
    Ok((meta, path.to_path_buf()))
}
