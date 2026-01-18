use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Filter;

use lucidity_proto::relay::RelayMessage;

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Clone)]
struct DesktopControl {
    tx: Tx,
}

#[derive(Clone)]
struct PendingSession {
    relay_id: String,
    client_id: String,
    mobile_tx: Tx,
}

#[derive(Default)]
struct SessionSlots {
    desktop_tx: Option<Tx>,
    mobile_tx: Option<Tx>,
}

struct SessionInfo {
    relay_id: String,
    slots: SessionSlots,
}

#[derive(Default)]
struct State {
    desktops: Mutex<HashMap<String, DesktopControl>>,
    pending: Mutex<HashMap<String, PendingSession>>,
    sessions: Mutex<HashMap<String, SessionInfo>>,
    jwt_secret: Option<Arc<String>>,
}

#[derive(Clone, Copy, Debug)]
enum SessionRole {
    Desktop,
    Mobile,
}

impl SessionRole {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "desktop" => Some(SessionRole::Desktop),
            "mobile" => Some(SessionRole::Mobile),
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let listen: SocketAddr = std::env::var("LUCIDITY_RELAY_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()
        .expect("invalid LUCIDITY_RELAY_LISTEN (expected host:port)");

    let jwt_secret = match std::env::var("LUCIDITY_RELAY_JWT_SECRET") {
        Ok(s) if !s.trim().is_empty() => Some(Arc::new(s)),
        _ => None,
    };

    let state = Arc::new(State {
        jwt_secret,
        ..State::default()
    });
    let with_state = {
        let state = state.clone();
        warp::any().map(move || state.clone())
    };

    let hello = warp::path::end().map(|| "Lucidity Relay is Active");
    let healthz = warp::path!("healthz").map(|| "ok");

    // Control-plane websockets.
    let ws_desktop = warp::path!("ws" / "desktop" / String)
        .and(warp::ws())
        .and(with_state.clone())
        .map(|relay_id: String, ws: warp::ws::Ws, state: Arc<State>| {
            ws.on_upgrade(move |socket| desktop_control(socket, relay_id, state))
        });

    let ws_mobile = warp::path!("ws" / "mobile" / String)
        .and(warp::ws())
        .and(with_state.clone())
        .and(warp::header::optional::<String>("authorization"))
        .map(|relay_id: String, ws: warp::ws::Ws, state: Arc<State>, auth: Option<String>| {
            ws.on_upgrade(move |socket| mobile_control(socket, relay_id, auth, state))
        });

    // Data-plane websockets: /ws/session/<session_id>?role=desktop|mobile
    let ws_session = warp::path!("ws" / "session" / String)
        .and(warp::ws())
        .and(warp::query::<HashMap<String, String>>())
        .and(with_state)
        .map(|session_id: String, ws: warp::ws::Ws, q: HashMap<String, String>, state: Arc<State>| {
            ws.on_upgrade(move |socket| session_tunnel(socket, session_id, q, state))
        });

    let routes = hello
        .or(healthz)
        .or(ws_desktop)
        .or(ws_mobile)
        .or(ws_session)
        .with(warp::cors().allow_any_origin())
        .with(warp::log("lucidity_relay"));

    log::info!("lucidity-relay listening on {}", listen);
    warp::serve(routes).run(listen).await;
}

async fn desktop_control(ws: WebSocket, relay_id: String, state: Arc<State>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

    let writer = tokio::task::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Register desktop immediately (path contains relay_id).
    {
        let mut desktops = state.desktops.lock().await;
        desktops.insert(relay_id.clone(), DesktopControl { tx: out_tx.clone() });
    }

    let _ = out_tx.send(Message::text(
        serde_json::to_string(&RelayMessage::Control {
            code: 200,
            message: "registered".to_string(),
        })
        .unwrap(),
    ));

    while let Some(result) = ws_rx.next().await {
        let msg = match result {
            Ok(m) => m,
            Err(_) => break,
        };
        if msg.is_close() {
            break;
        }
        if !(msg.is_text() || msg.is_binary()) {
            continue;
        }

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
                        },
                    );
                }

                let _ = pending.mobile_tx.send(Message::text(
                    serde_json::to_string(&RelayMessage::Control {
                        code: 200,
                        message: format!("session_accepted:{session_id}"),
                    })
                    .unwrap(),
                ));
                let _ = out_tx.send(Message::text(
                    serde_json::to_string(&RelayMessage::Control {
                        code: 200,
                        message: format!("open_session:{session_id}"),
                    })
                    .unwrap(),
                ));
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
                    let _ = p.mobile_tx.send(Message::text(
                        serde_json::to_string(&RelayMessage::Close { session_id, reason }).unwrap(),
                    ));
                }
            }
            _ => {}
        }
    }

    // Desktop disconnected: remove and close all pending + active sessions.
    {
        let mut desktops = state.desktops.lock().await;
        desktops.remove(&relay_id);
    }
    {
        let mut pending = state.pending.lock().await;
        let pending_ids: Vec<String> = pending
            .iter()
            .filter_map(|(sid, p)| if p.relay_id == relay_id { Some(sid.clone()) } else { None })
            .collect();
        for sid in pending_ids {
            if let Some(p) = pending.remove(&sid) {
                let _ = p.mobile_tx.send(Message::text(
                    serde_json::to_string(&RelayMessage::Close {
                        session_id: sid,
                        reason: "desktop_disconnected".to_string(),
                    })
                    .unwrap(),
                ));
            }
        }
    }
    {
        let mut sessions = state.sessions.lock().await;
        // Remove all sessions; tunnels will see missing entry and terminate.
        let ids: Vec<String> = sessions
            .iter()
            .filter_map(|(sid, s)| if s.relay_id == relay_id { Some(sid.clone()) } else { None })
            .collect();
        for sid in ids {
            sessions.remove(&sid);
        }
    }

    let _ = writer.abort();
    log::info!("desktop disconnected relay_id={}", relay_id);
}

