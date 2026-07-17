use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::{BodyExt, Full};
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Method, Request, Uri};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use protocol::{TunnelFrame, CHUNK_SIZE};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{HeaderName as WsHeaderName, HeaderValue as WsHeaderValue},
        Message as WsMessage,
    },
};

type HyperClient = Client<HttpConnector, Full<Bytes>>;

struct WsTunnel {
    tx: mpsc::UnboundedSender<WsMessage>,
}

static WS_TUNNELS: LazyLock<Arc<Mutex<HashMap<u64, WsTunnel>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

#[derive(Clone)]
pub struct ProxyHandler {
    port: u16,
    client: HyperClient,
}

impl ProxyHandler {
    pub fn new(port: u16) -> Self {
        let connector = HttpConnector::new();
        let client = Client::builder(TokioExecutor::new()).build(connector);
        Self { port, client }
    }

    /// Proxy an HTTP request to localhost. Returns the HTTP status code from localhost.
    pub async fn handle_http_request(
        &self,
        request_id: u64,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        tx: mpsc::Sender<TunnelFrame>,
    ) -> u16 {
        let uri = match format!("http://localhost:{}{}", self.port, path).parse::<Uri>() {
            Ok(u) => u,
            Err(e) => {
                let _ = tx
                    .send(TunnelFrame::HttpResponseHeader {
                        request_id,
                        status: 502,
                        headers: vec![("Content-Type".into(), "text/plain".into())],
                    })
                    .await;
                let _ = tx
                    .send(TunnelFrame::HttpResponseBody {
                        request_id,
                        chunk: format!("invalid uri: {e}").into_bytes(),
                        finished: true,
                    })
                    .await;
                return 502;
            }
        };

        let method = Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET);
        let mut req_builder = Request::builder().method(method).uri(uri);

        for (name, value) in &headers {
            if is_hop_by_hop(name) {
                continue;
            }
            if let (Ok(n), Ok(v)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                req_builder = req_builder.header(n, v);
            }
        }

        let request = match req_builder.body(Full::new(Bytes::from(body))) {
            Ok(r) => r,
            Err(e) => {
                send_error_response(request_id, 502, &format!("bad request: {e}"), &tx).await;
                return 502;
            }
        };

        let response = match self.client.request(request).await {
            Ok(r) => r,
            Err(e) => {
                send_error_response(request_id, 502, &format!("localhost unavailable: {e}"), &tx)
                    .await;
                return 502;
            }
        };

