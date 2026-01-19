//! WebSocket Relay Client for lucidity-host
//!
//! Connects to a relay server when P2P (UPnP/STUN) fails.
//! This provides a fallback connection path for mobile clients.

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use lucidity_proto::frame::{encode_frame, Frame, FrameDecoder};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::Duration;

use crate::bridge::{PaneBridge, PaneInfo};
use crate::protocol::{TYPE_JSON, TYPE_PANE_INPUT, TYPE_PANE_OUTPUT};
// Note: We might not need all logic from pairing_api if we just forward requests, 
// but for V1 we implement the host logic here too.
use crate::pairing_api::{
    handle_pairing_submit, pairing_payload_with_p2p, list_trusted_devices, verify_device_auth, 
    load_or_create_host_keypair
};

/// Relay connection status
#[derive(Debug, Clone, PartialEq)]
pub enum RelayStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

pub use lucidity_proto::protocol::{JsonRequest, JsonResponse};

/// Client for connecting to the Lucidity relay server
pub struct RelayClient {
    relay_url: String,
    relay_id: String,
    desktop_secret: Option<String>,
    bridge: Option<Arc<dyn PaneBridge>>,
    status: Arc<Mutex<RelayStatus>>,
    /// Channel to send outgoing messages to the relay
    outgoing_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

impl RelayClient {
    /// Create a new relay client
    pub fn new(relay_url: String, relay_id: String) -> Self {
        Self {
            relay_url,
            relay_id,
            desktop_secret: std::env::var("LUCIDITY_RELAY_SECRET").ok(),
            bridge: None,
            status: Arc::new(Mutex::new(RelayStatus::Disconnected)),
            outgoing_tx: None,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let relay_url = std::env::var("LUCIDITY_RELAY_URL").ok()?;
        let relay_id = std::env::var("LUCIDITY_RELAY_ID").ok()?;
        Some(Self::new(relay_url, relay_id))
    }

    /// Set the pane bridge for handling incoming frames
    pub fn set_bridge(&mut self, bridge: Arc<dyn PaneBridge>) {
        self.bridge = Some(bridge);
    }

    /// Get the current relay status
    pub async fn status(&self) -> RelayStatus {
        self.status.lock().await.clone()
    }

    /// Get the relay URL
    pub fn relay_url(&self) -> &str {
        &self.relay_url
    }

    /// Get the relay ID
    pub fn relay_id(&self) -> &str {
        &self.relay_id
    }

    /// Send binary data to the relay (to be forwarded to mobile)
    pub fn send(&self, data: Vec<u8>) -> Result<()> {
        if let Some(tx) = &self.outgoing_tx {
            tx.send(data).context("Failed to send to relay")?;
        } else {
            anyhow::bail!("Relay not connected");
        }
        Ok(())
    }

    /// Send a framed message to the relay
    pub fn send_frame(&self, frame_type: u8, payload: &[u8]) -> Result<()> {
        let frame_data = encode_frame(frame_type, payload);
        self.send(frame_data)
    }

    /// Connect to relay server via WebSocket
    pub async fn connect(&mut self) -> Result<()> {
        // Update status
        {
            let mut status = self.status.lock().await;
            *status = RelayStatus::Connecting;
        }

        // Build URL with optional secret
        let mut url_str = format!("{}/desktop/{}", self.relay_url, self.relay_id);
        if let Some(secret) = &self.desktop_secret {
            url_str = format!("{}?secret={}", url_str, secret);
        }

        let url = Url::parse(&url_str).context("Invalid relay URL")?;
        info!("Connecting to relay: {}", self.relay_url);

        let (ws_stream, _) = match connect_async(url).await {
            Ok(result) => result,
            Err(e) => {
                let mut status = self.status.lock().await;
                *status = RelayStatus::Error(e.to_string());
                return Err(e).context("Failed to connect to relay");
            }
        };

        info!("Connected to relay server: relay_id={}", self.relay_id);

        // Update status
        {
            let mut status = self.status.lock().await;
            *status = RelayStatus::Connected;
        }

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // Create channel for outgoing messages
        let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.outgoing_tx = Some(outgoing_tx.clone());

        // Clone status for the tasks
        let status_clone = self.status.clone();
        let relay_id = self.relay_id.clone();

        // Task: Forward outgoing messages to WebSocket
        let relay_id_out = relay_id.clone();
        tokio::spawn(async move {
            while let Some(data) = outgoing_rx.recv().await {
                if let Err(e) = ws_tx.send(Message::Binary(data)).await {
                    error!("Failed to send to relay {}: {}", relay_id_out, e);
                    break;
                }
            }
            debug!("Outgoing relay task ended for {}", relay_id_out);
        });

        // Task: Handle incoming messages from relay
        let bridge = self.bridge.clone();
        let relay_id_in = relay_id.clone();
        let outgoing_tx_handler = outgoing_tx.clone();
        
        tokio::spawn(async move {
            let mut decoder = FrameDecoder::new();
            
            // Per-session state (simplified for Relay: assuming one active controller per relay session)
            let mut authenticated = false;
            let mut auth_nonce: Option<String> = None;
            let attached = Arc::new(Mutex::new(None::<usize>));

            while let Some(msg_result) = ws_rx.next().await {
                match msg_result {
                    Ok(Message::Binary(data)) => {
                        decoder.push(&data);

                        while let Ok(Some(frame)) = decoder.next_frame() {
                            // If not authenticated and not already challenging, send challenge
                            // But ONLY if it's not an AuthResponse or PairingRequest
                            // For simplicity, we enforce auth for sensitive ops
                            
                            if let Err(e) = Self::handle_incoming_frame(
                                &bridge, 
                                &relay_id_in, 
                                frame, 
                                &outgoing_tx_handler,
                                &mut authenticated,
                                &mut auth_nonce,
                                &attached
                            ).await {
                                error!("Error handling frame from {}: {}", relay_id_in, e);
                            }
                        }
                    }
                    Ok(Message::Text(text)) => {
                        debug!("Received text from relay: {}", text);
                    }
                    Ok(Message::Ping(_)) => {
                        debug!("Received ping from relay");
                    }
                    Ok(Message::Pong(_)) => {
                        debug!("Received pong from relay");
                    }
                    Ok(Message::Close(_)) => {
                        info!("Relay connection closed: {}", relay_id_in);
                        break;
                    }
                    Ok(Message::Frame(_)) => {}
                    Err(e) => {
                        error!("Relay WebSocket error {}: {}", relay_id_in, e);
                        break;
                    }
                }
            }

            // Update status on disconnect
            let mut status = status_clone.lock().await;
            *status = RelayStatus::Disconnected;
            info!("Relay connection ended: {}", relay_id_in);
        });

        Ok(())
    }

    /// Handle an incoming frame from the relay
    async fn handle_incoming_frame(
        bridge: &Option<Arc<dyn PaneBridge>>,
        _relay_id: &str,
        frame: Frame,
        tx: &mpsc::UnboundedSender<Vec<u8>>,
        authenticated: &mut bool,
        auth_nonce: &mut Option<String>,
        attached: &Arc<Mutex<Option<usize>>>,
    ) -> Result<()> {
        match frame.typ {
            TYPE_JSON => {
                let req: JsonRequest = match serde_json::from_slice(&frame.payload) {
                    Ok(r) => r,
                    Err(err) => {
                        Self::send_json_response(tx, &JsonResponse::Error {
                            message: format!("invalid json request: {err}"),
                        })?;
                        return Ok(());
                    }
                };
                
                // Handle authentication logic
                match req {
                    JsonRequest::AuthResponse { public_key, signature, client_nonce } => {
                       if let Some(nonce) = auth_nonce {
                           verify_device_auth(&public_key, &signature, nonce)?;
                           *authenticated = true;
                           
                           // Register for push notifications
                           let (push_tx, mut push_rx) = tokio::sync::mpsc::unbounded_channel();
                           let tx_push = tx.clone();
                           tokio::spawn(async move {
                               while let Some(msg) = push_rx.recv().await {
                                   if let Err(_) = Self::send_json_response(&tx_push, &msg) {
                                       break;
                                   }
                               }
                           });
                           crate::registry::REGISTRY.register(public_key.clone(), push_tx);

                           let host_sig = if let Some(cn) = client_nonce {
                               let keypair = load_or_create_host_keypair()?;
                               Some(keypair.sign(cn.as_bytes()).to_base64())
                           } else {
                               None
                           };
                           
                           Self::send_json_response(tx, &JsonResponse::AuthSuccess {
                               signature: host_sig,
                           })?;
                           return Ok(());
                       } else {
                           // Unexpected auth response, maybe stale?
                           // Treat as unauthed if we didn't ask? Or just accept if valid? 
                           // Protocol requires challenge-response.
                           Self::send_json_response(tx, &JsonResponse::Error {
                               message: "unexpected auth response (no nonce)".to_string(),
                           })?;
                           return Ok(());
                       }
                    }
                    // These ops are allowed without auth
                    JsonRequest::PairingPayload | JsonRequest::PairingSubmit { .. } => {}
                    
                    // All other ops require auth
                    _ if !*authenticated => {
                        // Generate challenge
                        let nonce = Uuid::new_v4().to_string();
                        *auth_nonce = Some(nonce.clone());
                        
                        // Send challenge
                        Self::send_json_response(tx, &JsonResponse::AuthChallenge {
                            nonce: nonce,
                        })?;
                        
                        // Also send Error to indicate original request failed
                        Self::send_json_response(tx, &JsonResponse::Error {
                            message: "authentication required".to_string(),
                        })?;
                        return Ok(());
                    }
                    _ => {}
                }

                // Process request
                match req {
                    JsonRequest::ListPanes => {
                        if let Some(b) = bridge {
                            let panes = b.list_panes()?;
                            Self::send_json_response(tx, &JsonResponse::ListPanes { panes })?;
                        }
                    }
                    JsonRequest::Attach { pane_id } => {
                        if let Some(b) = bridge {
                            {
                                let mut a = attached.lock().await;
                                if a.is_some() {
                                    Self::send_json_response(tx, &JsonResponse::Error {
                                        message: "already attached".to_string(),
                                    })?;
                                    return Ok(());
                                }
                                *a = Some(pane_id);
                            }
                            
                            let sub = b.subscribe_output(pane_id)?;
                            let tx2 = tx.clone();
                            
                            // Spawn monitoring thread
                             tokio::task::spawn_blocking(move || {
                                while let Ok(Some(bytes)) = sub.recv_timeout(Duration::from_millis(250)) {
                                    let frame = encode_frame(TYPE_PANE_OUTPUT, &bytes);
                                    if tx2.send(frame).is_err() {
                                        break; 
                                    }
                                }
                            });
                            
                            Self::send_json_response(tx, &JsonResponse::AttachOk { pane_id })?;
                        }
                    }
                    JsonRequest::PairingPayload => {
                        // Relay mode: we don't know P2P addrs easily here, or we could pass them?
                        // For now pass None/None as we are using Relay
                        let payload = pairing_payload_with_p2p(None, None)?;
                        Self::send_json_response(tx, &JsonResponse::PairingPayload { payload })?;
                    }
                    JsonRequest::PairingSubmit { request } => {
                         let response = handle_pairing_submit(request)?;
                         Self::send_json_response(tx, &JsonResponse::PairingResponse { response })?;
                    }
                    JsonRequest::PairingListTrustedDevices => {
                        let devices = list_trusted_devices()?;
                        Self::send_json_response(tx, &JsonResponse::PairingTrustedDevices { devices })?;
                    }
                    JsonRequest::Paste { pane_id, text } => {
                        if let Some(b) = bridge {
                            b.send_paste(pane_id, &text)?;
                        }
                    }
                    JsonRequest::Resize { pane_id, rows, cols } => {
                        if let Some(b) = bridge {
                            b.resize(pane_id, rows, cols)?;
                        }
                    }
                    JsonRequest::RevokeDevice { public_key } => {
                        crate::pairing_api::revoke_device(&public_key)?;
                        Self::send_json_response(tx, &JsonResponse::Error {
                            message: "device revoked".to_string(),
                        })?;
                    }
                    _ => {} // AuthResponse handled above
                }
            }
            TYPE_PANE_INPUT => {
                if let Some(b) = bridge {
                    let mut a = attached.lock().await;
                     if let Some(pane_id) = *a {
                         b.send_input(pane_id, &frame.payload)?;
                     }
                }
            }
            _ => {
                warn!("Unknown frame type from relay: {}", frame.typ);
            }
        }
        Ok(())
    }
    
    fn send_json_response(tx: &mpsc::UnboundedSender<Vec<u8>>, resp: &JsonResponse) -> Result<()> {
        let payload = serde_json::to_vec(resp)?;
        let frame = encode_frame(TYPE_JSON, &payload);
        tx.send(frame).map_err(|_| anyhow!("failed to send to relay channel"))?;
        Ok(())
    }

    /// Disconnect from the relay
    pub async fn disconnect(&mut self) {
        self.outgoing_tx = None;
        let mut status = self.status.lock().await;
        *status = RelayStatus::Disconnected;
        info!("Disconnected from relay");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_client_creation() {
        let client = RelayClient::new(
            "ws://localhost:9090".to_string(),
            "test-relay-id".to_string(),
        );
        assert_eq!(client.relay_url(), "ws://localhost:9090");
        assert_eq!(client.relay_id(), "test-relay-id");
    }

    #[tokio::test]
    async fn test_relay_client_status() {
        let client = RelayClient::new(
            "ws://localhost:9090".to_string(),
            "test-id".to_string(),
        );
        assert_eq!(client.status().await, RelayStatus::Disconnected);
    }
}
