use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use futures_util::{SinkExt, StreamExt};
use protocol::{decode_frame, encode_frame, TunnelFrame};
use rand::Rng;
use shared::{ClientConfig, TunnelError};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::heartbeat;
use crate::proxy::ProxyHandler;
use crate::ui;

pub async fn run(
    config: ClientConfig,
    subdomain: Option<String>,
    duration_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    match connect_and_serve(&config, subdomain, duration_secs).await {
        Ok(()) => {}
        Err(e) => {
            ui::print_step_fail(&format!("Connection error: {e}"));
        }
    }
    Ok(())
}

async fn connect_and_serve(
    config: &ClientConfig,
    subdomain: Option<String>,
    duration_secs: u64,
) -> Result<(), TunnelError> {
    let token = generate_token();
    let url = &config.relay_url;
    let connect_url = if url.contains("localhost") || url.contains("127.0.0.1") {
        url.replace("wss://", "ws://")
    } else {
        url.clone()
    };

    let (ws_stream, _) = connect_async(&connect_url)
        .await
        .map_err(|e| TunnelError::RelayUnavailable(e.to_string()))?;

    ui::print_step_done(&format!(
        "Connected to TunnelX server ({})",
        connect_url.replace("ws://", "").replace("wss://", "").split('/').next().unwrap_or("relay")
    ));

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // ── Register with custom subdomain + duration ──────────────
    ui::print_step_start(2, 3, "Registering tunnel...");

    let register = TunnelFrame::register_with_options(
        token.clone(),
        subdomain.clone(),
        Some(duration_secs),
    );
    ws_tx
        .send(Message::Binary(
            encode_frame(&register)
                .map_err(|e| TunnelError::ProtocolError(e.to_string()))?
                .into(),
        ))
        .await
        .map_err(|e| TunnelError::ConnectionError(e.to_string()))?;

    // ── Wait for Registered or Error ───────────────────────────
    let (registered_subdomain, public_url, expires_in) = loop {
        match ws_rx.next().await {
            Some(Ok(Message::Binary(data))) => match decode_frame(&data) {
                Ok(TunnelFrame::Registered {
                    subdomain,
                    public_url,
                    expires_in_secs,
                }) => {
                    break (subdomain, public_url, expires_in_secs.unwrap_or(duration_secs));
                }
                Ok(TunnelFrame::Error { code, message }) => {
                    ui::print_step_fail(&format!("{code}: {message}"));
                    return Err(TunnelError::ProtocolError(format!("{code}: {message}")));
                }
                Ok(_) => continue,
                Err(e) => return Err(TunnelError::ProtocolError(e.to_string())),
            },
            Some(Ok(Message::Close(_))) | None => {
                return Err(TunnelError::ConnectionError("relay closed connection".into()));
            }
            Some(Err(e)) => return Err(TunnelError::ConnectionError(e.to_string())),
            _ => continue,
        }
    };

    ui::print_step_done(&format!("Subdomain \"{}\" registered!", registered_subdomain));

    // ── Step 3: Show live tunnel info ──────────────────────────
    ui::print_tunnel_live(&public_url, config.port, expires_in);

    let tunnel_start = Instant::now();
    let proxy = ProxyHandler::new(config.port);
    let (frame_tx, mut frame_rx) = mpsc::channel::<TunnelFrame>(256);

    // Channel for request log entries from proxy to UI
    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<ui::RequestEntry>();

    let ws_tx = Arc::new(tokio::sync::Mutex::new(ws_tx));
    let ws_tx_writer = ws_tx.clone();

    // Forward outbound frames to websocket
    let write_task = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            let bytes = match encode_frame(&frame) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let mut tx = ws_tx_writer.lock().await;
            if tx.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
    });

    // Heartbeat
    let (heartbeat_ack_tx, heartbeat_ack_rx) = mpsc::channel(1);
    let (heartbeat_failed_tx, mut heartbeat_failed_rx) = tokio::sync::oneshot::channel();
    let heartbeat_tx = frame_tx.clone();
    let heartbeat_handle =
        heartbeat::spawn_heartbeat(heartbeat_tx, heartbeat_ack_rx, heartbeat_failed_tx);

    // Keyboard input listener
    let (key_tx, mut key_rx) = mpsc::channel::<KeyCode>(16);
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                    if key_tx.blocking_send(code).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Countdown timer ticker
    let mut countdown_interval = tokio::time::interval(Duration::from_secs(1));

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    let mut shutdown_requested = false;

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                shutdown_requested = true;
                break;
            }
            _ = &mut heartbeat_failed_rx => {
                ui::print_step_fail("Relay heartbeat timed out");
                break;
            }
            // Countdown timer
            _ = countdown_interval.tick() => {
                let remaining = ui::remaining_secs(tunnel_start, expires_in);
                if remaining == 0 {
                    ui::print_expired();
                    break;
                }
                ui::print_countdown(remaining);
            }
            // Request log entries from proxy
            entry = log_rx.recv() => {
                if let Some(entry) = entry {
                    // Clear the countdown line, print log entry, reprint countdown
                    print!("\r                                                              \r");
                    ui::print_request_entry(&entry);
                    let remaining = ui::remaining_secs(tunnel_start, expires_in);
                    ui::print_countdown(remaining);
                }
            }
            // Keyboard input
            key = key_rx.recv() => {
                match key {
                    Some(KeyCode::Char('q') | KeyCode::Char('Q')) => {
                        shutdown_requested = true;
                        break;
                    }
                    Some(KeyCode::Char('c') | KeyCode::Char('C')) => {
                        if ui::copy_to_clipboard(&public_url) {
                            ui::print_clipboard_copied();
                        } else {
                            ui::print_clipboard_failed();
                        }
                    }
                    _ => {}
                }
            }
            // WebSocket messages from relay
            msg = ws_rx.next() => {
                let Some(msg) = msg else { break; };
                match msg {
                    Ok(Message::Binary(data)) => match decode_frame(&data) {
                        Ok(TunnelFrame::HeartbeatAck) => {
                            let _ = heartbeat_ack_tx.try_send(());
                        }
                        Ok(TunnelFrame::HttpRequest {
                            request_id,
                            method,
                            path,
                            headers,
                            body,
                        }) => {
                            let proxy = proxy.clone();
                            let tx = frame_tx.clone();
                            let log = log_tx.clone();
                            let req_method = method.clone();
                            let req_path = path.clone();
                            let start = Instant::now();
                            tokio::spawn(async move {
                                let status = proxy
                                    .handle_http_request(request_id, method, path, headers, body, tx.clone())
                                    .await;
                                let _ = log.send(ui::RequestEntry {
                                    timestamp: ui::now_timestamp(),
                                    method: req_method,
                                    path: req_path,
                                    status,
                                    latency_ms: start.elapsed().as_millis() as u64,
                                });
                            });
                        }
                        Ok(TunnelFrame::WsOpen {
                            request_id,
                            path,
                            headers,
                        }) => {
                            let proxy = proxy.clone();
                            let tx = frame_tx.clone();
                            let log = log_tx.clone();
                            let ws_path = path.clone();
                            tokio::spawn(async move {
                                proxy.handle_ws_open(request_id, path, headers, tx).await;
                                let _ = log.send(ui::RequestEntry {
                                    timestamp: ui::now_timestamp(),
                                    method: "WS".into(),
                                    path: ws_path,
                                    status: 101,
                                    latency_ms: 0,
                                });
                            });
                        }
                        Ok(TunnelFrame::WsFrame {
                            request_id,
                            data,
                            opcode,
                        }) => {
                            proxy.forward_ws_frame(request_id, data, opcode).await;
                        }
                        Ok(TunnelFrame::WsClose { request_id }) => {
                            proxy.close_ws(request_id).await;
                        }
                        Ok(TunnelFrame::Disconnect) => break,
                        Ok(TunnelFrame::Error { code, message }) => {
                            ui::print_step_fail(&format!("Relay error {code}: {message}"));
                        }
                        Ok(_) => {}
                        Err(e) => tracing::error!(%e, "decode error"),
                    },
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(data)) => {
                        let mut tx = ws_tx.lock().await;
                        let _ = tx.send(Message::Pong(data)).await;
                    }
                    Err(e) => {
                        tracing::error!(%e, "websocket error");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // ── Cleanup ────────────────────────────────────────────────
    heartbeat_handle.abort();
    let _ = heartbeat_handle.await;
    write_task.abort();
    let _ = write_task.await;

    if shutdown_requested {
        let disconnect = encode_frame(&TunnelFrame::Disconnect)
            .map_err(|e| TunnelError::ProtocolError(e.to_string()))?;
        let mut tx = ws_tx.lock().await;
        let _ = tx.send(Message::Binary(disconnect.into())).await;
    }

    ui::print_disconnect();
    Ok(())
}

/// Generate a cryptographically secure token for tunnel registration.
/// Uses 32 random bytes (256 bits) for sufficient entropy.
fn generate_token() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32)
        .map(|_| rng.random_range(0..256) as u8)
        .collect();
    hex::encode(&bytes)
}