        let (parts, body) = response.into_parts();
        let status = parts.status.as_u16();
        let resp_headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .filter(|(k, _)| !is_hop_by_hop(k.as_str()))
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.as_str().to_string(), val.to_string()))
            })
            .collect();

        if tx
            .send(TunnelFrame::HttpResponseHeader {
                request_id,
                status,
                headers: resp_headers,
            })
            .await
            .is_err()
        {
            return status;
        }

        let mut body = body;
        loop {
            match body.frame().await {
                Some(Ok(frame)) => {
                    if let Ok(chunk) = frame.into_data() {
                        for chunk in chunk.chunks(CHUNK_SIZE) {
                            if tx
                                .send(TunnelFrame::HttpResponseBody {
                                    request_id,
                                    chunk: chunk.to_vec(),
                                    finished: false,
                                })
                                .await
                                .is_err()
                            {
                                return status;
                            }
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::error!(%e, "body read error");
                    break;
                }
                None => break,
            }
        }

        let _ = tx
            .send(TunnelFrame::HttpResponseBody {
                request_id,
                chunk: vec![],
                finished: true,
            })
            .await;

        status
    }

    pub async fn handle_ws_open(
        &self,
        request_id: u64,
        path: String,
        headers: Vec<(String, String)>,
        tx: mpsc::Sender<TunnelFrame>,
    ) {
        let url = format!("ws://127.0.0.1:{}{}", self.port, path);
        let mut request = match url.into_client_request() {
            Ok(request) => request,
            Err(e) => {
                tracing::error!(%e, "failed to build localhost websocket request");
                let _ = tx.send(TunnelFrame::WsClose { request_id }).await;
                return;
            }
        };

        for (name, value) in headers {
            if let (Ok(name), Ok(value)) = (
                WsHeaderName::from_bytes(name.as_bytes()),
                WsHeaderValue::from_str(&value),
            ) {
                request.headers_mut().append(name, value);
            }
        }

        let ws_result = connect_async(request).await;

        let (ws_stream, _) = match ws_result {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(%e, "failed to connect to localhost websocket");
                let _ = tx.send(TunnelFrame::WsClose { request_id }).await;
                return;
            }
        };

        let (mut ws_tx, mut ws_rx) = ws_stream.split();
        let (local_tx, mut local_rx) = mpsc::unbounded_channel();

        WS_TUNNELS
            .lock()
            .await
            .insert(request_id, WsTunnel { tx: local_tx });

        let tx_clone = tx.clone();
        let read_task = tokio::spawn(async move {
            while let Some(msg) = ws_rx.next().await {
                let frame = match msg {
                    Ok(WsMessage::Text(text)) => TunnelFrame::WsFrame {
                        request_id,
                        data: text.as_bytes().to_vec(),
                        opcode: 1,
                    },
                    Ok(WsMessage::Binary(data)) => TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 2,
                    },
                    Ok(WsMessage::Ping(data)) => TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 9,
                    },
                    Ok(WsMessage::Pong(data)) => TunnelFrame::WsFrame {
                        request_id,
                        data: data.to_vec(),
                        opcode: 10,
                    },
                    Ok(WsMessage::Close(_)) => TunnelFrame::WsClose { request_id },
                    Err(_) => TunnelFrame::WsClose { request_id },
                    _ => continue,
                };
                let is_close = matches!(&frame, TunnelFrame::WsClose { .. });
                if tx_clone.send(frame).await.is_err() {
                    break;
                }
                if is_close {
                    break;
                }
            }
        });

        while let Some(msg) = local_rx.recv().await {
            let result = match &msg {
                WsMessage::Text(t) => ws_tx.send(WsMessage::Text(t.clone())).await,
                WsMessage::Binary(b) => ws_tx.send(WsMessage::Binary(b.clone())).await,
                WsMessage::Ping(d) => ws_tx.send(WsMessage::Ping(d.clone())).await,
                WsMessage::Pong(d) => ws_tx.send(WsMessage::Pong(d.clone())).await,
                WsMessage::Close(c) => ws_tx.send(WsMessage::Close(c.clone())).await,
                _ => Ok(()),
            };
            if result.is_err() {
                break;
            }
        }

        read_task.abort();
        WS_TUNNELS.lock().await.remove(&request_id);
    }

    pub async fn forward_ws_frame(&self, request_id: u64, data: Vec<u8>, opcode: u8) {
        let tunnels = WS_TUNNELS.lock().await;
        if let Some(tunnel) = tunnels.get(&request_id) {
            let msg = match opcode {
                1 => WsMessage::Text(String::from_utf8_lossy(&data).into_owned().into()),
                2 => WsMessage::Binary(data.into()),
                8 => WsMessage::Close(None),
                9 => WsMessage::Ping(data.into()),
                10 => WsMessage::Pong(data.into()),
                _ => WsMessage::Binary(data.into()),
            };
            let _ = tunnel.tx.send(msg);
        }
    }

    pub async fn close_ws(&self, request_id: u64) {
        let mut tunnels = WS_TUNNELS.lock().await;
        if let Some(tunnel) = tunnels.remove(&request_id) {
            let _ = tunnel.tx.send(WsMessage::Close(None));
        }
    }
}

async fn send_error_response(
    request_id: u64,
    status: u16,
    message: &str,
    tx: &mpsc::Sender<TunnelFrame>,
) {
    let _ = tx
        .send(TunnelFrame::HttpResponseHeader {
            request_id,
            status,
            headers: vec![("Content-Type".into(), "text/plain".into())],
        })
        .await;
    let _ = tx
        .send(TunnelFrame::HttpResponseBody {
            request_id,
            chunk: message.as_bytes().to_vec(),
            finished: true,
        })
        .await;
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "connection" | "transfer-encoding" | "upgrade" | "keep-alive" | "proxy-connection"
    )
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::ProxyHandler;
    use protocol::TunnelFrame;

    #[tokio::test]
    async fn preserves_small_streaming_response_chunks_until_end_of_stream() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind localhost test server");
        let port = listener.local_addr().expect("test server address").port();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept proxied request");
            let mut request_buffer = [0; 1024];
            let _ = socket
                .read(&mut request_buffer)
                .await
                .expect("read request");
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n3\r\none\r\n",
                )
                .await
                .expect("write first chunk");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            socket
                .write_all(b"3\r\ntwo\r\n0\r\n\r\n")
                .await
                .expect("write final chunks");
        });

        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        ProxyHandler::new(port)
            .handle_http_request(7, "GET".into(), "/events".into(), vec![], vec![], tx)
            .await;

        let mut body_frames = Vec::new();
        while let Some(frame) = rx.recv().await {
            if let TunnelFrame::HttpResponseBody {
                chunk, finished, ..
            } = frame
            {
                body_frames.push((chunk, finished));
            }
        }

        assert_eq!(
            body_frames,
            vec![
                (b"one".to_vec(), false),
                (b"two".to_vec(), false),
                (Vec::new(), true),
            ]
        );
        server.await.expect("test server task");
    }
}
