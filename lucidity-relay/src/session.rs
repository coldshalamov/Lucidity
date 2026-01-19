use anyhow::Result;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::mpsc;
use warp::ws::{Message, WebSocket};

/// Session represents a desktop-mobile pairing
pub struct Session {
    pub relay_id: String,
    pub desktop_tx: Option<mpsc::UnboundedSender<Message>>,
    pub mobile_tx: Option<mpsc::UnboundedSender<Message>>,
}

impl Session {
    pub fn new(relay_id: String) -> Self {
        Self {
            relay_id,
            desktop_tx: None,
            mobile_tx: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.desktop_tx.is_some() && self.mobile_tx.is_some()
    }
}

/// SessionManager manages all active relay sessions
pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<tokio::sync::RwLock<Session>>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn get_or_create(&self, relay_id: String) -> Arc<tokio::sync::RwLock<Session>> {
        self.sessions
            .entry(relay_id.clone())
            .or_insert_with(|| Arc::new(tokio::sync::RwLock::new(Session::new(relay_id))))
            .clone()
    }

    pub fn remove(&self, relay_id: &str) {
        self.sessions.remove(relay_id);
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// RelayServer handles WebSocket connections and message routing
pub struct RelayServer {
    manager: Arc<SessionManager>,
}

impl RelayServer {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SessionManager::new()),
        }
    }

    pub fn manager(&self) -> Arc<SessionManager> {
        self.manager.clone()
    }

    /// Handle desktop connection
    pub async fn handle_desktop(
        &self,
        relay_id: String,
        ws: WebSocket,
    ) -> Result<()> {
        info!("Desktop connected: relay_id={}", relay_id);

        let (mut ws_tx, mut ws_rx) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        // Register desktop in session
        let session = self.manager.get_or_create(relay_id.clone());
        {
            let mut sess = session.write().await;
            sess.desktop_tx = Some(tx.clone());
        }

        // Spawn task to forward messages from channel to WebSocket
        let relay_id_clone = relay_id.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_tx.send(msg).await {
                    error!("Failed to send to desktop {}: {}", relay_id_clone, e);
                    break;
                }
            }
        });

        // Forward messages from WebSocket to mobile
        while let Some(msg_result) = ws_rx.next().await {
            match msg_result {
                Ok(msg) => {
                    if msg.is_close() {
                        info!("Desktop {} disconnected", relay_id);
                        break;
                    }

                    // Forward to mobile if connected
                    let sess = session.read().await;
                    if let Some(mobile_tx) = &sess.mobile_tx {
                        if let Err(e) = mobile_tx.send(msg) {
                            warn!("Failed to forward to mobile: {}", e);
                        }
                    } else {
                        debug!("No mobile connected for relay_id={}", relay_id);
                    }
                }
                Err(e) => {
                    error!("WebSocket error from desktop {}: {}", relay_id, e);
                    break;
                }
            }
        }

        // Cleanup
        {
            let mut sess = session.write().await;
            sess.desktop_tx = None;
        }
        info!("Desktop {} session ended", relay_id);

        Ok(())
    }

    /// Handle mobile connection
    pub async fn handle_mobile(
        &self,
        relay_id: String,
        ws: WebSocket,
    ) -> Result<()> {
        info!("Mobile connected: relay_id={}", relay_id);

        let (mut ws_tx, mut ws_rx) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        // Register mobile in session
        let session = self.manager.get_or_create(relay_id.clone());
        {
            let mut sess = session.write().await;
            sess.mobile_tx = Some(tx.clone());
        }

        // Spawn task to forward messages from channel to WebSocket
        let relay_id_clone = relay_id.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = ws_tx.send(msg).await {
                    error!("Failed to send to mobile {}: {}", relay_id_clone, e);
                    break;
                }
            }
        });

        // Forward messages from WebSocket to desktop
        while let Some(msg_result) = ws_rx.next().await {
            match msg_result {
                Ok(msg) => {
                    if msg.is_close() {
                        info!("Mobile {} disconnected", relay_id);
                        break;
                    }

                    // Forward to desktop if connected
                    let sess = session.read().await;
                    if let Some(desktop_tx) = &sess.desktop_tx {
                        if let Err(e) = desktop_tx.send(msg) {
                            warn!("Failed to forward to desktop: {}", e);
                        }
                    } else {
                        debug!("No desktop connected for relay_id={}", relay_id);
                    }
                }
                Err(e) => {
                    error!("WebSocket error from mobile {}: {}", relay_id, e);
                    break;
                }
            }
        }

        // Cleanup
        {
            let mut sess = session.write().await;
            sess.mobile_tx = None;
        }
        info!("Mobile {} session ended", relay_id);

        Ok(())
    }
}

impl Default for RelayServer {
    fn default() -> Self {
        Self::new()
    }
}
