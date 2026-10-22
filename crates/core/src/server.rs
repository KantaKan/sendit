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
    routing::post,
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) fn router(n: Arc<Inner>) -> Router {
    Router::new()
        .route(&format!("{API}/register"), post(register))
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
    if info.protocol == Scheme::Https {
        n.see_device(&info, remote.ip());
    }
    Json(n.my_info())
}

async fn prepare_upload(
    State(n): State<Arc<Inner>>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Query(q): Query<PinQuery>,
    Json(req): Json<PrepareUploadRequest>,
) -> Response {
    if let Some(pin) = &n.cfg.pin {
        if q.pin.as_deref() != Some(pin.as_str()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    let Some(from) = n.see_device(&req.info, remote.ip()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if from.trust == TrustLevel::Blocked {
        return StatusCode::FORBIDDEN.into_response();
    }
    if n.session.lock().unwrap().is_some() {
        return StatusCode::CONFLICT.into_response();
    }
    if req.files.is_empty() {
        return StatusCode::NO_CONTENT.into_response();
    }
    // Prove the sender owns the key it claims: call it back, pinned to that key.
    let sender_verified = req.info.protocol == Scheme::Https
        && discovery::register_with(
            &n,
            SocketAddr::new(remote.ip(), req.info.port),
            Some(&req.info.fingerprint),
        )
        .await
        .is_ok();
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
        n.pending
            .lock()
            .unwrap()
            .insert(request_id.clone(), Pending { tx, total });
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
    let (meta, dest, meter, cancel) = match n.claim_file(&q) {
        Ok(c) => c,
        Err(code) => return code,
    };
    match receive::write_file(&meta, &dest, body, &meter, &cancel).await {
        Ok(path) => {
            n.file_finished(&q.session_id, &q.file_id, path);
            StatusCode::OK
        }
        Err(reason) => {
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
