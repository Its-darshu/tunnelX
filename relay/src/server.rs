use std::time::Instant;

use axum::{
    body::Body,
    extract::{ws::Message, FromRequest, State, WebSocketUpgrade},
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use protocol::{decode_frame, encode_frame, TunnelFrame};
use shared::RelayConfig;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::crypto;
use crate::router::{self, extract_subdomain, is_websocket_upgrade, run_browser_ws_proxy};
use crate::session::{insert_session, remove_session, Session};
use crate::state::{self, PendingResponse};
use crate::url_gen::generate_subdomain;

#[derive(Clone)]
pub struct AppState {
    pub domain: String,
    pub public_url_scheme: &'static str,
    pub path_routing: bool,
}

pub async fn run(config: RelayConfig) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState {
        domain: config.domain.clone(),
        public_url_scheme: if config.tls_enabled() {
            "https"
        } else {
            "http"
        },
        path_routing: config.path_routing,
    };

    let app = app(state);

    let bind = config.bind_addr.parse()?;

    if config.tls_enabled() {
        let cert = config.tls_cert_path.as_ref().unwrap();
        let key = config.tls_key_path.as_ref().unwrap();
        let tls = RustlsConfig::from_pem_file(cert, key).await?;
        tracing::info!(%bind, "listening with TLS");
        axum_server::bind_rustls(bind, tls)
            .serve(app.into_make_service())
            .await?;
    } else {
        tracing::warn!(%bind, "listening without TLS (development mode)");
        let listener = tokio::net::TcpListener::bind(bind).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async {
                let count = crate::session::session_count();
                tracing::debug!(active_sessions = count, "health check");
                StatusCode::NO_CONTENT
            }),
        )
        .route("/tunnel", any(handle_tunnel_upgrade))
        .fallback(any(handle_public_request))
        .with_state(state)
}

async fn handle_tunnel_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_client_connection(socket, state))
}

