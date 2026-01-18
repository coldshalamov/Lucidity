//! Integration tests for lucidity-relay WebSocket server
//!
//! These tests verify the full WebSocket flow including:
//! - Server startup and health checks
//! - Desktop registration and duplicate rejection
//! - Mobile connection to online/offline desktops
//! - Session accept flow and notifications
//! - Data tunnel forwarding between peers
//! - Cleanup on disconnect

use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use warp::Filter;

use lucidity_proto::relay::RelayMessage;
use lucidity_relay::{desktop_control, mobile_control, session_tunnel, AuthMode, State};

/// Helper function to extract text from a message, skipping ping/pong
fn extract_text(msg: Message) -> String {
    match msg {
        Message::Text(t) => t,
        Message::Ping(_) | Message::Pong(_) => {
            // Skip ping/pong messages - these are heartbeat
            panic!("Got ping/pong, caller should filter these out")
        }
        other => panic!("Expected text message, got {:?}", other),
    }
}

/// Helper function to wait for a text message, skipping ping/pong
async fn wait_for_text<S>(stream: &mut S) -> Result<String, String>
where
    S: futures::StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        match tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
            Ok(Some(Ok(msg))) => match msg {
                Message::Text(t) => return Ok(t),
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => return Err("Connection closed".to_string()),
                _ => continue,
            },
            Ok(Some(Err(e))) => return Err(format!("WebSocket error: {}", e)),
            Ok(None) => return Err("Connection closed".to_string()),
            Err(_) => return Err("Timeout waiting for message".to_string()),
        }
    }
    Err("Timeout waiting for text message".to_string())
}

/// Helper function to spawn a test server on a specific port
async fn spawn_test_server(port: u16) -> tokio::task::JoinHandle<()> {
    std::env::set_var("LUCIDITY_RELAY_NO_AUTH", "true");
    std::env::set_var("LUCIDITY_RELAY_LISTEN", format!("127.0.0.1:{}", port));
    std::env::set_var("RUST_LOG", "warn"); // Reduce log noise in tests

    tokio::spawn(async move {
        let listen = format!("127.0.0.1:{}", port);
        let addr: std::net::SocketAddr = listen.parse().expect("Invalid listen address");

        let state = Arc::new(State {
            jwt_secret: None,
            desktop_secret: None,
            auth_mode: AuthMode::Disabled,
            ..State::default()
        });

        let with_state = warp::any().map(move || state.clone());
        let healthz = warp::path!("healthz").map(|| "ok");

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

        let routes = healthz.or(ws_desktop).or(ws_mobile).or(ws_session);

        warp::serve(routes).run(addr).await;
    })
}

/// Helper to connect a desktop client
async fn connect_desktop(
    port: u16,
    relay_id: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://127.0.0.1:{}/ws/desktop/{}", port, relay_id);
    let (ws_stream, _) = connect_async(&url)
        .await
        .expect("Failed to connect desktop");
    ws_stream
}

/// Helper to connect a mobile client
async fn connect_mobile(
    port: u16,
    relay_id: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://127.0.0.1:{}/ws/mobile/{}", port, relay_id);
    let (ws_stream, _) = connect_async(&url).await.expect("Failed to connect mobile");
    ws_stream
}

/// Helper to connect a session tunnel
async fn connect_session_tunnel(
    port: u16,
    session_id: &str,
    role: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!(
        "ws://127.0.0.1:{}/ws/session/{}?role={}",
        port, session_id, role
    );
    let (ws_stream, _) = connect_async(&url)
        .await
        .expect("Failed to connect session tunnel");
    ws_stream
}

#[tokio::test]
async fn test_server_starts_and_responds_to_healthz() {
    let port = 19790;
    let _server = spawn_test_server(port).await;

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://127.0.0.1:{}/healthz", port))
        .send()
        .await
        .expect("Failed to send healthz request");

    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("Failed to read response body");
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn test_desktop_registers_successfully() {
    let port = 19791;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut ws = connect_desktop(port, "test-desktop-1").await;

    // Wait for registration confirmation
    let msg = timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("Timeout waiting for registration message")
        .expect("Connection closed");

    let msg = msg.expect("Failed to receive message");
    assert!(msg.is_text());

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse RelayMessage");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message, got {:?}", relay_msg),
    }

    // Close connection
    ws.close(None).await.expect("Failed to close connection");
}

