//! End-to-end integration tests for TunnelX.
//!
//! Each test spawns the relay binary as a child process on a random port,
//! then acts as both "tunnel client" and "browser" to exercise the full path.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use protocol::{decode_frame, encode_frame, TunnelFrame, PROTOCOL_VERSION};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{header::HOST, HeaderValue},
        Message,
    },
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve the relay binary path from the cargo target directory.
fn relay_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe()
        .expect("current exe")
        .parent()
        .expect("deps dir")
        .parent()
        .expect("target/debug")
        .to_path_buf();
    path.push("relay");
    if !path.exists() {
        // Try without debug profile nesting
        panic!(
            "relay binary not found at {path:?}. Run `cargo build` first."
        );
    }
    path
}

/// Find a free TCP port.
async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

async fn start_relay_with_env(port: u16, path_routing: bool) -> tokio::process::Child {
    let addr = format!("127.0.0.1:{port}");
    let mut cmd = tokio::process::Command::new(relay_bin());
    cmd.env("RELAY_BIND", &addr)
        .env("RELAY_DOMAIN", "tunnel.test")
        .env("RUST_LOG", "warn")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    if path_routing {
        cmd.env("PATH_ROUTING", "true");
    }

    let child = cmd.spawn().expect("spawn relay");

    // Wait for the relay to accept connections.
    let addr: std::net::SocketAddr = addr.parse().unwrap();
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return child;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("relay did not start within 5 seconds");
}

/// Start the relay on a given port. Returns the child process.
async fn start_relay(port: u16) -> tokio::process::Child {
    start_relay_with_env(port, false).await
}

type WsTx = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;
type WsRx = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
>;

/// Register a tunnel client and return (subdomain, ws_tx, ws_rx).
async fn register_tunnel(port: u16) -> (String, WsTx, WsRx) {
    let url = format!("ws://127.0.0.1:{port}/tunnel");
    let (ws, _) = connect_async(url).await.expect("connect to relay");
    let (mut tx, mut rx) = ws.split();

    let register = TunnelFrame::Register {
        token: "integration-test-token".into(),
        version: PROTOCOL_VERSION,
        requested_subdomain: None,
        duration_secs: None,
    };
    tx.send(Message::Binary(encode_frame(&register).unwrap().into()))
        .await
        .unwrap();

    let subdomain = loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), rx.next())
            .await
            .expect("timeout waiting for Registered")
            .expect("stream ended")
            .expect("ws error");
        if let Message::Binary(data) = msg {
            if let Ok(TunnelFrame::Registered { subdomain, .. }) = decode_frame(&data) {
                break subdomain;
            }
        }
    };

    (subdomain, tx, rx)
}

/// Read the next TunnelFrame from the tunnel WS stream.
async fn next_frame(rx: &mut WsRx) -> TunnelFrame {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), rx.next())
            .await
            .expect("timeout")
            .expect("stream closed")
            .expect("ws error");
        if let Message::Binary(data) = msg {
            return decode_frame(&data).expect("decode frame");
        }
    }
}

