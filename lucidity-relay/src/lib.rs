//! Lucidity Relay Server Library
//!
//! This library contains the core relay server functionality that can be used
//! by both the binary and integration tests.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

use lucidity_proto::relay::RelayMessage;

/// Channel buffer size - prevents unbounded memory growth
pub const CHANNEL_BUFFER_SIZE: usize = 1024;

/// Heartbeat interval in seconds
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Connection timeout after missed heartbeats
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 90;

pub type Tx = mpsc::Sender<Message>;

#[derive(Clone)]
pub struct DesktopControl {
    pub tx: Tx,
    /// The public key fingerprint of the desktop (for authentication)
    pub public_key_fingerprint: Option<String>,
    /// Last heartbeat received
    pub last_heartbeat: Arc<Mutex<Instant>>,
}

#[derive(Clone)]
pub struct PendingSession {
    pub relay_id: String,
    pub client_id: String,
    pub mobile_tx: Tx,
    /// The public key fingerprint of the mobile client that created this session
    pub mobile_fingerprint: Option<String>,
}

#[derive(Default)]
pub struct SessionSlots {
    pub desktop_tx: Option<Tx>,
    pub mobile_tx: Option<Tx>,
}

pub struct SessionInfo {
    pub relay_id: String,
    pub slots: SessionSlots,
    /// Fingerprint of the desktop that accepted this session
    pub desktop_fingerprint: Option<String>,
    /// Fingerprint of the mobile that created this session
    pub mobile_fingerprint: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Require authentication (production default)
    Required,
    /// Allow unauthenticated connections (dev mode - requires explicit opt-in)
    Disabled,
}

pub struct State {
    pub desktops: Mutex<HashMap<String, DesktopControl>>,
    pub pending: Mutex<HashMap<String, PendingSession>>,
    pub sessions: Mutex<HashMap<String, SessionInfo>>,
    pub jwt_secret: Option<Arc<String>>,
    /// Desktop authentication secret (shared secret for HMAC or can be extended to ed25519)
    pub desktop_secret: Option<Arc<String>>,
    pub auth_mode: AuthMode,
}

impl Default for State {
    fn default() -> Self {
        Self {
            desktops: Mutex::default(),
            pending: Mutex::default(),
            sessions: Mutex::default(),
            jwt_secret: None,
            desktop_secret: None,
            auth_mode: AuthMode::Required,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRole {
    Desktop,
    Mobile,
}

impl SessionRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "desktop" => Some(SessionRole::Desktop),
            "mobile" => Some(SessionRole::Mobile),
            _ => None,
        }
    }
}

/// Background task that checks for dead connections and cleans them up
pub async fn heartbeat_checker(state: Arc<State>) {
    let mut ticker = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
    loop {
        ticker.tick().await;

        let timeout = Duration::from_secs(HEARTBEAT_TIMEOUT_SECS);
        let now = Instant::now();

        // Check desktops for timeout
        let mut dead_desktops = Vec::new();
        {
            let desktops = state.desktops.lock().await;
            for (relay_id, desktop) in desktops.iter() {
                let last = *desktop.last_heartbeat.lock().await;
                if now.duration_since(last) > timeout {
                    dead_desktops.push(relay_id.clone());
                }
            }
        }

        // Remove dead desktops
        for relay_id in dead_desktops {
            log::warn!("Desktop {} timed out (no heartbeat)", relay_id);
            let mut desktops = state.desktops.lock().await;
            if let Some(desktop) = desktops.remove(&relay_id) {
                let _ = desktop.tx.send(Message::close()).await;
            }
        }
    }
}