#[tokio::test]
async fn test_desktop_rejects_duplicate_relay_id() {
    let port = 19792;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // First desktop should register successfully
    let mut ws1 = connect_desktop(port, "duplicate-desktop").await;

    let msg = timeout(Duration::from_secs(5), ws1.next())
        .await
        .expect("Timeout waiting for first registration")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse first registration");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message for first desktop"),
    }

    // Second desktop with same relay_id should be rejected
    let mut ws2 = connect_desktop(port, "duplicate-desktop").await;

    let msg = timeout(Duration::from_secs(5), ws2.next())
        .await
        .expect("Timeout waiting for rejection")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse rejection");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 409);
            assert_eq!(message, "relay_id_in_use");
        }
        _ => panic!("Expected Control message with 409 for duplicate"),
    }

    // Verify ws2 closes
    let msg = timeout(Duration::from_secs(2), ws2.next())
        .await
        .expect("Timeout waiting for close");

    assert!(msg.is_some());
    let msg = msg.unwrap().expect("Failed to receive message");
    assert!(msg.is_close());

    // Clean up first connection
    ws1.close(None).await.ok();
}

#[tokio::test]
async fn test_mobile_connects_to_online_desktop() {
    let port = 19793;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register desktop first
    let mut desktop_ws = connect_desktop(port, "online-desktop").await;

    let msg = timeout(Duration::from_secs(5), desktop_ws.next())
        .await
        .expect("Timeout waiting for desktop registration")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse registration");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message"),
    }

    // Connect mobile
    let mut mobile_ws = connect_mobile(port, "online-desktop").await;

    // Send Connect message
    let connect_msg = RelayMessage::Connect {
        relay_id: "online-desktop".to_string(),
        pairing_client_id: "mobile-client-1".to_string(),
    };

    let connect_json = serde_json::to_string(&connect_msg).unwrap();
    mobile_ws
        .send(Message::text(connect_json))
        .await
        .expect("Failed to send Connect");

    // Receive session_created response
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_created")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse session_created");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert!(message.starts_with("session_created:"));
        }
        _ => panic!("Expected Control message with session_created"),
    }

    // Desktop should receive SessionRequest (skip ping/pong messages)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), desktop_ws.next())
            .await
            .expect("Timeout waiting for SessionRequest")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let relay_msg: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse SessionRequest");

    match relay_msg {
        RelayMessage::SessionRequest {
            session_id,
            client_id,
        } => {
            assert_eq!(client_id, "mobile-client-1");
            assert!(!session_id.is_empty());
        }
        _ => panic!("Expected SessionRequest message"),
    }

    // Clean up
    mobile_ws.close(None).await.ok();
    desktop_ws.close(None).await.ok();
}

#[tokio::test]
async fn test_mobile_gets_404_for_offline_desktop() {
    let port = 19794;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect mobile to non-existent desktop
    let mut mobile_ws = connect_mobile(port, "offline-desktop").await;

    // Send Connect message
    let connect_msg = RelayMessage::Connect {
        relay_id: "offline-desktop".to_string(),
        pairing_client_id: "mobile-client-2".to_string(),
    };

    let connect_json = serde_json::to_string(&connect_msg).unwrap();
    mobile_ws
        .send(Message::text(connect_json))
        .await
        .expect("Failed to send Connect");

    // Receive 404 error (skip ping/pong)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), mobile_ws.next())
            .await
            .expect("Timeout waiting for 404 response")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let relay_msg: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse 404 response");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 404);
            assert_eq!(message, "desktop_offline");
        }
        _ => panic!("Expected Control message with 404"),
    }

    // Connection should close
    let msg = timeout(Duration::from_secs(2), mobile_ws.next())
        .await
        .expect("Timeout waiting for close");

    assert!(msg.is_some());
    let msg = msg.unwrap().expect("Failed to receive message");
    assert!(msg.is_close());
}