async fn handle_client_connection(socket: axum::extract::ws::WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (client_tx, mut client_rx) = mpsc::channel::<TunnelFrame>(256);

    let (register_token, requested_subdomain, duration_secs) = loop {
        match ws_rx.next().await {
            Some(Ok(Message::Binary(data))) => match decode_frame(&data) {
                Ok(TunnelFrame::Register {
                    token: reg_token,
                    version,
                    requested_subdomain,
                    duration_secs,
                }) => {
                    if version != protocol::PROTOCOL_VERSION {
                        let _ = ws_tx
                            .send(Message::Binary(
                                encode_frame(&TunnelFrame::error(
                                    426,
                                    "unsupported protocol version",
                                ))
                                .unwrap()
                                .into(),
                            ))
                            .await;
                        return;
                    }
                    if reg_token.len() < shared::MIN_TOKEN_LENGTH {
                        let _ = ws_tx
                            .send(Message::Binary(
                                encode_frame(&TunnelFrame::error(
                                    400,
                                    "invalid token: too short",
                                ))
                                .unwrap()
                                .into(),
                            ))
                            .await;
                        return;
                    }
                    break (reg_token, requested_subdomain, duration_secs);
                }
                Ok(other) => {
                    tracing::warn!(?other, "expected Register frame");
                    return;
                }
                Err(e) => {
                    tracing::error!(%e, "failed to decode register frame");
                    return;
                }
            },
            Some(Ok(Message::Close(_))) | None => return,
            Some(Err(e)) => {
                tracing::error!(%e, "websocket error during registration");
                return;
            }
            _ => continue,
        }
    };

    // Resolve subdomain: use requested if valid and available, otherwise generate.
    let subdomain = if let Some(ref req) = requested_subdomain {
        let clean = req.trim().to_lowercase();
        let is_valid = !clean.is_empty()
            && clean.len() <= 63
            && clean
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !clean.starts_with('-')
            && !clean.ends_with('-');

        if is_valid && crate::session::get_session(&clean).is_none() {
            clean
        } else if !is_valid {
            let _ = ws_tx
                .send(Message::Binary(
                    encode_frame(&TunnelFrame::error(
                        400,
                        "invalid subdomain: use lowercase letters, numbers, and hyphens",
                    ))
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        } else {
            let _ = ws_tx
                .send(Message::Binary(
                    encode_frame(&TunnelFrame::error(
                        409,
                        format!("subdomain '{clean}' is already in use"),
                    ))
                    .unwrap()
                    .into(),
                ))
                .await;
            return;
        }
    } else {
        generate_subdomain()
    };

    let public_url = if state.path_routing {
        format!(
            "{}://{}/t/{}",
            state.public_url_scheme, state.domain, subdomain
        )
    } else {
        format!(
            "{}://{}.{}",
            state.public_url_scheme, subdomain, state.domain
        )
    };

    let token_hash = crypto::hash_token(&register_token);
    let session = Session {
        id: Uuid::new_v4(),
        subdomain: subdomain.clone(),
        token_hash,
        client_tx: client_tx.clone(),
        created_at: Instant::now(),
        last_heartbeat: Instant::now(),
        last_request: Instant::now(),
        request_count: 0,
        duration_secs,
        client_addr: None,
    };
    insert_session(session);

    let registered = TunnelFrame::Registered {
        subdomain: subdomain.clone(),
        public_url: public_url.clone(),
        expires_in_secs: duration_secs,
    };
    if ws_tx
        .send(Message::Binary(encode_frame(&registered).unwrap().into()))
        .await
        .is_err()
    {
        remove_session(&subdomain);
        return;
    }

    tracing::info!(subdomain, %public_url, ?duration_secs, "client connected");

    let subdomain_for_cleanup = subdomain.clone();
    let client_tx_for_reader = client_tx.clone();

    let write_task = tokio::spawn(async move {
        while let Some(frame) = client_rx.recv().await {
            if ws_tx
                .send(Message::Binary(encode_frame(&frame).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Binary(data)) => match decode_frame(&data) {
                Ok(TunnelFrame::Heartbeat) => {
                    if let Some(mut entry) = crate::session::sessions().get_mut(&subdomain) {
                        entry.touch_heartbeat();
                    }
                    let _ = client_tx_for_reader.send(TunnelFrame::HeartbeatAck).await;
                }
                Ok(TunnelFrame::Disconnect) => break,
                Ok(TunnelFrame::HttpResponseHeader {
                    request_id,
                    status,
                    headers,
                }) => {
                    state::dispatch_response_header(request_id, status, headers).await;
                }
                Ok(TunnelFrame::HttpResponseBody {
                    request_id,
                    chunk,
                    finished,
                }) => {
                    state::dispatch_response_body(request_id, Bytes::from(chunk), finished).await;
                }
                Ok(TunnelFrame::WsFrame {
                    request_id,
                    data,
                    opcode,
                }) => {
                    router::forward_ws_to_browser(request_id, data, opcode).await;
                }
                Ok(TunnelFrame::WsClose { request_id }) => {
                    router::close_browser_ws(request_id).await;
                }
                Ok(TunnelFrame::Error { code, message }) => {
                    tracing::warn!(code, message, "client reported error");
                }
                Ok(other) => {
                    tracing::debug!(?other, "ignored frame from client");
                }
                Err(e) => tracing::error!(%e, "decode error"),
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {
                let _ = client_tx_for_reader.send(TunnelFrame::HeartbeatAck).await;
            }
            Err(e) => {
                tracing::error!(%e, "websocket read error");
                break;
            }
            _ => {}
        }
    }

    write_task.abort();
    state::abort_pending_for_session(&subdomain_for_cleanup).await;
    remove_session(&subdomain_for_cleanup);
    tracing::info!(subdomain = subdomain_for_cleanup, "client disconnected");
}

async fn handle_public_request(State(state): State<AppState>, mut req: Request<Body>) -> Response {
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();

    let mut path_subdomain = None;
    let mut rewrite_to: Option<String> = None;

    // 1. Explicit path-based routing: /t/<subdomain>/...
    if path.starts_with("/t/") {
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() >= 3 && !segments[2].is_empty() {
            let sub = segments[2].to_string();

            // If it is exactly /t/subdomain without trailing slash, redirect to /t/subdomain/
            if segments.len() == 3 && !path.ends_with('/') {
                let redirect_url = format!("{}/{}", path, query);
                return axum::response::Redirect::temporary(&redirect_url).into_response();
            }

            path_subdomain = Some(sub);

            // Rewrite path to omit /t/subdomain and preserve rest of path
            let rest_path = segments[3..].join("/");
            let new_path = if rest_path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", rest_path)
            };
            rewrite_to = Some(format!("{}{}", new_path, query));
        }
    } else {
        // 2. Absolute-path asset fallback (e.g. Vite emits /@vite/client, /src/main.tsx,
        //    /@react-refresh — these bypass the /t/<sub>/ prefix). Recover the tunnel from
        //    the Referer header, which for same-origin requests carries the full path.
        if let Some(sub) = req
            .headers()
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok())
            .and_then(subdomain_from_referer)
        {
            tracing::debug!(referer_subdomain = %sub, path = %path, "recovered tunnel from referer");
            path_subdomain = Some(sub);
            // Forward the original absolute path unchanged.
        }
    }

    // Apply the URI rewrite for explicit path-based requests.
    if let Some(new_uri_str) = rewrite_to {
        match axum::http::Uri::builder()
            .path_and_query(new_uri_str.as_str())
            .build()
        {
            Ok(uri) => {
                tracing::debug!(uri = %uri, "rewrote request URI");
                *req.uri_mut() = uri;
            }
            Err(e) => {
                tracing::warn!(error = %e, uri = %new_uri_str, "failed to build rewritten URI");
            }
        }
    }

    let subdomain = match path_subdomain {
        Some(s) => s,
        None => {
            let host = req
                .headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            match extract_subdomain(&host, &state.domain) {
                Some(s) => s,
                None => return (StatusCode::NOT_FOUND, "invalid host or path-based tunnel").into_response(),
            }
        }
    };

    let session = match crate::session::get_session(&subdomain) {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, "tunnel not found").into_response(),
    };

    let client_tx = session.client_tx.clone();
    drop(session);

    if is_websocket_upgrade(req.headers()) {
        let (parts, body) = req.into_parts();

        let path = parts
            .uri
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| parts.uri.path().to_string());

        let headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .filter(|(k, _)| {
                let name = k.as_str().to_lowercase();
                !matches!(
                    name.as_str(),
                    "host"
                        | "connection"
                        | "upgrade"
                        | "sec-websocket-key"
                        | "sec-websocket-version"
                        | "sec-websocket-extensions"
                )
            })
            .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
            .take(shared::MAX_HEADERS_PER_REQUEST)
            .collect();

        let req = Request::from_parts(parts, body);
        match WebSocketUpgrade::from_request(req, &()).await {
            Ok(ws) => return handle_browser_websocket(ws, path, headers, client_tx).await,
            Err(rejection) => return rejection.into_response(),
        }
    }

    proxy_http_request(req, subdomain, client_tx).await
}

async fn handle_browser_websocket(
    ws: WebSocketUpgrade,
    path: String,
    headers: Vec<(String, String)>,
    client_tx: mpsc::Sender<TunnelFrame>,
) -> Response {
    let request_id = state::next_request_id();

    let ws_open = TunnelFrame::WsOpen {
        request_id,
        path,
        headers,
    };

    if client_tx.send(ws_open).await.is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "tunnel client disconnected",
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| run_browser_ws_proxy(socket, request_id, client_tx))
        .into_response()
}

