//! Incoming side: one session at a time, files streamed to `.sendit-part`, then renamed.
use crate::{
    events::{stalled, Device, Direction, Event, FailReason, Meter},
    node::{Decision, Inner},
    protocol::*,
    sanitize::{sanitize_relative, unique_path},
};
use axum::{body::Body, http::StatusCode};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) const PART_SUFFIX: &str = ".sendit-part";

pub(crate) struct Pending {
    pub tx: oneshot::Sender<Decision>,
    pub total: u64,
}

pub(crate) struct Session {
    pub id: String,
    pub dest: PathBuf,
    pub files: HashMap<String, Slot>,
    pub meter: Arc<Meter>,
    pub cancel: CancellationToken,
    /// Why the session ended early, so in-flight uploads answer accordingly.
    pub why: Arc<OnceLock<FailReason>>,
    pub paths: Vec<PathBuf>,
}

pub(crate) struct Slot {
    pub meta: FileMeta,
    pub token: String,
    pub state: SlotState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotState {
    Waiting,
    Receiving,
    Done,
}

pub(crate) fn has_space(dir: &Path, need: u64) -> bool {
    fs2::available_space(dir)
        .map(|free| free >= need)
        .unwrap_or(true)
}

impl FailReason {
    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::Checksum => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Protocol(_) => StatusCode::BAD_REQUEST,
            Self::PeerGone => StatusCode::REQUEST_TIMEOUT,
            Self::Cancelled => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

type Claim = (
    FileMeta,
    PathBuf,
    Arc<Meter>,
    CancellationToken,
    Arc<OnceLock<FailReason>>,
);

impl Inner {
    /// Returns None if another session is already active.
    pub(crate) fn open_session(
        self: &Arc<Self>,
        from: Device,
        files: Vec<FileMeta>,
        total: u64,
        dest: PathBuf,
    ) -> Option<PrepareUploadResponse> {
        let id = Uuid::new_v4().to_string();
        let meter = Meter::new(self.events.clone(), id.clone(), Direction::Incoming, total);
        let cancel = CancellationToken::new();
        let mut tokens = HashMap::new();
        let mut slots = HashMap::new();
        for meta in &files {
            let token = Uuid::new_v4().to_string();
            tokens.insert(meta.id.clone(), token.clone());
            slots.insert(
                meta.id.clone(),
                Slot {
                    meta: meta.clone(),
                    token,
                    state: SlotState::Waiting,
                },
            );
        }
        {
            let mut guard = self.session.lock().unwrap();
            if guard.is_some() {
                return None;
            }
            *guard = Some(Session {
                id: id.clone(),
                dest,
                files: slots,
                meter: meter.clone(),
                cancel: cancel.clone(),
                why: Arc::new(OnceLock::new()),
                paths: vec![],
            });
        }
        self.emit(Event::TransferStarted {
            transfer_id: id.clone(),
            direction: Direction::Incoming,
            peer: from.fingerprint,
            files,
            total,
            sources: vec![],
        });
        let n = self.clone();
        let (idle, sid) = (self.cfg.idle_timeout, id.clone());
        tokio::spawn(async move {
            tokio::select! {
                _ = stalled(&meter, idle) => { n.fail_incoming(&sid, FailReason::PeerGone); }
                _ = cancel.cancelled() => {}
            }
        });
        Some(PrepareUploadResponse {
            session_id: id,
            files: tokens,
        })
    }

    pub(crate) fn claim_file(&self, q: &UploadQuery) -> Result<Claim, StatusCode> {
        let mut guard = self.session.lock().unwrap();
        let s = guard
            .as_mut()
            .filter(|s| s.id == q.session_id)
            .ok_or(StatusCode::FORBIDDEN)?;
        let slot = s.files.get_mut(&q.file_id).ok_or(StatusCode::FORBIDDEN)?;
        if slot.token != q.token || slot.state != SlotState::Waiting {
            return Err(StatusCode::FORBIDDEN);
        }
        slot.state = SlotState::Receiving;
        Ok((
            slot.meta.clone(),
            s.dest.clone(),
            s.meter.clone(),
            s.cancel.clone(),
            s.why.clone(),
        ))
    }