#[tokio::test]
async fn test_session_accept_flow() {
    let port = 19795;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register desktop
    let mut desktop_ws = connect_desktop(port, "accept-test-desktop").await;

    let msg = timeout(Duration::from_secs(5), desktop_ws.next())
        .await
        .expect("Timeout waiting for desktop registration")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse registration");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message"),
    }

    // Connect mobile
    let mut mobile_ws = connect_mobile(port, "accept-test-desktop").await;

    let connect_msg = RelayMessage::Connect {
        relay_id: "accept-test-desktop".to_string(),
        pairing_client_id: "mobile-client-accept".to_string(),
    };

    let connect_json = serde_json::to_string(&connect_msg).unwrap();
    mobile_ws
        .send(Message::text(connect_json))
        .await
        .expect("Failed to send Connect");

    // Get session_created from mobile
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_created")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let session_id = match serde_json::from_str::<RelayMessage>(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse session_created")
    {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert!(message.starts_with("session_created:"));
            message
                .strip_prefix("session_created:")
                .unwrap()
                .to_string()
        }
        _ => panic!("Expected Control message"),
    };

    // Get SessionRequest on desktop (skip ping/pong messages)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), desktop_ws.next())
            .await
            .expect("Timeout waiting for SessionRequest")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let relay_msg: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse SessionRequest");

    match &relay_msg {
        RelayMessage::SessionRequest { .. } => {}
        _ => panic!("Expected SessionRequest message"),
    }

    // Desktop accepts session
    let accept_msg = RelayMessage::SessionAccept {
        session_id: session_id.clone(),
    };

    let accept_json = serde_json::to_string(&accept_msg).unwrap();
    desktop_ws
        .send(Message::text(accept_json))
        .await
        .expect("Failed to send SessionAccept");

    // Mobile should receive session_accepted notification
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_accepted")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse session_accepted");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, format!("session_accepted:{}", session_id));
        }
        _ => panic!("Expected Control message with session_accepted"),
    }

    // Desktop should also receive open_session notification
    let msg = timeout(Duration::from_secs(5), desktop_ws.next())
        .await
        .expect("Timeout waiting for open_session")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse open_session");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, format!("open_session:{}", session_id));
        }
        _ => panic!("Expected Control message with open_session"),
    }

    // Clean up
    mobile_ws.close(None).await.ok();
    desktop_ws.close(None).await.ok();
}

#[tokio::test]
async fn test_session_tunnel_data_forwarding() {
    let port = 19796;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register desktop
    let mut desktop_ws = connect_desktop(port, "tunnel-test-desktop").await;

    let msg = timeout(Duration::from_secs(5), desktop_ws.next())
        .await
        .expect("Timeout waiting for desktop registration")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse registration");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message"),
    }

    // Connect mobile control
    let mut mobile_ws = connect_mobile(port, "tunnel-test-desktop").await;

    let connect_msg = RelayMessage::Connect {
        relay_id: "tunnel-test-desktop".to_string(),
        pairing_client_id: "mobile-client-tunnel".to_string(),
    };

    let connect_json = serde_json::to_string(&connect_msg).unwrap();
    mobile_ws
        .send(Message::text(connect_json))
        .await
        .expect("Failed to send Connect");

    // Get session_id
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_created")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let session_id = match serde_json::from_str::<RelayMessage>(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse session_created")
    {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            message
                .strip_prefix("session_created:")
                .unwrap()
                .to_string()
        }
        _ => panic!("Expected Control message"),
    };

    // Accept session (skip ping/pong)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), desktop_ws.next())
            .await
            .expect("Timeout waiting for SessionRequest")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let _session_request: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse SessionRequest");

    let accept_msg = RelayMessage::SessionAccept {
        session_id: session_id.clone(),
    };

    let accept_json = serde_json::to_string(&accept_msg).unwrap();
    desktop_ws
        .send(Message::text(accept_json))
        .await
        .expect("Failed to send SessionAccept");

    // Wait for session_accepted
    let _ = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_accepted")
        .expect("Connection closed")
        .expect("Failed to receive message");

    // Connect desktop tunnel
    let mut desktop_tunnel = connect_session_tunnel(port, &session_id, "desktop").await;

    // Connect mobile tunnel
    let mut mobile_tunnel = connect_session_tunnel(port, &session_id, "mobile").await;

    // Wait a bit for tunnels to establish
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send data from mobile to desktop
    let test_data = b"Hello from mobile!";
    mobile_tunnel
        .send(Message::binary(test_data.to_vec()))
        .await
        .expect("Failed to send data from mobile");

    // Desktop should receive the data
    let msg = timeout(Duration::from_secs(5), desktop_tunnel.next())
        .await
        .expect("Timeout waiting for data on desktop")
        .expect("Connection closed")
        .expect("Failed to receive message");

    assert!(msg.is_binary());
    if let Message::Binary(data) = msg {
        assert_eq!(data.as_slice(), test_data);
    } else {
        panic!("Expected binary message");
    }

    // Send data from desktop to mobile
    let test_data2 = b"Hello from desktop!";
    desktop_tunnel
        .send(Message::binary(test_data2.to_vec()))
        .await
        .expect("Failed to send data from desktop");

    // Mobile should receive the data
    let msg = timeout(Duration::from_secs(5), mobile_tunnel.next())
        .await
        .expect("Timeout waiting for data on mobile")
        .expect("Connection closed")
        .expect("Failed to receive message");

    assert!(msg.is_binary());
    if let Message::Binary(data) = msg {
        assert_eq!(data.as_slice(), test_data2);
    } else {
        panic!("Expected binary message");
    }

    // Clean up
    mobile_tunnel.close(None).await.ok();
    desktop_tunnel.close(None).await.ok();
    mobile_ws.close(None).await.ok();
    desktop_ws.close(None).await.ok();
}