/// Send a raw HTTP/1.1 request over TCP and return the full response.
async fn raw_http(port: u16, request: &str) -> String {
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut tmp))
            .await
            .expect("read timeout")
            .expect("read error");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);

        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&buf[..pos]).unwrap_or("");
            if let Some(cl) = headers.lines().find_map(|l| {
                let lower = l.to_lowercase();
                lower
                    .strip_prefix("content-length: ")
                    .map(|v| v.to_string())
            }) {
                let cl: usize = cl.trim().parse().unwrap_or(0);
                if buf.len() >= pos + 4 + cl {
                    break;
                }
            }
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// HTTP GET through the tunnel returns the correct response.
#[tokio::test]
async fn http_get_through_tunnel() {
    let port = free_port().await;
    let mut relay = start_relay(port).await;

    let (subdomain, mut ttx, mut trx) = register_tunnel(port).await;
    let host = format!("{subdomain}.tunnel.test");

    let bhost = host.clone();
    let browser = tokio::spawn(async move {
        raw_http(
            port,
            &format!("GET /hello HTTP/1.1\r\nHost: {bhost}\r\nConnection: close\r\n\r\n"),
        )
        .await
    });

    let frame = next_frame(&mut trx).await;
    let rid = match frame {
        TunnelFrame::HttpRequest {
            request_id,
            method,
            path,
            ..
        } => {
            assert_eq!(method, "GET");
            assert_eq!(path, "/hello");
            request_id
        }
        other => panic!("expected HttpRequest, got {other:?}"),
    };

    let body = b"Hello from localhost!";
    for f in [
        TunnelFrame::HttpResponseHeader {
            request_id: rid,
            status: 200,
            headers: vec![
                ("content-type".into(), "text/plain".into()),
                ("content-length".into(), body.len().to_string()),
            ],
        },
        TunnelFrame::HttpResponseBody {
            request_id: rid,
            chunk: body.to_vec(),
            finished: true,
        },
    ] {
        ttx.send(Message::Binary(encode_frame(&f).unwrap().into()))
            .await
            .unwrap();
    }

    let resp = browser.await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 200"), "response: {resp}");
    assert!(resp.contains("Hello from localhost!"), "body: {resp}");

    ttx.send(Message::Close(None)).await.ok();
    relay.kill().await.ok();
}