pub async fn desktop_control(
    ws: WebSocket,
    relay_id: String,
    auth: Option<String>,
    state: Arc<State>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(CHANNEL_BUFFER_SIZE);

    // Authenticate desktop if auth mode is required
    let public_key_fingerprint = if state.auth_mode == AuthMode::Required {
        if let Some(ref secret) = state.desktop_secret {
            match authorize_desktop(secret, &auth) {
                Ok(fingerprint) => Some(fingerprint),
                Err(e) => {
                    log::warn!("Desktop auth failed for relay_id={}: {}", relay_id, e);
                    let _ = ws_tx
                        .send(Message::close_with(4401u16, "unauthorized"))
                        .await;
                    return;
                }
            }
        } else {
            // No desktop secret configured but auth required - reject
            log::warn!("Desktop connection rejected: no LUCIDITY_RELAY_DESKTOP_SECRET configured");
            let _ = ws_tx
                .send(Message::close_with(4401u16, "auth_not_configured"))
                .await;
            return;
        }
    } else {
        None
    };

    // Check if relay_id is already in use
    {
        let desktops = state.desktops.lock().await;
        if desktops.contains_key(&relay_id) {
            log::warn!(
                "Desktop connection rejected: relay_id={} already in use",
                relay_id
            );
            let _ = ws_tx
                .send(Message::text(
                    serde_json::to_string(&RelayMessage::Control {
                        code: 409,
                        message: "relay_id_in_use".to_string(),
                    })
                    .unwrap(),
                ))
                .await;
            let _ = ws_tx.send(Message::close()).await;
            return;
        }
    }

    let last_heartbeat = Arc::new(Mutex::new(Instant::now()));

    let writer = tokio::task::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Register desktop
    {
        let mut desktops = state.desktops.lock().await;
        desktops.insert(
            relay_id.clone(),
            DesktopControl {
                tx: out_tx.clone(),
                public_key_fingerprint: public_key_fingerprint.clone(),
                last_heartbeat: last_heartbeat.clone(),
            },
        );
    }

    let _ = out_tx
        .send(Message::text(
            serde_json::to_string(&RelayMessage::Control {
                code: 200,
                message: "registered".to_string(),
            })
            .unwrap(),
        ))
        .await;

    log::info!(
        "Desktop registered relay_id={} fingerprint={:?}",
        relay_id,
        public_key_fingerprint
    );

    // Spawn heartbeat sender
    let heartbeat_tx = out_tx.clone();
    let heartbeat_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            if heartbeat_tx.send(Message::ping(vec![])).await.is_err() {
                break;
            }
        }
    });

    while let Some(result) = ws_rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(_) => break,
        };

        // Update heartbeat on any message (including pong)
        if msg.is_pong() {
            *last_heartbeat.lock().await = Instant::now();
            continue;
        }

        if msg.is_close() {
            break;
        }
        if !(msg.is_text() || msg.is_binary()) {
            continue;
        }

        // Update heartbeat on data messages too
        *last_heartbeat.lock().await = Instant::now();

        let parsed: RelayMessage = match serde_json::from_slice(msg.as_bytes()) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match parsed {
            RelayMessage::SessionAccept { session_id } => {
                // Desktop accepted: promote pending -> active session and notify mobile.
                let pending = {
                    let mut pending = state.pending.lock().await;
                    pending.remove(&session_id)
                };

                let pending = match pending {
                    Some(p) => p,
                    None => continue,
                };
                if pending.relay_id != relay_id {
                    continue;
                }

                {
                    let mut sessions = state.sessions.lock().await;
                    sessions.insert(
                        session_id.clone(),
                        SessionInfo {
                            relay_id: relay_id.clone(),
                            slots: SessionSlots::default(),
                            desktop_fingerprint: public_key_fingerprint.clone(),
                            mobile_fingerprint: pending.mobile_fingerprint.clone(),
                        },
                    );
                }

                let _ = pending
                    .mobile_tx
                    .send(Message::text(
                        serde_json::to_string(&RelayMessage::Control {
                            code: 200,
                            message: format!("session_accepted:{session_id}"),
                        })
                        .unwrap(),
                    ))
                    .await;
                let _ = out_tx
                    .send(Message::text(
                        serde_json::to_string(&RelayMessage::Control {
                            code: 200,
                            message: format!("open_session:{session_id}"),
                        })
                        .unwrap(),
                    ))
                    .await;

                log::info!(
                    "Session accepted session_id={} relay_id={}",
                    session_id,
                    relay_id
                );
            }
            RelayMessage::Close { session_id, reason } => {
                // Desktop can force-close an active session.
                {
                    let mut sessions = state.sessions.lock().await;
                    sessions.remove(&session_id);
                }
                // Best-effort notify pending mobile (if any).
                let pending = {
                    let mut pending = state.pending.lock().await;
                    pending.remove(&session_id)
                };
                if let Some(p) = pending {
                    let _ = p
                        .mobile_tx
                        .send(Message::text(
                            serde_json::to_string(&RelayMessage::Close { session_id, reason })
                                .unwrap(),
                        ))
                        .await;
                }
            }
            _ => {}
        }
    }

    // Desktop disconnected: remove and close all pending + active sessions.
    heartbeat_handle.abort();
    {
        let mut desktops = state.desktops.lock().await;
        desktops.remove(&relay_id);
    }
    {
        let mut pending = state.pending.lock().await;
        let pending_ids: Vec<String> = pending
            .iter()
            .filter_map(|(sid, p)| {
                if p.relay_id == relay_id {
                    Some(sid.clone())
                } else {
                    None
                }
            })
            .collect();
        for sid in pending_ids {
            if let Some(p) = pending.remove(&sid) {
                let _ = p
                    .mobile_tx
                    .send(Message::text(
                        serde_json::to_string(&RelayMessage::Close {
                            session_id: sid,
                            reason: "desktop_disconnected".to_string(),
                        })
                        .unwrap(),
                    ))
                    .await;
            }
        }
    }
    {
        let mut sessions = state.sessions.lock().await;
        // Remove all sessions; tunnels will see missing entry and terminate.
        let ids: Vec<String> = sessions
            .iter()
            .filter_map(|(sid, s)| {
                if s.relay_id == relay_id {
                    Some(sid.clone())
                } else {
                    None
                }
            })
            .collect();
        for sid in ids {
            sessions.remove(&sid);
        }
    }

    let _ = writer.abort();
    log::info!("desktop disconnected relay_id={}", relay_id);
}