#[tokio::test]
async fn test_cleanup_on_desktop_disconnect() {
    let port = 19797;
    let _server = spawn_test_server(port).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register desktop
    let mut desktop_ws = connect_desktop(port, "cleanup-test-desktop").await;

    // Wait for registration confirmation (skip ping/pong)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), desktop_ws.next())
            .await
            .expect("Timeout waiting for desktop registration")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let relay_msg: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse registration");

    match relay_msg {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            assert_eq!(message, "registered");
        }
        _ => panic!("Expected Control message"),
    }

    // Connect mobile (pending session)
    let mut mobile_ws = connect_mobile(port, "cleanup-test-desktop").await;

    let connect_msg = RelayMessage::Connect {
        relay_id: "cleanup-test-desktop".to_string(),
        pairing_client_id: "mobile-client-cleanup".to_string(),
    };

    let connect_json = serde_json::to_string(&connect_msg).unwrap();
    mobile_ws
        .send(Message::text(connect_json))
        .await
        .expect("Failed to send Connect");

    // Get session_id
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_created")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let session_id = match serde_json::from_str::<RelayMessage>(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse session_created")
    {
        RelayMessage::Control { code, message } => {
            assert_eq!(code, 200);
            message
                .strip_prefix("session_created:")
                .unwrap()
                .to_string()
        }
        _ => panic!("Expected Control message"),
    };

    // Accept session (skip ping/pong)
    let text_msg = loop {
        let msg = timeout(Duration::from_secs(5), desktop_ws.next())
            .await
            .expect("Timeout waiting for SessionRequest")
            .expect("Connection closed")
            .expect("Failed to receive message");

        if let Message::Text(t) = msg {
            break t;
        }
        // Skip ping/pong messages
    };

    let _session_request: RelayMessage =
        serde_json::from_str(&text_msg).expect("Failed to parse SessionRequest");

    let accept_msg = RelayMessage::SessionAccept {
        session_id: session_id.clone(),
    };

    let accept_json = serde_json::to_string(&accept_msg).unwrap();
    desktop_ws
        .send(Message::text(accept_json))
        .await
        .expect("Failed to send SessionAccept");

    // Wait for session_accepted
    let _ = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for session_accepted")
        .expect("Connection closed")
        .expect("Failed to receive message");

    // Connect tunnels
    let mut desktop_tunnel = connect_session_tunnel(port, &session_id, "desktop").await;
    let mut mobile_tunnel = connect_session_tunnel(port, &session_id, "mobile").await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Disconnect desktop control socket
    desktop_ws
        .close(None)
        .await
        .expect("Failed to close desktop");

    // Mobile control should receive close notification
    let msg = timeout(Duration::from_secs(5), mobile_ws.next())
        .await
        .expect("Timeout waiting for desktop_disconnected notification")
        .expect("Connection closed")
        .expect("Failed to receive message");

    let relay_msg: RelayMessage = serde_json::from_str(match msg {
        Message::Text(ref t) => t,
        _ => panic!("Expected text message"),
    })
    .expect("Failed to parse desktop_disconnected notification");

    match relay_msg {
        RelayMessage::Close {
            session_id: sid,
            reason,
        } => {
            assert_eq!(sid, session_id);
            assert_eq!(reason, "desktop_disconnected");
        }
        _ => panic!("Expected Close message"),
    }

    // Mobile tunnel should eventually close or fail to forward
    // Try to send data - it should not reach desktop (desktop is gone)
    mobile_tunnel
        .send(Message::binary(b"test".to_vec()))
        .await
        .expect("Failed to send data");

    // Wait a bit for cleanup to happen
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Clean up remaining connections
    mobile_tunnel.close(None).await.ok();
    desktop_tunnel.close(None).await.ok();
    mobile_ws.close(None).await.ok();
}