#[derive(Debug, serde::Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    subscription_active: bool,
}

async fn mobile_control(ws: WebSocket, relay_id: String, auth: Option<String>, state: Arc<State>) {
    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

    let writer = tokio::task::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    if let Some(secret) = &state.jwt_secret {
        match authorize(secret, auth) {
            Ok(claims) => {
                if !claims.subscription_active {
                    let _ = out_tx.send(Message::close_with(4403u16, "subscription_required"));
                    let _ = writer.abort();
                    return;
                }
            }
            Err(_) => {
                let _ = out_tx.send(Message::close_with(4401u16, "unauthorized"));
                let _ = writer.abort();
                return;
            }
        }
    }

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
            let _ = out_tx.send(Message::text(
                serde_json::to_string(&RelayMessage::Control {
                    code: 400,
                    message: format!("invalid connect: {e}"),
                })
                .unwrap(),
            ));
            let _ = out_tx.send(Message::close());
            let _ = writer.abort();
            return;
        }
    };

    let client_id = match first_msg {
        RelayMessage::Connect { relay_id: rid, pairing_client_id } if rid == relay_id => pairing_client_id,
        _ => {
            let _ = out_tx.send(Message::text(
                serde_json::to_string(&RelayMessage::Control {
                    code: 400,
                    message: "expected connect".to_string(),
                })
                .unwrap(),
            ));
            let _ = out_tx.send(Message::close());
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
        let _ = out_tx.send(Message::text(
            serde_json::to_string(&RelayMessage::Control {
                code: 404,
                message: "desktop_offline".to_string(),
            })
            .unwrap(),
        ));
        let _ = out_tx.send(Message::close());
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
            },
        );
    }

    let _ = desktop.tx.send(Message::text(
        serde_json::to_string(&RelayMessage::SessionRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
        })
        .unwrap(),
    ));

    let _ = out_tx.send(Message::text(
        serde_json::to_string(&RelayMessage::Control {
            code: 200,
            message: format!("session_created:{session_id}"),
        })
        .unwrap(),
    ));

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

    // Control socket closed: remove pending (if still pending).
    {
        let mut pending = state.pending.lock().await;
        pending.remove(&session_id);
    }

    let _ = writer.abort();
    log::info!(
        "mobile control disconnected relay_id={} client_id={} session_id={}",
        relay_id,
        client_id,
        session_id
    );
}

fn authorize(secret: &str, auth: Option<String>) -> Result<Claims, ()> {
    let Some(raw) = auth else { return Err(()); };
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

async fn session_tunnel(ws: WebSocket, session_id: String, q: HashMap<String, String>, state: Arc<State>) {
    let role = q
        .get("role")
        .and_then(|v| SessionRole::parse(v))
        .unwrap_or(SessionRole::Mobile);

    // Must be an active accepted session.
    {
        let sessions = state.sessions.lock().await;
        if !sessions.contains_key(&session_id) {
            let (mut tx, _rx) = ws.split();
            let _ = tx.send(Message::close_with(4404u16, "unknown_session")).await;
            return;
        }
    }

    let (mut ws_tx, mut ws_rx) = ws.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

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
            let _ = peer_tx.send(msg);
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
    log::info!("session tunnel disconnected session_id={} role={:?}", session_id, role);
}