/// Authorize a desktop connection
/// Returns the public key fingerprint on success
pub fn authorize_desktop(secret: &str, auth: &Option<String>) -> Result<String, &'static str> {
    let Some(raw) = auth else {
        return Err("missing authorization header");
    };
    let token = raw
        .strip_prefix("Bearer ")
        .ok_or("invalid auth format")?
        .trim();

    // For now, use a simple shared secret comparison
    // In production, this should verify an ed25519 signature of a challenge
    // Format: "Bearer <relay_id>:<timestamp>:<hmac>"
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() < 3 {
        return Err("invalid token format");
    }

    let relay_id = parts[0];
    let timestamp: i64 = parts[1].parse().map_err(|_| "invalid timestamp")?;
    let provided_hmac = parts[2];

    // Check timestamp is within 5 minutes
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if (now - timestamp).abs() > 300 {
        return Err("timestamp expired");
    }

    // Verify HMAC
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{}:{}:{}", relay_id, timestamp, secret).hash(&mut hasher);
    let expected_hmac = format!("{:x}", hasher.finish());

    if provided_hmac != expected_hmac {
        return Err("invalid hmac");
    }

    // Return the relay_id as the "fingerprint" for now
    Ok(relay_id.to_string())
}

#[derive(Debug, serde::Deserialize)]
pub struct Claims {
    pub sub: String,
    #[allow(dead_code)]
    pub exp: usize,
    pub subscription_active: bool,
    /// Optional device fingerprint for session binding
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

pub async fn mobile_control(
    ws: WebSocket,
    relay_id: String,
    auth: Option<String>,
    state: Arc<State>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(CHANNEL_BUFFER_SIZE);

    let writer = tokio::task::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Extract device fingerprint from JWT claims
    let mobile_fingerprint = if state.auth_mode == AuthMode::Required {
        if let Some(secret) = &state.jwt_secret {
            match authorize(secret, auth) {
                Ok(claims) => {
                    if !claims.subscription_active {
                        let _ = out_tx
                            .send(Message::close_with(4403u16, "subscription_required"))
                            .await;
                        let _ = writer.abort();
                        return;
                    }
                    claims.device_fingerprint
                }
                Err(_) => {
                    let _ = out_tx
                        .send(Message::close_with(4401u16, "unauthorized"))
                        .await;
                    let _ = writer.abort();
                    return;
                }
            }
        } else {
            log::warn!("Mobile connection rejected: no LUCIDITY_RELAY_JWT_SECRET configured");
            let _ = out_tx
                .send(Message::close_with(4401u16, "auth_not_configured"))
                .await;
            let _ = writer.abort();
            return;
        }
    } else {
        None
    };

    // First message must be Connect.
    let first = loop {
        match ws_rx.next().await {
            None => {
                let _ = writer.abort();
                return;
            }
            Some(Err(_)) => {
                let _ = writer.abort();
                return;
            }
            Some(Ok(m)) => {
                if m.is_text() || m.is_binary() {
                    break m;
                }
            }
        }
    };

    let first_msg: RelayMessage = match serde_json::from_slice(first.as_bytes()) {
        Ok(m) => m,
        Err(e) => {
            let _ = out_tx
                .send(Message::text(
                    serde_json::to_string(&RelayMessage::Control {
                        code: 400,
                        message: format!("invalid connect: {e}"),
                    })
                    .unwrap(),
                ))
                .await;
            let _ = out_tx.send(Message::close()).await;
            let _ = writer.abort();
            return;
        }
    };

    let client_id = match first_msg {
        RelayMessage::Connect {
            relay_id: rid,
            pairing_client_id,
        } if rid == relay_id => pairing_client_id,
        _ => {
            let _ = out_tx
                .send(Message::text(
                    serde_json::to_string(&RelayMessage::Control {
                        code: 400,
                        message: "expected connect".to_string(),
                    })
                    .unwrap(),
                ))
                .await;
            let _ = out_tx.send(Message::close()).await;
            let _ = writer.abort();
            return;
        }
    };

    // Must have an online desktop.
    let desktop = {
        let desktops = state.desktops.lock().await;
        desktops.get(&relay_id).cloned()
    };
    let Some(desktop) = desktop else {
        let _ = out_tx
            .send(Message::text(
                serde_json::to_string(&RelayMessage::Control {
                    code: 404,
                    message: "desktop_offline".to_string(),
                })
                .unwrap(),
            ))
            .await;
        let _ = out_tx.send(Message::close()).await;
        let _ = writer.abort();
        return;
    };

    let session_id = Uuid::new_v4().to_string();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(
            session_id.clone(),
            PendingSession {
                relay_id: relay_id.clone(),
                client_id: client_id.clone(),
                mobile_tx: out_tx.clone(),
                mobile_fingerprint: mobile_fingerprint.clone(),
            },
        );
    }

