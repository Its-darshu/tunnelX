use std::time::Duration;

use protocol::TunnelFrame;
use shared::{HEARTBEAT_ACK_TIMEOUT_SECS, HEARTBEAT_INTERVAL_SECS};

pub fn spawn_heartbeat(
    tx: tokio::sync::mpsc::Sender<TunnelFrame>,
    mut acknowledgements: tokio::sync::mpsc::Receiver<()>,
    failed_tx: tokio::sync::oneshot::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if tx.send(TunnelFrame::Heartbeat).await.is_err() {
                break;
            }

            match tokio::time::timeout(
                Duration::from_secs(HEARTBEAT_ACK_TIMEOUT_SECS),
                acknowledgements.recv(),
            )
            .await
            {
                Ok(Some(())) => {}
                Ok(None) | Err(_) => {
                    tracing::warn!("relay did not acknowledge heartbeat");
                    let _ = failed_tx.send(());
                    break;
                }
            }
        }
    })
}
