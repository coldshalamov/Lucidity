use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    /// Desktop -> Relay: "I am ready to accept connections"
    Register {
        relay_id: String,
        /// Signed timestamp to prove ownership of the relay_id (optional for now, good for security later)
        signature: Option<String>,
    },
    
    /// Mobile -> Relay: "Connect me to this desktop"
    Connect {
        relay_id: String,
        /// The pairing payload proving authorization
        pairing_client_id: String, 
    },

    /// Relay -> Desktop: "A mobile client wants to connect"
    SessionRequest {
        session_id: String,
        client_id: String,
    },

    /// Desktop -> Relay: "I accept this session"
    SessionAccept {
        session_id: String,
    },

    /// Relay -> Desktop/Mobile: "Here is data for your session"
    Data {
        session_id: String,
        payload: Vec<u8>,
    },

    /// Relay -> Desktop/Mobile: "Session ended"
    Close {
        session_id: String,
        reason: String,
    },
    
    /// Relay -> Client: "Error / Ack"
    Control {
        code: u16,
        message: String,
    }
}