async fn proxy_http_request(
    req: Request<Body>,
    subdomain: String,
    client_tx: mpsc::Sender<TunnelFrame>,
) -> Response {
    let request_id = state::next_request_id();
    let (parts, body) = req.into_parts();

    let body_bytes = match axum::body::to_bytes(body, shared::MAX_BODY_SIZE).await {
        Ok(b) => b.to_vec(),
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "body too large").into_response(),
    };

    let path = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| parts.uri.path().to_string());

    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .filter(|(k, _)| {
            let name = k.as_str().to_lowercase();
            !matches!(
                name.as_str(),
                "host" | "connection" | "transfer-encoding" | "upgrade" | "keep-alive"
            )
        })
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .take(shared::MAX_HEADERS_PER_REQUEST)
        .collect();

    let frame = TunnelFrame::HttpRequest {
        request_id,
        method: parts.method.to_string(),
        path,
        headers,
        body: body_bytes,
    };

    let (header_tx, header_rx) = oneshot::channel();
    let (body_tx, mut body_rx) = mpsc::channel::<(Bytes, bool)>(64);

    state::register_pending_response(
        request_id,
        PendingResponse {
            subdomain,
            header_tx: Some(header_tx),
            body_tx,
        },
    )
    .await;

    if client_tx.send(frame).await.is_err() {
        state::remove_pending_response(request_id).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "tunnel client disconnected",
        )
            .into_response();
    }

    let header_result = tokio::time::timeout(std::time::Duration::from_secs(30), header_rx).await;

    let (status, resp_headers) = match header_result {
        Ok(Ok(state::ResponseStart::Response { status, headers })) => (status, headers),
        Ok(Ok(state::ResponseStart::TunnelClosed)) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "tunnel client disconnected",
            )
                .into_response();
        }
        Ok(Err(_)) | Err(_) => {
            state::remove_pending_response(request_id).await;
            return (StatusCode::BAD_GATEWAY, "localhost unavailable").into_response();
        }
    };

    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);

    let (stream_tx, stream_rx) = mpsc::channel::<Result<Bytes, std::convert::Infallible>>(64);

    tokio::spawn(async move {
        while let Some((chunk, finished)) = body_rx.recv().await {
            if stream_tx.send(Ok(chunk)).await.is_err() {
                break;
            }
            if finished {
                break;
            }
        }
    });

    let body_stream = Body::from_stream(ReceiverStream::new(stream_rx));

    let mut builder = Response::builder().status(status);
    for (name, value) in resp_headers {
        if name.len() > shared::MAX_HEADER_NAME_LEN || value.len() > shared::MAX_HEADER_VALUE_LEN {
            tracing::warn!(
                header_name_len = name.len(),
                header_value_len = value.len(),
                "rejecting oversized header"
            );
            continue;
        }
        if let (Ok(name), Ok(val)) = (
            header::HeaderName::from_bytes(name.as_bytes()),
            header::HeaderValue::from_str(&value),
        ) {
            builder = builder.header(name, val);
        }
    }

    builder.body(body_stream).unwrap_or_else(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to build response",
        )
            .into_response()
    })
}

