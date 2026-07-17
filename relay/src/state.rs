use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, Mutex};

pub fn next_request_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct PendingResponse {
    pub subdomain: String,
    pub header_tx: Option<oneshot::Sender<ResponseStart>>,
    pub body_tx: mpsc::Sender<(Bytes, bool)>,
}

pub enum ResponseStart {
    Response {
        status: u16,
        headers: Vec<(String, String)>,
    },
    TunnelClosed,
}

static PENDING_RESPONSES: LazyLock<Arc<Mutex<HashMap<u64, PendingResponse>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub async fn register_pending_response(request_id: u64, pending: PendingResponse) {
    PENDING_RESPONSES.lock().await.insert(request_id, pending);
}

pub async fn remove_pending_response(request_id: u64) {
    PENDING_RESPONSES.lock().await.remove(&request_id);
}

pub async fn dispatch_response_header(
    request_id: u64,
    status: u16,
    headers: Vec<(String, String)>,
) {
    let mut map = PENDING_RESPONSES.lock().await;
    if let Some(pending) = map.get_mut(&request_id) {
        if let Some(tx) = pending.header_tx.take() {
            let _ = tx.send(ResponseStart::Response { status, headers });
        }
    }
}

pub async fn dispatch_response_body(request_id: u64, chunk: Bytes, finished: bool) {
    let body_tx = {
        let map = PENDING_RESPONSES.lock().await;
        map.get(&request_id).map(|pending| pending.body_tx.clone())
    };

    if let Some(body_tx) = body_tx {
        let _ = body_tx.send((chunk, finished)).await;
    }

    if finished {
        PENDING_RESPONSES.lock().await.remove(&request_id);
    }
}

/// Fail all requests that are waiting for a tunnel which has disconnected.
pub async fn abort_pending_for_session(subdomain: &str) {
    let pending = {
        let mut map = PENDING_RESPONSES.lock().await;
        let request_ids: Vec<u64> = map
            .iter()
            .filter_map(|(request_id, pending)| {
                (pending.subdomain == subdomain).then_some(*request_id)
            })
            .collect();

        request_ids
            .into_iter()
            .filter_map(|request_id| map.remove(&request_id))
            .collect::<Vec<_>>()
    };

    for mut response in pending {
        if let Some(header_tx) = response.header_tx.take() {
            let _ = header_tx.send(ResponseStart::TunnelClosed);
        }
    }
}
