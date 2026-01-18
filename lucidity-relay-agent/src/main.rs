use std::time::Duration;

use anyhow::{anyhow, Context};
use futures::{SinkExt, StreamExt};
use lucidity_proto::frame::{encode_frame, FrameDecoder};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

#[derive(Debug, Deserialize)]
struct PairingPayload {
    desktop_public_key: String,
    relay_id: String,
    timestamp: i64,
    version: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let relay_base =
        std::env::var("LUCIDITY_RELAY_BASE").unwrap_or_else(|_| "ws://127.0.0.1:9090".to_string());
    let host_addr =
        std::env::var("LUCIDITY_HOST_ADDR").unwrap_or_else(|_| "127.0.0.1:9797".to_string());

    log::info!(
        "lucidity-relay-agent starting (host={}, relay={})",
        host_addr,
        relay_base
    );

    // Fetch relay_id from local host pairing payload.
    let payload = fetch_pairing_payload(&host_addr).await?;
    log::info!("desktop relay_id={}", payload.relay_id);

    let desktop_ws_url = Url::parse(&format!(
        "{}/ws/desktop/{}",
        relay_base.trim_end_matches('/'),
        payload.relay_id
    ))
    .context("invalid relay base URL")?;

    loop {
        match run_control_loop(&desktop_ws_url, &relay_base, &host_addr).await {
            Ok(()) => {
                log::warn!("control loop ended; reconnecting in 1s");
            }
            Err(e) => {
                log::warn!("control loop error: {e:#}; reconnecting in 1s");
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn fetch_pairing_payload(host_addr: &str) -> anyhow::Result<PairingPayload> {
    let mut socket = TcpStream::connect(host_addr)
        .await
        .with_context(|| format!("connect to host {host_addr}"))?;

    // Send {"op":"pairing_payload"}
    let json = br#"{"op":"pairing_payload"}"#;
    let frame = encode_frame(1, json);
    socket.write_all(&frame).await?;
    socket.flush().await?;

    let mut decoder = FrameDecoder::new();
    let mut buf = vec![0u8; 8192];

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(anyhow!("timeout waiting for pairing_payload"));
        }

        let n = socket.read(&mut buf).await?;
        if n == 0 {
            return Err(anyhow!("host closed while waiting for pairing_payload"));
        }

        decoder.push(&buf[..n]);
        if let Some(frame) = decoder
            .next_frame()
            .context("decode pairing_payload frame")?
        {
            if frame.typ != 1 {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&frame.payload)?;
            if v.get("op").and_then(|o| o.as_str()) != Some("pairing_payload") {
                continue;
            }
            let payload = v
                .get("payload")
                .ok_or_else(|| anyhow!("missing payload field"))?;
            let p: PairingPayload = serde_json::from_value(payload.clone())?;
            return Ok(p);
        }
    }
}

async fn run_control_loop(
    desktop_ws_url: &Url,
    relay_base: &str,
    host_addr: &str,
) -> anyhow::Result<()> {
    let (ws, _resp) = tokio_tungstenite::connect_async(desktop_ws_url.as_str())
        .await
        .context("connect desktop control ws")?;

    let (mut ws_tx, mut ws_rx) = ws.split();

    // Wait for "registered" control ack (best-effort).
    if let Some(Ok(Message::Text(t))) = ws_rx.next().await {
        log::info!("control: {}", t);
    }

    while let Some(result) = ws_rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(e) => return Err(anyhow!("control ws error: {e}")),
        };

        if let Message::Text(text) = msg {
            let parsed: Result<lucidity_proto::relay::RelayMessage, _> =
                serde_json::from_str(&text);
            let Ok(parsed) = parsed else {
                continue;
            };

            if let lucidity_proto::relay::RelayMessage::SessionRequest {
                session_id,
                client_id,
            } = parsed
            {
                log::info!(
                    "session request session_id={} client_id={}",
                    session_id,
                    client_id
                );

                // Accept immediately. Desktop-side pairing approval already happened earlier.
                ws_tx
                    .send(Message::Text(serde_json::to_string(
                        &lucidity_proto::relay::RelayMessage::SessionAccept {
                            session_id: session_id.clone(),
                        },
                    )?))
                    .await?;

                let relay_base = relay_base.to_string();
                let host_addr = host_addr.to_string();
                tokio::spawn(async move {
                    if let Err(e) = run_session_bridge(&relay_base, &session_id, &host_addr).await {
                        log::warn!("session {} ended: {e:#}", session_id);
                    }
                });
            }
        }
    }

    Ok(())
}

async fn run_session_bridge(
    relay_base: &str,
    session_id: &str,
    host_addr: &str,
) -> anyhow::Result<()> {
    let base = relay_base.trim_end_matches('/');
    let url = Url::parse(&format!("{base}/ws/session/{session_id}?role=desktop"))?;

    let (ws, _resp) = tokio_tungstenite::connect_async(url.as_str()).await?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    let tcp = TcpStream::connect(host_addr).await?;
    let (mut tcp_r, mut tcp_w) = tcp.into_split();

    let ws_to_tcp = async move {
        while let Some(result) = ws_rx.next().await {
            let msg = result?;
            match msg {
                Message::Binary(b) => {
                    tcp_w.write_all(&b).await?;
                    tcp_w.flush().await?;
                }
                Message::Text(t) => {
                    // Ignore control-plane text on data tunnel.
                    log::debug!(
                        "session {}: ignoring text msg ({} bytes)",
                        session_id,
                        t.len()
                    );
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        anyhow::Ok(())
    };

    let tcp_to_ws = async move {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            let n = tcp_r.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            ws_tx.send(Message::Binary(buf[..n].to_vec())).await?;
        }
        anyhow::Ok(())
    };

    tokio::select! {
        r = ws_to_tcp => r?,
        r = tcp_to_ws => r?,
    }

    Ok(())
}
