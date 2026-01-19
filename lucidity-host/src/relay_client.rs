use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use lucidity_proto::{Frame, FrameDecoder};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

use crate::bridge::PaneBridge;

pub struct RelayClient {
    relay_url: String,
    relay_id: String,
    bridge: Option<Arc<dyn PaneBridge>>,
}

impl RelayClient {
    pub fn new(relay_url: String, relay_id: String) -> Self {
        Self {
            relay_url,
            relay_id,
            bridge: None,
        }
    }

    pub fn set_bridge(&mut self, bridge: Arc<dyn PaneBridge>) {
        self.bridge = Some(bridge);
    }

    /// Connect to relay server via WebSocket
    pub async fn connect(&self) -> Result<()> {
        let url_str = format!("{}/desktop/{}", self.relay_url, self.relay_id);
        let url = Url::parse(&url_str).context("Invalid relay URL")?;

        info!("Connecting to relay: {}", url);

        let (ws_stream, _) = connect_async(url).await.context("Failed to connect to relay")?;
        info!("Connected to relay server!");

        let (mut write, mut read) = ws_stream.split();
        let bridge = self.bridge.clone().context("PaneBridge not set")?;

        // Handle incoming messages from relay (from mobile)
        tokio::spawn(async move {
            let mut decoder = FrameDecoder::new();

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        // Decode frames from raw bytes
                        decoder.push(&data);
                        
                        while let Some(frame) = decoder.next_frame() {
                            match frame {
                                Ok(frame) => {
                                    // Handle frame via bridge
                                    let b = bridge.clone();
                                    // TODO: We need a way to feed frames into the bridge loop
                                    // or handle them directly here.
                                    // For now, this is a placeholder integration.
                                    debug!("Received frame via relay: {:?}", frame.frame_type);
                                },
                                Err(e) => error!("Frame decode error: {}", e),
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Relay connection closed");
                        break;
                    }
                    Ok(_) => {} // Ignore other message types
                    Err(e) => {
                        error!("Relay WebSocket error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}
