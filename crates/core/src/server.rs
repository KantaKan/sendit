use crate::{
    discovery,
    events::{Event, FailReason, IncomingRequest},
    node::{Decision, Inner},
    protocol::*,
    receive::{self, has_space, Pending},
    trust::TrustLevel,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) fn router(n: Arc<Inner>) -> Router {
    Router::new()
        .route(&format!("{API}/register"), post(register))
        .route(&format!("{API}/info"), get(info))
        .route(&format!("{API}/prepare-upload"), post(prepare_upload))
        .route(&format!("{API}/upload"), post(upload))
        .route(&format!("{API}/cancel"), post(cancel))
        // JSON bodies only; /upload streams its raw body and is not affected.
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(n)
}

async fn register(
    State(n): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Json(info): Json<DeviceInfo>,
) -> Json<DeviceInfo> {
    discovery::verify_claim(&n, &info, remote.ip()).await;
    Json(n.my_info())
}

async fn info(State(n): State<Arc<Inner>>) -> Json<DeviceInfo> {
    Json(n.my_info())
}

// ponytail: failures never decay except on a correct PIN; a time window if real
// users get locked out.
const MAX_PIN_FAILURES: u32 = 5;

fn busy(n: &Inner) -> bool {
    n.session.lock().unwrap().is_some() || !n.pending.lock().unwrap().is_empty()
}

async fn prepare_upload(
    State(n): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Query(q): Query<PinQuery>,
    Json(req): Json<PrepareUploadRequest>,
) -> Response {
    if let Some(pin) = &n.cfg.pin {
        let ip = remote.ip();
        let failures = n
            .pin_failures
            .lock()
            .unwrap()
            .get(&ip)
            .copied()
            .unwrap_or(0);
        if failures >= MAX_PIN_FAILURES {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        if q.pin.as_deref() != Some(pin.as_str()) {
            *n.pin_failures.lock().unwrap().entry(ip).or_default() += 1;
            return StatusCode::UNAUTHORIZED.into_response();
        }
        n.pin_failures.lock().unwrap().remove(&ip);
    }
    if req
        .info
        .fingerprint
        .eq_ignore_ascii_case(&n.identity.fingerprint)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if busy(&n) {
        return StatusCode::CONFLICT.into_response();
    }
    if req.files.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    // Prove the sender owns the key it claims: call it back, pinned to that key.
    let proven = discovery::verify_claim(&n, &req.info, remote.ip()).await;
    let sender_verified = proven.is_some();
    let from = proven.unwrap_or_else(|| n.unverified_device(&req.info, remote.ip()));
    if from.trust == TrustLevel::Blocked {
        return StatusCode::FORBIDDEN.into_response();
    }
    let files: Vec<FileMeta> = req.files.into_values().collect();
    let total: u64 = files.iter().map(|f| f.size).sum();
    let auto = sender_verified
        && from.trust == TrustLevel::Verified
        && n.trust.lock().unwrap().auto_accept(&from.fingerprint);
    let decision = if auto {
        Decision::Accept { dest: None }
    } else {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            // Re-check: another request may have arrived during verification.
            if n.session.lock().unwrap().is_some() {
                return StatusCode::CONFLICT.into_response();
            }
            let mut pending = n.pending.lock().unwrap();
            if !pending.is_empty() {
                return StatusCode::CONFLICT.into_response();
            }
            pending.insert(request_id.clone(), Pending { tx, total });
        }
        n.emit(Event::IncomingRequest {
            request: IncomingRequest {
                request_id: request_id.clone(),
                from: from.clone(),
                files: files.clone(),
                total_size: total,
                fits_on_disk: has_space(&n.cfg.download_dir, total),
                sender_verified,
            },
        });
        let answer = tokio::time::timeout(n.cfg.request_timeout, rx).await;
        n.pending.lock().unwrap().remove(&request_id);
        match answer {
            Ok(Ok(d)) => d,
            _ => Decision::Decline,
        }
    };
    let Decision::Accept { dest } = decision else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let dest = dest.unwrap_or_else(|| n.cfg.download_dir.clone());
    match n.open_session(from, files, total, dest) {
        Some(resp) => Json(resp).into_response(),
        None => StatusCode::CONFLICT.into_response(),
    }
}

async fn upload(
    State(n): State<Arc<Inner>>,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> StatusCode {
    let (meta, dest, meter, cancel, why) = match n.claim_file(&q) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match receive::write_file(&meta, &dest, body, &meter, &cancel).await {
        Ok(path) => {
            n.file_finished(&q.session_id, &q.file_id, path);
            StatusCode::OK
        }
        Err(reason) => {
            // Stopped by the session ending: answer with the session's reason.
            let reason = match reason {
                FailReason::Cancelled => why.get().cloned().unwrap_or(reason),
                other => other,
            };
            let code = reason.status();
            n.fail_incoming(&q.session_id, reason);
            code
        }
    }
}

async fn cancel(State(n): State<Arc<Inner>>, Query(q): Query<SessionQuery>) -> StatusCode {
    n.fail_incoming(&q.session_id, FailReason::Cancelled);
    StatusCode::OK
}