    let _ = desktop
        .tx
        .send(Message::text(
            serde_json::to_string(&RelayMessage::SessionRequest {
                session_id: session_id.clone(),
                client_id: client_id.clone(),
            })
            .unwrap(),
        ))
        .await;

    let _ = out_tx
        .send(Message::text(
            serde_json::to_string(&RelayMessage::Control {
                code: 200,
                message: format!("session_created:{session_id}"),
            })
            .unwrap(),
        ))
        .await;

    log::info!(
        "Mobile connected relay_id={} client_id={} session_id={} fingerprint={:?}",
        relay_id,
        client_id,
        session_id,
        mobile_fingerprint
    );

    // Keep socket open; mobile will open data tunnel after accept.
    while let Some(result) = ws_rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(_) => break,
        };
        if msg.is_close() {
            break;
        }
        // Ignore anything else on control socket for now.
    }

    // Control socket closed: remove pending (if still pending) and notify desktop
    {
        let mut pending = state.pending.lock().await;
        if let Some(_p) = pending.remove(&session_id) {
            // Notify desktop that mobile disconnected before acceptance
            let desktops = state.desktops.lock().await;
            if let Some(desktop) = desktops.get(&relay_id) {
                let _ = desktop
                    .tx
                    .send(Message::text(
                        serde_json::to_string(&RelayMessage::Close {
                            session_id: session_id.clone(),
                            reason: "mobile_disconnected".to_string(),
                        })
                        .unwrap(),
                    ))
                    .await;
            }
        }
    }

    let _ = writer.abort();
    log::info!(
        "mobile control disconnected relay_id={} client_id={} session_id={}",
        relay_id,
        client_id,
        session_id
    );
}

