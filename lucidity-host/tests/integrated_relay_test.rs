use lucidity_host::{
    FakePaneBridge, PaneInfo, TYPE_JSON, 
};
use lucidity_proto::frame::{encode_frame, FrameDecoder};
use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[tokio::test]
async fn relay_end_to_end_test() {
    let dir = tempfile::tempdir().unwrap();
    let host_kp_path = dir.path().join("host_keypair.json");
    let device_db_path = dir.path().join("devices.db");
    
    std::env::set_var("LUCIDITY_HOST_KEYPAIR", &host_kp_path);
    std::env::set_var("LUCIDITY_DEVICE_TRUST_DB", &device_db_path);
    std::env::set_var("LUCIDITY_RELAY_URL", "ws://127.0.0.1:9090");
    std::env::set_var("LUCIDITY_RELAY_SECRET", "test-secret-123");
    
    // 1. Start Relay Server
    let relay_addr: std::net::SocketAddr = "127.0.0.1:9090".parse().unwrap();
    let relay_server = Arc::new(lucidity_relay::RelayServer::new());
    
    // We use a simplified version of the relay routes for testing
    let relay_server_desktop = relay_server.clone();
    let relay_server_mobile = relay_server.clone();
    
    tokio::spawn(async move {
        // Warp filter for desktop /desktop/{id}
        use warp::Filter;
        let secret_filter = warp::query::<std::collections::HashMap<String, String>>();
        
        let d_route = warp::path!("desktop" / String)
            .and(warp::ws())
            .and(secret_filter.clone())
            .map(move |id, ws: warp::ws::Ws, q: std::collections::HashMap<String, String>| {
                 let s = relay_server_desktop.clone();
                 ws.on_upgrade(move |websocket| async move {
                     if q.get("secret").map(|s| s.as_str()) != Some("test-secret-123") { return; }
                     let _ = s.handle_desktop(id, websocket).await;
                 })
            });

        let m_route = warp::path!("mobile" / String)
            .and(warp::ws())
            .and(secret_filter)
            .map(move |id, ws: warp::ws::Ws, q: std::collections::HashMap<String, String>| {
                 let s = relay_server_mobile.clone();
                 ws.on_upgrade(move |websocket| async move {
                     if q.get("secret").map(|s| s.as_str()) != Some("test-secret-123") { return; }
                     let _ = s.handle_mobile(id, websocket).await;
                 })
            });
            
        warp::serve(d_route.or(m_route)).run(relay_addr).await;
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // 2. Start Host
    // In tests, we manually trigger the relay client connection logic parts
    let keypair = lucidity_pairing::KeypairStore::open(host_kp_path).load_or_generate().unwrap();
    let pubkey_b64 = keypair.public_key().to_base64();
    let relay_id = pubkey_b64.chars().take(16).collect::<String>();
    
    let fake_bridge = Arc::new(FakePaneBridge::new(vec![PaneInfo {
        pane_id: 1,
        title: "relay-test-pane".to_string(),
    }]));
    
    let mut relay_client = lucidity_host::RelayClient::new("ws://127.0.0.1:9090".to_string(), relay_id.clone());
    relay_client.set_bridge(fake_bridge.clone());
    
    tokio::spawn(async move {
        relay_client.connect().await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // 3. Mock Mobile Client connects to Relay
    let mobile_url = format!("ws://127.0.0.1:9090/mobile/{}?secret=test-secret-123", relay_id);
    let (ws_stream, _) = connect_async(Url::parse(&mobile_url).unwrap()).await.unwrap();
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // 4. Send list_panes via Relay
    let list_req = serde_json::to_vec(&serde_json::json!({ "op": "list_panes" })).unwrap();
    ws_tx.send(Message::Binary(encode_frame(TYPE_JSON, &list_req))).await.unwrap();

    // 5. Expect Auth Challenge (because we are not localhost)
    let msg = ws_rx.next().await.unwrap().unwrap();
    let data = msg.into_data();
    let mut decoder = FrameDecoder::new();
    decoder.push(&data);
    let frame = decoder.next_frame().unwrap().unwrap();
    assert_eq!(frame.typ, TYPE_JSON);
    
    let v: serde_json::Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(v["op"], "auth_challenge");
    let nonce = v["nonce"].as_str().unwrap().to_string();

    // Consume the second frame: "authentication required" error
    let msg_err = ws_rx.next().await.unwrap().unwrap();
    let mut decoder = FrameDecoder::new();
    decoder.push(&msg_err.into_data());
    let frame_err = decoder.next_frame().unwrap().unwrap();
    let v_err: serde_json::Value = serde_json::from_slice(&frame_err.payload).unwrap();
    assert_eq!(v_err["op"], "error");
    assert!(v_err["message"].as_str().unwrap().contains("required"));

    // 6. Respond with Auth Response (mocking a paired device)
    // First, we need to add the mobile device to the trust store
    let mobile_kp = lucidity_pairing::Keypair::generate();
    let store = lucidity_pairing::DeviceTrustStore::open(&device_db_path).unwrap();
    store.add_device(&lucidity_pairing::TrustedDevice {
        public_key: mobile_kp.public_key(),
        user_email: "test@example.com".to_string(),
        device_name: "Test Mobile".to_string(),
        paired_at: 0,
        last_seen: None,
    }).unwrap();

    let sig = mobile_kp.sign(nonce.as_bytes()).to_base64();
    let auth_resp = serde_json::to_vec(&serde_json::json!({
        "op": "auth_response",
        "public_key": mobile_kp.public_key().to_base64(),
        "signature": sig,
    })).unwrap();
    ws_tx.send(Message::Binary(encode_frame(TYPE_JSON, &auth_resp))).await.unwrap();

    // 7. Expect Auth Success
    let msg = ws_rx.next().await.unwrap().unwrap();
    let mut decoder = FrameDecoder::new();
    decoder.push(&msg.into_data());
    let frame = decoder.next_frame().unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(v["op"], "auth_success");

    // 8. Now retry list_panes
    ws_tx.send(Message::Binary(encode_frame(TYPE_JSON, &list_req))).await.unwrap();
    let msg = ws_rx.next().await.unwrap().unwrap();
    let mut decoder = FrameDecoder::new();
    decoder.push(&msg.into_data());
    let frame = decoder.next_frame().unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&frame.payload).unwrap();
    assert_eq!(v["op"], "list_panes");
    assert_eq!(v["panes"][0]["title"], "relay-test-pane");

    // 9. Test Revocation
    let revoke_req = serde_json::to_vec(&serde_json::json!({
        "op": "revoke_device",
        "public_key": mobile_kp.public_key().to_base64(),
    })).unwrap();
    ws_tx.send(Message::Binary(encode_frame(TYPE_JSON, &revoke_req))).await.unwrap();
    
    // Catch the success/error message
    let _ = ws_rx.next().await.unwrap().unwrap();
    
    // Verify it's gone from the store
    assert!(!store.is_trusted(&mobile_kp.public_key()).unwrap());

    println!("✅ Revocation Test Passed!");
    println!("✅ Relay End-to-End Integrated Test Passed!");
}
