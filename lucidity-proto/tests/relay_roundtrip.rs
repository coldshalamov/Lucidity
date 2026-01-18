use k9::assert_equal;
use lucidity_proto::relay::RelayMessage;

#[test]
fn test_register_serialization() {
    let original = RelayMessage::Register {
        relay_id: "desktop-123".to_string(),
        signature: Some("sig-abc".to_string()),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_connect_serialization() {
    let original = RelayMessage::Connect {
        relay_id: "desktop-123".to_string(),
        pairing_client_id: "client-456".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_session_request_serialization() {
    let original = RelayMessage::SessionRequest {
        session_id: "session-789".to_string(),
        client_id: "client-456".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_session_accept_serialization() {
    let original = RelayMessage::SessionAccept {
        session_id: "session-789".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_data_serialization() {
    let original = RelayMessage::Data {
        session_id: "session-789".to_string(),
        payload: vec![1, 2, 3, 4, 5],
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_close_serialization() {
    let original = RelayMessage::Close {
        session_id: "session-789".to_string(),
        reason: "Client disconnected".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}

#[test]
fn test_control_serialization() {
    let original = RelayMessage::Control {
        code: 500,
        message: "Internal error".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let decoded: RelayMessage = serde_json::from_str(&json).unwrap();

    assert_equal!(decoded, original);
}
