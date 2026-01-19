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

    let relay_server = std::sync::Arc::new(lucidity_relay::RelayServer::new());

    info!("🚀 Lucidity Relay Server starting on {}", listen_addr);

    // Health check endpoint
    let health = warp::path("health")
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({"status": "ok"})));

    // Desktop WebSocket endpoint: /desktop/{relay_id}
    let relay_server_desktop = relay_server.clone();
    let desktop_route = warp::path!("desktop" / String)
        .and(warp::ws())
        .map(move |relay_id: String, ws: warp::ws::Ws| {
            let server = relay_server_desktop.clone();
            ws.on_upgrade(move |websocket| async move {
                let ws_stream = websocket;
                if let Err(e) = server.handle_desktop(relay_id, ws_stream).await {
                    warn!("Desktop handler error: {}", e);
                }
            })
        });

    // Mobile WebSocket endpoint: /mobile/{relay_id}
    let relay_server_mobile = relay_server.clone();
    let mobile_route = warp::path!("mobile" / String)
        .and(warp::ws())
        .map(move |relay_id: String, ws: warp::ws::Ws| {
            let server = relay_server_mobile.clone();
            ws.on_upgrade(move |websocket| async move {
                let ws_stream = websocket;
                if let Err(e) = server.handle_mobile(relay_id, ws_stream).await {
                    warn!("Mobile handler error: {}", e);
                }
            })
        });

    let routes = health.or(desktop_route).or(mobile_route);

    info!("✅ Relay server ready");
    info!("   Health: http://{}/health", listen_addr);
    info!("   Desktop: ws://{}/desktop/{{relay_id}}", listen_addr);
    info!("   Mobile: ws://{}/mobile/{{relay_id}}", listen_addr);

    warp::serve(routes).run(listen_addr).await;
}
