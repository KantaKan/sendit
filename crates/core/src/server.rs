use crate::{node::Inner, protocol::*};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    routing::post,
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};

pub(crate) fn router(n: Arc<Inner>) -> Router {
    Router::new()
        .route(&format!("{API}/register"), post(register))
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
