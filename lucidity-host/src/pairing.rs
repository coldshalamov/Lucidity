use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use qrcodegen::{QrCode, QrCodeEcc};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PAIRING_TTL: Duration = Duration::from_secs(60);
const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingInfo {
    pub protocol_version: u32,
    pub host_id: String,
    pub pairing_token: String,
    pub pairing_code: String,
    pub expires_at_unix_ms: u64,
    pub host_lan_candidates: Vec<String>,
    pub cloud_rendezvous_url: String,
}

#[derive(Debug, Clone)]
struct PairingState {
    token_bytes: [u8; 16],
    expires_at_unix_ms: u64,
}

static HOST_ID: OnceLock<Uuid> = OnceLock::new();
static PAIRING_STATE: OnceLock<Mutex<Option<PairingState>>> = OnceLock::new();

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn host_id() -> Uuid {
    *HOST_ID.get_or_init(Uuid::new_v4)
}

fn pairing_mutex() -> &'static Mutex<Option<PairingState>> {
    PAIRING_STATE.get_or_init(|| Mutex::new(None))
}

fn current_listen_addr() -> SocketAddr {
    std::env::var("LUCIDITY_LISTEN")
        .ok()
        .and_then(|s| s.parse::<SocketAddr>().ok())
        .unwrap_or_else(|| crate::server::HostConfig::default().listen)
}

fn best_effort_local_ip() -> Option<IpAddr> {
    // Common trick: connect UDP socket and see which local address the OS picks.
    // We don’t send data; connect() is enough to select a route.
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    Some(sock.local_addr().ok()?.ip())
}

fn host_lan_candidates() -> Vec<String> {
    let listen = current_listen_addr();
    let port = listen.port();

    match listen.ip() {
        IpAddr::V4(ipv4) if ipv4.is_loopback() || ipv4.octets() == [0, 0, 0, 0] => {
            if let Some(ip) = best_effort_local_ip() {
                vec![format!("{ip}:{port}")]
            } else {
                vec![]
            }
        }
        other => vec![format!("{other}:{port}")],
    }
}

fn token_to_code(token_bytes: &[u8; 16]) -> String {
    // Local-only Phase 3: derive a stable 6-digit code from the token.
    // This is not intended to be cryptographically meaningful yet.
    let mut n: u64 = 0;
    for b in token_bytes.iter() {
        n = n.wrapping_mul(257).wrapping_add(*b as u64);
    }
    format!("LUC-{code:06}", code = (n % 1_000_000))
}

fn token_to_string(token_bytes: &[u8; 16]) -> String {
    URL_SAFE_NO_PAD.encode(token_bytes)
}

fn ensure_pairing_state() -> PairingState {
    let now = now_unix_ms();
    let mut guard = pairing_mutex().lock().unwrap();
    if let Some(state) = guard.as_ref() {
        if now < state.expires_at_unix_ms {
            return state.clone();
        }
    }

    let mut token = [0u8; 16];
    for b in token.iter_mut() {
        *b = fastrand::u8(..);
    }
    let expires_at_unix_ms = now.saturating_add(PAIRING_TTL.as_millis() as u64);
    let state = PairingState {
        token_bytes: token,
        expires_at_unix_ms,
    };
    *guard = Some(state.clone());
    state
}

pub fn pairing_info() -> PairingInfo {
    let state = ensure_pairing_state();
    PairingInfo {
        protocol_version: PROTOCOL_VERSION,
        host_id: host_id().to_string(),
        pairing_token: token_to_string(&state.token_bytes),
        pairing_code: token_to_code(&state.token_bytes),
        expires_at_unix_ms: state.expires_at_unix_ms,
        host_lan_candidates: host_lan_candidates(),
        cloud_rendezvous_url: "https://example.invalid/lucidity/rendezvous".to_string(),
    }
}

pub fn pairing_claim_by_code(code: &str) -> Option<PairingInfo> {
    let info = pairing_info();
    if info.pairing_code.eq_ignore_ascii_case(code.trim()) {
        Some(info)
    } else {
        None
    }
}

pub fn pairing_rotate() {
    let mut guard = pairing_mutex().lock().unwrap();
    *guard = None;
}

pub fn pairing_display_text() -> String {
    let info = pairing_info();
    let listen = current_listen_addr();

    let qr_payload = serde_json::json!({
        "protocol_version": info.protocol_version,
        "host_id": info.host_id,
        "pairing_token": info.pairing_token,
        "host_lan_candidates": info.host_lan_candidates,
        "cloud_rendezvous_url": info.cloud_rendezvous_url,
    })
    .to_string();

    let qr_ascii = render_qr_ascii(&qr_payload);

    let mut s = String::new();
    s.push_str("Lucidity\r\n\r\n");
    s.push_str("Connect Lucidity Mobile\r\n\r\n");
    s.push_str(&qr_ascii);
    s.push_str("\r\n");
    s.push_str(&format!("Pairing code: {}\r\n", info.pairing_code));
    s.push_str(&format!(
        "Expires: {} ms since epoch\r\n",
        info.expires_at_unix_ms
    ));
    s.push_str(&format!("Host listen: {listen}\r\n"));
    s.push_str("Scan in the mobile app or enter code.\r\n");
    s.push_str("Press Enter to continue locally.  (R = refresh)\r\n");
    s.push_str("\r\n");
    s
}

fn render_qr_ascii(data: &str) -> String {
    let qr = QrCode::encode_text(data, QrCodeEcc::Medium).unwrap();
    let size = qr.size();
    let quiet = 2;

    let mut out = String::new();
    for y in (-quiet)..(size + quiet) {
        for x in (-quiet)..(size + quiet) {
            let dark = if x >= 0 && y >= 0 && x < size && y < size {
                qr.get_module(x, y)
            } else {
                false
            };
            out.push_str(if dark { "██" } else { "  " });
        }
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_code_is_stable_for_token() {
        let token = [1u8; 16];
        let a = token_to_code(&token);
        let b = token_to_code(&token);
        assert_eq!(a, b);
        assert!(a.starts_with("LUC-"));
    }

    #[test]
    fn pairing_info_has_reasonable_fields() {
        let info = pairing_info();
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);
        assert!(info.host_id.len() >= 8);
        assert!(info.pairing_token.len() >= 10);
        assert!(info.pairing_code.starts_with("LUC-"));
        assert!(info.expires_at_unix_ms > 0);
    }

    #[test]
    fn claim_by_code_roundtrips() {
        pairing_rotate();
        let info = pairing_info();
        let claimed = pairing_claim_by_code(&info.pairing_code).unwrap();
        assert_eq!(claimed.pairing_token, info.pairing_token);
    }
}