pub fn authorize(secret: &str, auth: Option<String>) -> Result<Claims, ()> {
    let Some(raw) = auth else {
        return Err(());
    };
    let token = raw.strip_prefix("Bearer ").ok_or(())?.trim();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let decoded = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ())?;
    if decoded.claims.sub.trim().is_empty() {
        return Err(());
    }
    Ok(decoded.claims)
}

pub async fn session_tunnel(
    ws: WebSocket,
    session_id: String,
    q: HashMap<String, String>,
    state: Arc<State>,
) {
    let role = q
        .get("role")
        .and_then(|v| SessionRole::parse(v))
        .unwrap_or(SessionRole::Mobile);

    // Get the participant fingerprint from query params (for validation)
    let provided_fingerprint = q.get("fingerprint").cloned();

    // Must be an active accepted session.
    let session_info = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .map(|s| (s.desktop_fingerprint.clone(), s.mobile_fingerprint.clone()))
    };

    let Some((desktop_fp, mobile_fp)) = session_info else {
        let (mut tx, _rx) = ws.split();
        let _ = tx
            .send(Message::close_with(4404u16, "unknown_session"))
            .await;
        return;
    };

    // Validate participant identity if auth is required
    if state.auth_mode == AuthMode::Required {
        let expected_fp = match role {
            SessionRole::Desktop => desktop_fp,
            SessionRole::Mobile => mobile_fp,
        };

        // If we have an expected fingerprint, the connecting party must match
        if let Some(ref expected) = expected_fp {
            match &provided_fingerprint {
                Some(provided) if provided == expected => {
                    // OK - fingerprint matches
                }
                Some(provided) => {
                    log::warn!(
                        "Session tunnel rejected: fingerprint mismatch for session_id={} role={:?} (expected={}, got={})",
                        session_id,
                        role,
                        expected,
                        provided
                    );
                    let (mut tx, _rx) = ws.split();
                    let _ = tx
                        .send(Message::close_with(4403u16, "fingerprint_mismatch"))
                        .await;
                    return;
                }
                None => {
                    log::warn!(
                        "Session tunnel rejected: no fingerprint provided for session_id={} role={:?}",
                        session_id,
                        role
                    );
                    let (mut tx, _rx) = ws.split();
                    let _ = tx
                        .send(Message::close_with(4401u16, "fingerprint_required"))
                        .await;
                    return;
                }
            }
        }
    }

    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(CHANNEL_BUFFER_SIZE);

    // Register our sender.
    {
        let mut sessions = state.sessions.lock().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            match role {
                SessionRole::Desktop => s.slots.desktop_tx = Some(out_tx.clone()),
                SessionRole::Mobile => s.slots.mobile_tx = Some(out_tx.clone()),
            }
        }
    }

    let writer = tokio::task::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    log::info!(
        "Session tunnel connected session_id={} role={:?} fingerprint={:?}",
        session_id,
        role,
        provided_fingerprint
    );

    while let Some(result) = ws_rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(_) => break,
        };
        if msg.is_close() {
            break;
        }
        if !(msg.is_binary() || msg.is_text()) {
            continue;
        }

        // Forward to the opposite side if present.
        let peer = {
            let sessions = state.sessions.lock().await;
            sessions.get(&session_id).and_then(|s| match role {
                SessionRole::Desktop => s.slots.mobile_tx.clone(),
                SessionRole::Mobile => s.slots.desktop_tx.clone(),
            })
        };
        if let Some(peer_tx) = peer {
            // Use try_send to avoid blocking; if channel is full, drop message (backpressure)
            if peer_tx.try_send(msg).is_err() {
                log::warn!(
                    "Dropping message for session_id={}: channel full (backpressure)",
                    session_id
                );
            }
        }
    }

    // Cleanup: clear our slot.
    {
        let mut sessions = state.sessions.lock().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            match role {
                SessionRole::Desktop => s.slots.desktop_tx = None,
                SessionRole::Mobile => s.slots.mobile_tx = None,
            }
        }
    }

    let _ = writer.abort();
    log::info!(
        "session tunnel disconnected session_id={} role={:?}",
        session_id,
        role
    );
}
