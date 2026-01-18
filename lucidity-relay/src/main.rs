//! Lucidity Relay Server
//!
//! A secure WebSocket relay server that brokers connections between desktop and mobile clients.
//!
//! # Security Model
//! - Desktop authentication via signature verification (ed25519)
//! - Session participant validation prevents hijacking
//! - Bounded channels prevent DoS via memory exhaustion
//! - Heartbeat mechanism detects dead connections
//! - Explicit opt-in required to disable authentication

use std::net::SocketAddr;
use std::sync::Arc;

use lucidity_relay::{
    desktop_control, heartbeat_checker, mobile_control, session_tunnel, AuthMode, State,
};
use warp::Filter;

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

    let desktop_secret = match std::env::var("LUCIDITY_RELAY_DESKTOP_SECRET") {
        Ok(s) if !s.trim().is_empty() => Some(Arc::new(s)),
        _ => None,
    };

    // Auth mode: require explicit LUCIDITY_RELAY_NO_AUTH=true to disable
    let auth_mode = match std::env::var("LUCIDITY_RELAY_NO_AUTH") {
        Ok(s) if s.to_lowercase() == "true" || s == "1" => {
            log::warn!(
                "⚠️  LUCIDITY_RELAY_NO_AUTH=true - Authentication DISABLED. DO NOT USE IN PRODUCTION!"
            );
            AuthMode::Disabled
        }
        _ => {
            if jwt_secret.is_none() && desktop_secret.is_none() {
                log::error!("❌ No authentication configured!");
                log::error!("   Set LUCIDITY_RELAY_JWT_SECRET for mobile auth");
                log::error!("   Set LUCIDITY_RELAY_DESKTOP_SECRET for desktop auth");
                log::error!("   Or set LUCIDITY_RELAY_NO_AUTH=true for development (insecure)");
                std::process::exit(1);
            }
            AuthMode::Required
        }
    };

    let state = Arc::new(State {
        jwt_secret,
        desktop_secret,
        auth_mode,
        ..State::default()
    });

    // Spawn heartbeat checker
    {
        let state = state.clone();
        tokio::spawn(async move {
            heartbeat_checker(state).await;
        });
    }

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
        .and(warp::header::optional::<String>("authorization"))
        .map(
            |relay_id: String, ws: warp::ws::Ws, state: Arc<State>, auth: Option<String>| {
                ws.on_upgrade(move |socket| desktop_control(socket, relay_id, auth, state))
            },
        );

    let ws_mobile = warp::path!("ws" / "mobile" / String)
        .and(warp::ws())
        .and(with_state.clone())
        .and(warp::header::optional::<String>("authorization"))
        .map(
            |relay_id: String, ws: warp::ws::Ws, state: Arc<State>, auth: Option<String>| {
                ws.on_upgrade(move |socket| mobile_control(socket, relay_id, auth, state))
            },
        );

    // Data-plane websockets: /ws/session/<session_id>?role=desktop|mobile&token=<session_token>
    let ws_session = warp::path!("ws" / "session" / String)
        .and(warp::ws())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(with_state)
        .map(
            |session_id: String,
             ws: warp::ws::Ws,
             q: std::collections::HashMap<String, String>,
             state: Arc<State>| {
                ws.on_upgrade(move |socket| session_tunnel(socket, session_id, q, state))
            },
        );

    let routes = hello
        .or(healthz)
        .or(ws_desktop)
        .or(ws_mobile)
        .or(ws_session)
        .with(warp::cors().allow_any_origin())
        .with(warp::log("lucidity_relay"));

    log::info!("lucidity-relay listening on {}", listen);
    log::info!(
        "Auth mode: {}",
        match auth_mode {
            AuthMode::Required => "REQUIRED",
            AuthMode::Disabled => "DISABLED (insecure)",
        }
    );
    warp::serve(routes).run(listen).await;
}