/// POST with JSON body round-trips correctly.
#[tokio::test]
async fn http_post_json_roundtrip() {
    let port = free_port().await;
    let mut relay = start_relay(port).await;

    let (subdomain, mut ttx, mut trx) = register_tunnel(port).await;
    let host = format!("{subdomain}.tunnel.test");

    let bhost = host.clone();
    let browser = tokio::spawn(async move {
        let json = r#"{"name":"tunnelx"}"#;
        raw_http(
            port,
            &format!(
                "POST /api/create HTTP/1.1\r\nHost: {bhost}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                json.len()
            ),
        )
        .await
    });

    let frame = next_frame(&mut trx).await;
    let rid = match frame {
        TunnelFrame::HttpRequest {
            request_id,
            method,
            path,
            body,
            headers,
        } => {
            assert_eq!(method, "POST");
            assert_eq!(path, "/api/create");
            assert_eq!(body, br#"{"name":"tunnelx"}"#);
            assert!(headers
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("content-type")
                    && v == "application/json"));
            request_id
        }
        other => panic!("expected HttpRequest, got {other:?}"),
    };

    let resp_body = br#"{"id":1,"created":true}"#;
    for f in [
        TunnelFrame::HttpResponseHeader {
            request_id: rid,
            status: 201,
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("content-length".into(), resp_body.len().to_string()),
            ],
        },
        TunnelFrame::HttpResponseBody {
            request_id: rid,
            chunk: resp_body.to_vec(),
            finished: true,
        },
    ] {
        ttx.send(Message::Binary(encode_frame(&f).unwrap().into()))
            .await
            .unwrap();
    }

    let resp = browser.await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 201"), "resp: {resp}");
    assert!(resp.contains(r#"{"id":1,"created":true}"#), "body: {resp}");

    ttx.send(Message::Close(None)).await.ok();
    relay.kill().await.ok();
}

/// WebSocket upgrade and bidirectional message proxying.
#[tokio::test]
async fn websocket_bidirectional_proxy() {
    let port = free_port().await;
    let mut relay = start_relay(port).await;

    let (subdomain, mut ttx, mut trx) = register_tunnel(port).await;
    let host = format!("{subdomain}.tunnel.test");

    let mut ws_req = format!("ws://127.0.0.1:{port}/live")
        .into_client_request()
        .unwrap();
    ws_req
        .headers_mut()
        .insert(HOST, HeaderValue::from_str(&host).unwrap());

    let browser_ws_task = tokio::spawn(async move {
        connect_async(ws_req).await.expect("browser ws connect").0
    });

    let ws_rid = match next_frame(&mut trx).await {
        TunnelFrame::WsOpen {
            request_id, path, ..
        } => {
            assert_eq!(path, "/live");
            request_id
        }
        other => panic!("expected WsOpen, got {other:?}"),
    };

    let mut browser_ws = browser_ws_task.await.unwrap();

    // Browser → tunnel client
    browser_ws.send(Message::Text("ping".into())).await.unwrap();
    match next_frame(&mut trx).await {
        TunnelFrame::WsFrame {
            request_id,
            data,
            opcode,
        } => {
            assert_eq!(request_id, ws_rid);
            assert_eq!(data, b"ping");
            assert_eq!(opcode, 1);
        }
        other => panic!("expected WsFrame, got {other:?}"),
    }

    // Tunnel client → browser
    ttx.send(Message::Binary(
        encode_frame(&TunnelFrame::WsFrame {
            request_id: ws_rid,
            data: b"pong".to_vec(),
            opcode: 1,
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();

    match tokio::time::timeout(Duration::from_secs(3), browser_ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
    {
        Message::Text(t) => assert_eq!(t, "pong"),
        other => panic!("expected text pong, got {other:?}"),
    }

    browser_ws.close(None).await.ok();
    ttx.send(Message::Close(None)).await.ok();
    relay.kill().await.ok();
}

/// After tunnel client disconnects, public URL returns 404.
#[tokio::test]
async fn disconnect_returns_not_found() {
    let port = free_port().await;
    let mut relay = start_relay(port).await;

    let (subdomain, mut ttx, trx) = register_tunnel(port).await;
    let host = format!("{subdomain}.tunnel.test");

    // Disconnect the tunnel client.
    ttx.send(Message::Binary(
        encode_frame(&TunnelFrame::Disconnect).unwrap().into(),
    ))
    .await
    .unwrap();
    ttx.send(Message::Close(None)).await.ok();
    drop(ttx);
    drop(trx);

    // Give the relay time to clean up.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let resp = raw_http(
        port,
        &format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"),
    )
    .await;
    assert!(
        resp.starts_with("HTTP/1.1 404"),
        "expected 404 after disconnect: {resp}"
    );

    relay.kill().await.ok();
}

/// HTTP GET through path-based routing works and rewrites paths correctly.
#[tokio::test]
async fn http_get_through_path_routing_tunnel() {
    let port = free_port().await;
    let mut relay = start_relay_with_env(port, true).await;

    let (subdomain, mut ttx, mut trx) = register_tunnel(port).await;

    // Send request via path-based routing: /t/{subdomain}/hello
    let browser = tokio::spawn(async move {
        raw_http(
            port,
            &format!("GET /t/{subdomain}/hello?foo=bar HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
        )
        .await
    });

    let frame = next_frame(&mut trx).await;
    let rid = match frame {
        TunnelFrame::HttpRequest {
            request_id,
            method,
            path,
            ..
        } => {
            assert_eq!(method, "GET");
            // The path must have the /t/{subdomain} stripped off!
            assert_eq!(path, "/hello?foo=bar");
            request_id
        }
        other => panic!("expected HttpRequest, got {other:?}"),
    };

    let body = b"Hello path-based routing!";
    for f in [
        TunnelFrame::HttpResponseHeader {
            request_id: rid,
            status: 200,
            headers: vec![
                ("content-type".into(), "text/plain".into()),
                ("content-length".into(), body.len().to_string()),
            ],
        },
        TunnelFrame::HttpResponseBody {
            request_id: rid,
            chunk: body.to_vec(),
            finished: true,
        },
    ] {
        ttx.send(Message::Binary(encode_frame(&f).unwrap().into()))
            .await
            .unwrap();
    }

    let resp = browser.await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 200"), "response: {resp}");
    assert!(resp.contains("Hello path-based routing!"), "body: {resp}");

    ttx.send(Message::Close(None)).await.ok();
    relay.kill().await.ok();
}
