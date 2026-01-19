use log::{info, warn};
use std::net::SocketAddr;
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let listen_addr: SocketAddr = std::env::var("LUCIDITY_RELAY_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()
        .expect("Invalid LUCIDITY_RELAY_LISTEN address");

    let relay_secret = std::env::var("LUCIDITY_RELAY_SECRET").ok();
    if relay_secret.is_some() {
        info!("🔐 Relay authentication ENABLED (LUCIDITY_RELAY_SECRET is set)");
    } else {
        warn!("⚠️  Relay authentication DISABLED (LUCIDITY_RELAY_SECRET is not set). Anyone can use this relay!");
    }

    let relay_server = std::sync::Arc::new(lucidity_relay::RelayServer::new());

    info!("🚀 Lucidity Relay Server starting on {}", listen_addr);

    // Health check endpoint
    let health = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

    let secret_filter = warp::query::<std::collections::HashMap<String, String>>();

    // Desktop WebSocket endpoint: /desktop/{relay_id}?secret=...
    let relay_server_desktop = relay_server.clone();
    let secret_desktop = relay_secret.clone();
    let desktop_route = warp::path!("desktop" / String)
        .and(warp::ws())
        .and(secret_filter)
        .map(move |relay_id: String, ws: warp::ws::Ws, query: std::collections::HashMap<String, String>| {
            let server = relay_server_desktop.clone();
            let expected = secret_desktop.clone();
            
            ws.on_upgrade(move |websocket| async move {
                if let Some(expected_secret) = expected {
                    let provided = query.get("secret");
                    if provided != Some(&expected_secret) {
                        warn!("Desktop connection REJECTED: invalid secret for relay_id={}", relay_id);
                        return;
                    }
                }
                
                if let Err(e) = server.handle_desktop(relay_id, websocket).await {
                    warn!("Desktop handler error: {}", e);
                }
            })
        });

    // Mobile WebSocket endpoint: /mobile/{relay_id}?secret=...
    let relay_server_mobile = relay_server.clone();
    let secret_mobile = relay_secret.clone();
    let mobile_route = warp::path!("mobile" / String)
        .and(warp::ws())
        .and(secret_filter)
        .map(move |relay_id: String, ws: warp::ws::Ws, query: std::collections::HashMap<String, String>| {
            let server = relay_server_mobile.clone();
            let expected = secret_mobile.clone();
            
            ws.on_upgrade(move |websocket| async move {
                if let Some(expected_secret) = expected {
                    let provided = query.get("secret");
                    if provided != Some(&expected_secret) {
                        warn!("Mobile connection REJECTED: invalid secret for relay_id={}", relay_id);
                        return;
                    }
                }

                if let Err(e) = server.handle_mobile(relay_id, websocket).await {
                    warn!("Mobile handler error: {}", e);
                }
            })
        });

    let routes = health.or(desktop_route).or(mobile_route);

    info!("✅ Relay server ready");
    info!("   Health: http://{}/health", listen_addr);
    info!("   Endpoints: /desktop/{{id}} and /mobile/{{id}}");

    warp::serve(routes).run(listen_addr).await;
}