/// Extract the tunnel subdomain from a Referer header value.
///
/// The Referer of a path-routed page looks like
/// `https://tunnelx.darsha.dev/t/arctic-bamboo-49/`. When the browser requests an
/// absolute-path asset (e.g. Vite's `/@vite/client`), the request itself lacks the
/// `/t/<sub>/` prefix, but the Referer still carries it, letting us route the asset.
fn subdomain_from_referer(referer: &str) -> Option<String> {
    let uri: axum::http::Uri = referer.parse().ok()?;
    let path = uri.path();
    if path.starts_with("/t/") {
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() >= 3 && !segments[2].is_empty() {
            return Some(segments[2].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use protocol::{decode_frame, encode_frame, TunnelFrame, PROTOCOL_VERSION};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{
            client::IntoClientRequest,
            http::{header::HOST, HeaderValue},
            Message as ClientWsMessage,
        },
    };

    use super::{app, subdomain_from_referer, AppState};

    #[test]
    fn extracts_subdomain_from_path_routed_referer() {
        assert_eq!(
            subdomain_from_referer("https://tunnelx.darsha.dev/t/arctic-bamboo-49/"),
            Some("arctic-bamboo-49".to_string())
        );
        assert_eq!(
            subdomain_from_referer("https://tunnelx.darsha.dev/t/epic-sun-43/some/page"),
            Some("epic-sun-43".to_string())
        );
    }

    #[test]
    fn ignores_referer_without_tunnel_prefix() {
        assert_eq!(subdomain_from_referer("https://tunnelx.darsha.dev/"), None);
        assert_eq!(subdomain_from_referer("https://example.com/t/"), None);
        assert_eq!(subdomain_from_referer("not a url"), None);
    }

    async fn read_http_response(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut response = Vec::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buffer))
                .await
                .expect("timed out reading response")
                .expect("read response");
            assert_ne!(bytes_read, 0, "connection closed before complete response");
            response.extend_from_slice(&buffer[..bytes_read]);

            let Some(headers_end) = response.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = std::str::from_utf8(&response[..headers_end]).expect("utf-8 headers");
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .or_else(|| {
                    headers
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                })
                .expect("content length")
                .parse::<usize>()
                .expect("numeric content length");

            if response.len() >= headers_end + 4 + content_length {
                return response;
            }
        }
    }

    async fn next_tunnel_frame(
        stream: &mut futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    ) -> TunnelFrame {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .expect("timed out waiting for tunnel frame")
                .expect("tunnel closed")
                .expect("valid tunnel websocket message");
            if let ClientWsMessage::Binary(data) = message {
                return decode_frame(&data).expect("valid tunnel frame");
            }
        }
    }

    #[tokio::test]
    async fn proxies_post_websockets_and_invalidates_on_disconnect() {
        crate::session::sessions().clear();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind relay");
        let address = listener.local_addr().expect("relay address");
        let relay = tokio::spawn(async move {
            axum::serve(
                listener,
                app(AppState {
                    domain: "tunnel.test".into(),
                    public_url_scheme: "http",
                    path_routing: false,
                }),
            )
            .await
            .expect("serve relay");
        });

        let tunnel_url = format!("ws://{address}/tunnel");
        let (tunnel, _) = connect_async(tunnel_url).await.expect("connect tunnel");
        let (mut tunnel_tx, mut tunnel_rx) = tunnel.split();
        tunnel_tx
            .send(ClientWsMessage::Binary(
                encode_frame(&TunnelFrame::Register {
                    token: "test-token".into(),
                    version: PROTOCOL_VERSION,
                    requested_subdomain: None,
                    duration_secs: None,
                })
                .expect("encode register")
                .into(),
            ))
            .await
            .expect("register tunnel");

        let subdomain = match next_tunnel_frame(&mut tunnel_rx).await {
            TunnelFrame::Registered { subdomain, .. } => subdomain,
            frame => panic!("expected registration, got {frame:?}"),
        };
        let host = format!("{subdomain}.tunnel.test");

        let http_address = address;
        let http_host = host.clone();
        let request = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(http_address)
                .await
                .expect("connect browser");
            stream
                .write_all(
                    format!(
                        "POST /api/items?draft=true HTTP/1.1\r\nHost: {http_host}\r\nContent-Type: application/json\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{{\"name\":\"tux\"}}"
                    )
                    .as_bytes(),
                )
                .await
                .expect("write browser request");
            read_http_response(&mut stream).await
        });

        let request_frame = next_tunnel_frame(&mut tunnel_rx).await;
        let request_id = match request_frame {
            TunnelFrame::HttpRequest {
                request_id,
                method,
                path,
                headers,
                body,
            } => {
                assert_eq!(method, "POST");
                assert_eq!(path, "/api/items?draft=true");
                assert!(headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("content-type") && value == "application/json"
                }));
                assert_eq!(body, br#"{"name":"tux"}"#);
                request_id
            }
            frame => panic!("expected HTTP request, got {frame:?}"),
        };

        for frame in [
            TunnelFrame::HttpResponseHeader {
                request_id,
                status: 201,
                headers: vec![
                    ("content-type".into(), "application/json".into()),
                    ("content-length".into(), "16".into()),
                ],
            },
            TunnelFrame::HttpResponseBody {
                request_id,
                chunk: br#"{"created":"#.to_vec(),
                finished: false,
            },
            TunnelFrame::HttpResponseBody {
                request_id,
                chunk: b"true}".to_vec(),
                finished: false,
            },
            TunnelFrame::HttpResponseBody {
                request_id,
                chunk: vec![],
                finished: true,
            },
        ] {
            tunnel_tx
                .send(ClientWsMessage::Binary(
                    encode_frame(&frame).expect("encode response frame").into(),
                ))
                .await
                .expect("respond through tunnel");
        }

        let response = String::from_utf8(request.await.expect("browser task"))
            .expect("utf-8 browser response");
        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.ends_with(r#"{"created":true}"#), "{response:?}");

        let mut browser_ws_request = format!("ws://{address}/socket")
            .into_client_request()
            .expect("build browser websocket request");
        browser_ws_request
            .headers_mut()
            .insert(HOST, HeaderValue::from_str(&host).expect("valid host"));
        let browser_ws_task = tokio::spawn(async move {
            connect_async(browser_ws_request)
                .await
                .expect("connect browser websocket")
                .0
        });

        let websocket_id = match next_tunnel_frame(&mut tunnel_rx).await {
            TunnelFrame::WsOpen {
                request_id, path, ..
            } => {
                assert_eq!(path, "/socket");
                request_id
            }
            frame => panic!("expected websocket open, got {frame:?}"),
        };
        let mut browser_ws = browser_ws_task.await.expect("browser websocket task");

        browser_ws
            .send(ClientWsMessage::Text("from-browser".into()))
            .await
            .expect("send browser websocket frame");
        match next_tunnel_frame(&mut tunnel_rx).await {
            TunnelFrame::WsFrame {
                request_id,
                data,
                opcode,
            } => {
                assert_eq!(request_id, websocket_id);
                assert_eq!(data, b"from-browser");
                assert_eq!(opcode, 1);
            }
            frame => panic!("expected websocket frame, got {frame:?}"),
        }

        tunnel_tx
            .send(ClientWsMessage::Binary(
                encode_frame(&TunnelFrame::WsFrame {
                    request_id: websocket_id,
                    data: b"from-localhost".to_vec(),
                    opcode: 1,
                })
                .expect("encode websocket frame")
                .into(),
            ))
            .await
            .expect("send websocket frame to browser");
        match tokio::time::timeout(Duration::from_secs(2), browser_ws.next())
            .await
            .expect("timed out waiting for browser websocket frame")
            .expect("browser websocket closed")
            .expect("valid browser websocket frame")
        {
            ClientWsMessage::Text(text) => assert_eq!(text, "from-localhost"),
            message => panic!("expected browser text frame, got {message:?}"),
        }
        let _ = browser_ws.close(None).await;

        tunnel_tx
            .send(ClientWsMessage::Close(None))
            .await
            .expect("close tunnel");
        drop(tunnel_tx);

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if crate::session::get_session(&subdomain).is_none() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("session should be removed after disconnect");

        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("connect browser after disconnect");
        stream
            .write_all(
                format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
            )
            .await
            .expect("write post-disconnect request");
        let response = String::from_utf8(read_http_response(&mut stream).await)
            .expect("utf-8 not found response");
        assert!(response.starts_with("HTTP/1.1 404"));

        relay.abort();
        crate::session::sessions().clear();
    }
}