    pub(crate) fn file_finished(&self, session_id: &str, file_id: &str, path: PathBuf) {
        let finished = {
            let mut guard = self.session.lock().unwrap();
            let Some(s) = guard.as_mut().filter(|s| s.id == session_id) else {
                return;
            };
            if let Some(slot) = s.files.get_mut(file_id) {
                slot.state = SlotState::Done;
            }
            s.paths.push(path);
            if s.files.values().all(|f| f.state == SlotState::Done) {
                guard.take()
            } else {
                None
            }
        };
        if let Some(s) = finished {
            s.cancel.cancel(); // stops the watchdog
            self.emit(Event::TransferDone {
                transfer_id: s.id,
                direction: Direction::Incoming,
                paths: s.paths,
            });
        }
    }

    /// Ends the session `session_id` if it is still active. Returns false if it was not.
    pub(crate) fn fail_incoming(&self, session_id: &str, reason: FailReason) -> bool {
        let taken = {
            let mut guard = self.session.lock().unwrap();
            if guard.as_ref().is_some_and(|s| s.id == session_id) {
                guard.take()
            } else {
                None
            }
        };
        let Some(s) = taken else { return false };
        let _ = s.why.set(reason.clone());
        s.cancel.cancel();
        self.emit(Event::TransferFailed {
            transfer_id: s.id,
            direction: Direction::Incoming,
            reason,
        });
        true
    }
}

pub(crate) async fn write_file(
    meta: &FileMeta,
    dest: &Path,
    body: Body,
    meter: &Meter,
    cancel: &CancellationToken,
) -> Result<PathBuf, FailReason> {
    let rel = sanitize_relative(&meta.file_name);
    let target = dest.join(&rel);
    let dir = target.parent().unwrap_or(dest).to_path_buf();
    tokio::fs::create_dir_all(&dir).await?;
    // A junction/symlink already inside `dest` must not lead writes outside it.
    let inside = match (std::fs::canonicalize(&dir), std::fs::canonicalize(dest)) {
        (Ok(d), Ok(root)) => d.starts_with(root),
        _ => false,
    };
    if !inside {
        return Err(FailReason::Protocol(
            "path leaves the download folder".into(),
        ));
    }
    let part = dir.join(format!(".{}{PART_SUFFIX}", Uuid::new_v4()));
    let result: Result<PathBuf, FailReason> = async {
        stream_into(&part, meta, body, meter, cancel).await?;
        Ok(finalize(&part, dest, &rel)?)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&part).await;
    }
    result
}

// ponytail: one global lock around pick-a-free-name + rename (rename replaces
// existing files); per-directory locks if this ever shows up in a profile.
static FINALIZE: Mutex<()> = Mutex::new(());

fn finalize(part: &Path, dest: &Path, rel: &Path) -> std::io::Result<PathBuf> {
    let _guard = FINALIZE.lock().unwrap_or_else(|e| e.into_inner());
    let final_path = unique_path(dest, rel);
    std::fs::rename(part, &final_path)?;
    Ok(final_path)
}

async fn stream_into(
    part: &Path,
    meta: &FileMeta,
    body: Body,
    meter: &Meter,
    cancel: &CancellationToken,
) -> Result<(), FailReason> {
    let mut file = tokio::fs::File::create(part).await?;
    let mut hasher = meta.sha256.as_ref().map(|_| Sha256::new());
    let mut stream = body.into_data_stream();
    let mut written = 0u64;
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => return Err(FailReason::Cancelled),
            next = stream.next() => next,
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|_| FailReason::PeerGone)?;
        written += chunk.len() as u64;
        if written > meta.size {
            return Err(FailReason::Protocol(
                "peer sent more bytes than it declared".into(),
            ));
        }
        if let Some(h) = hasher.as_mut() {
            h.update(&chunk);
        }
        file.write_all(&chunk).await?;
        meter.add(chunk.len() as u64);
    }
    if written != meta.size {
        return Err(FailReason::PeerGone);
    }
    file.flush().await?;
    if let (Some(h), Some(expected)) = (hasher, &meta.sha256) {
        if !hex::encode(h.finalize()).eq_ignore_ascii_case(expected) {
            return Err(FailReason::Checksum);
        }
    }
    Ok(())
}
