use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use protocol::TunnelFrame;
use tokio::sync::{mpsc, Mutex};

struct WsProxy {
    browser_tx: mpsc::UnboundedSender<Message>,
}

static WS_PROXIES: LazyLock<Arc<Mutex<HashMap<u64, WsProxy>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

pub fn extract_subdomain(host: &str, domain: &str) -> Option<String> {
    let host = host.split(':').next().unwrap_or(host);
    let suffix = format!(".{domain}");
    if host.ends_with(&suffix) {
        let sub = host.strip_suffix(&suffix)?.to_string();
        if !sub.is_empty() && sub != "relay" {
            return Some(sub);
        }
    }
    None
}

pub fn is_websocket_upgrade(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(axum::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

pub async fn register_browser_ws(request_id: u64, tx: mpsc::UnboundedSender<Message>) {
    WS_PROXIES
        .lock()
        .await
        .insert(request_id, WsProxy { browser_tx: tx });
}

pub async fn forward_ws_to_browser(request_id: u64, data: Vec<u8>, opcode: u8) {
    let proxies = WS_PROXIES.lock().await;
    if let Some(proxy) = proxies.get(&request_id) {
        let msg = match opcode {
            1 => Message::Text(String::from_utf8_lossy(&data).into_owned().into()),
            2 => Message::Binary(data.into()),
            8 => Message::Close(None),
            9 => Message::Ping(data.into()),
            10 => Message::Pong(data.into()),
            _ => Message::Binary(data.into()),
        };
        let _ = proxy.browser_tx.send(msg);
    }
}

pub async fn close_browser_ws(request_id: u64) {
    let mut proxies = WS_PROXIES.lock().await;
    if let Some(proxy) = proxies.remove(&request_id) {
        let _ = proxy.browser_tx.send(Message::Close(None));
    }
}

pub async fn run_browser_ws_proxy(
    socket: WebSocket,
    request_id: u64,
    client_tx: mpsc::Sender<TunnelFrame>,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (browser_tx, mut browser_rx) = mpsc::unbounded_channel();
    register_browser_ws(request_id, browser_tx).await;

    let client_tx_clone = client_tx.clone();
    let read_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let frame = TunnelFrame::WsFrame {
                        request_id,
                        data: text.as_bytes().to_vec(),
                        opcode: 1,
                    };
                    if client_tx_clone.send(frame).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    let frame = TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 2,
                    };
                    if client_tx_clone.send(frame).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Ping(data)) => {
                    let frame = TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 9,
                    };
                    if client_tx_clone.send(frame).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Pong(data)) => {
                    let frame = TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 10,
                    };
                    if client_tx_clone.send(frame).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    let _ = client_tx_clone
                        .send(TunnelFrame::WsClose { request_id })
                        .await;
                    break;
                }
                Err(_) => break,
            }
        }
        close_browser_ws(request_id).await;
    });

    while let Some(msg) = browser_rx.recv().await {
        if ws_tx.send(msg).await.is_err() {
            break;
        }
    }

    read_task.abort();
    close_browser_ws(request_id).await;
}
