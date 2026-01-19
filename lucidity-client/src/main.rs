use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use lucidity_host::{TYPE_JSON, TYPE_PANE_INPUT, TYPE_PANE_OUTPUT};
use lucidity_pairing::{Keypair, PairingPayload, PairingRequest, PairingResponse};
use lucidity_proto::frame::{encode_frame, Frame, FrameDecoder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Parser)]
#[command(about = "Lucidity test client")]
struct Opts {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Pair with a host using a QR payload or URL
    Pair {
        /// Pairing data (e.g. "lucidity://pair?data=...")
        pairing_uri: String,

        /// Where to save the generated identity
        #[arg(long, default_value = "client_identity.json")]
        identity: PathBuf,
    },
    /// Connect to a host using a saved identity
    Connect {
        /// Path to saved identity file
        #[arg(long, default_value = "client_identity.json")]
        identity: PathBuf,

        /// Pane ID to attach to (optional, defaults to listing/picking first)
        #[arg(long)]
        pane_id: Option<usize>,

        /// Manual override for host address (defaults to what's in pairing data)
        #[arg(long)]
        addr: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ClientIdentity {
    mobile_keypair: String, // Base64 encoded keypair
    desktop_public_key: String, // Base64 encoded public key
    relay_id: String,
    lan_addr: Option<String>,
    external_addr: Option<String>,
    paired_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonRequest {
    ListPanes,
    Attach {
        pane_id: usize,
    },
    PairingSubmit {
        request: PairingRequest,
    },
    AuthResponse {
        public_key: String,
        signature: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonResponse {
    ListPanes {
        panes: Vec<lucidity_host::PaneInfo>,
    },
    AttachOk {
        pane_id: usize,
    },
    PairingResponse {
        response: PairingResponse,
    },
    AuthChallenge {
        nonce: String,
    },
    AuthSuccess,
    Error {
        message: String,
    },
}

fn read_one_frame(stream: &mut TcpStream, dec: &mut FrameDecoder) -> anyhow::Result<Frame> {
    let mut buf = [0u8; 64 * 1024];
    loop {
        if let Some(frame) = dec.next_frame()? {
            return Ok(frame);
        }
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Err(anyhow!("server closed connection"));
        }
        dec.push(&buf[..n]);
    }
}

fn send_json(stream: &mut dyn Write, req: &JsonRequest) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(req)?;
    stream.write_all(&encode_frame(TYPE_JSON, &payload))?;
    stream.flush().ok();
    Ok(())
}

fn expect_json_response(
    stream: &mut TcpStream,
    dec: &mut FrameDecoder,
) -> anyhow::Result<JsonResponse> {
    loop {
        let frame = read_one_frame(stream, dec)?;
        if frame.typ == TYPE_JSON {
            return Ok(serde_json::from_slice(&frame.payload)?);
        }
        // Ignore other frames during handshake (e.g. unsolicited output)
    }
}

use base64::Engine;

fn to_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn from_base64_32(s: &str) -> anyhow::Result<[u8; 32]> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .context("invalid base64")?;
    if bytes.len() != 32 {
        anyhow::bail!("expected 32 bytes, got {}", bytes.len());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn perform_pair(uri: String, identity_path: PathBuf) -> anyhow::Result<()> {
    let payload = lucidity_pairing::parse_pairing_url(&uri).context("failed to parse pairing URI")?;

    println!("Detected Host: {}", payload.relay_id);
    if let Some(lan) = &payload.lan_addr {
        println!("  LAN: {}", lan);
    }

    let addr = payload
        .lan_addr
        .clone()
        .ok_or_else(|| anyhow!("No LAN address in pairing payload - cannot connect securely"))?;

    println!("Connecting to {}...", addr);
    let mut stream = TcpStream::connect(&addr).context("connect")?;
    let mut dec = FrameDecoder::new();

    let mobile_keypair = Keypair::generate();
    let request = PairingRequest::new(
        &mobile_keypair,
        &payload.desktop_public_key,
        "mock-client@localhost".to_string(),
        "Mock Client (Rust)".to_string(),
    );

    println!("Submitting pairing request...");
    send_json(
        &mut stream,
        &JsonRequest::PairingSubmit {
            request: request.clone(),
        },
    )?;

    match expect_json_response(&mut stream, &mut dec)? {
        JsonResponse::PairingResponse { response } => {
            if response.approved {
                println!("✅ Pairing APPROVED!");
                let identity = ClientIdentity {
                    mobile_keypair: to_base64(&mobile_keypair.to_bytes()),
                    desktop_public_key: payload.desktop_public_key.to_base64(),
                    relay_id: payload.relay_id,
                    lan_addr: payload.lan_addr,
                    external_addr: payload.external_addr,
                    paired_at: chrono::Utc::now().timestamp(),
                };
                let json = serde_json::to_string_pretty(&identity)?;
                fs::write(&identity_path, json)?;
                println!("Saved identity to {:?}", identity_path);
            } else {
                return Err(anyhow!(
                    "Pairing REJECTED: {}",
                    response.reason.unwrap_or_default()
                ));
            }
        }
        JsonResponse::Error { message } => return Err(anyhow!("Server error: {}", message)),
        other => return Err(anyhow!("Unexpected response: {:?}", other)),
    }

    Ok(())
}

fn perform_connect(
    identity_path: PathBuf,
    pane_id: Option<usize>,
    addr_override: Option<String>,
) -> anyhow::Result<()> {
    let json = fs::read_to_string(&identity_path)
        .with_context(|| format!("reading {:?}", identity_path))?;
    let id: ClientIdentity = serde_json::from_str(&json)?;
    
    let key_bytes = from_base64_32(&id.mobile_keypair)?;
    let keypair = Keypair::from_bytes(&key_bytes);

    let addr = addr_override
        .or(id.lan_addr)
        .ok_or_else(|| anyhow!("No LAN address known"))?;

    println!("Connecting to {}...", addr);
    let mut stream = TcpStream::connect(addr)?;
    let mut dec = FrameDecoder::new();

    // 1. Wait for Auth Challenge (or success if localhost shortcut is active, but we shouldn't rely on it)
    let challenge = match expect_json_response(&mut stream, &mut dec)? {
        JsonResponse::AuthChallenge { nonce } => nonce,
        JsonResponse::Error { message } => return Err(anyhow!("Connect error: {}", message)),
        other => return Err(anyhow!("Expected AuthChallenge, got {:?}", other)),
    };

    // 2. Respond
    let signature = keypair.sign(challenge.as_bytes());
    send_json(
        &mut stream,
        &JsonRequest::AuthResponse {
            public_key: keypair.public_key().to_base64(),
            signature: signature.to_base64(),
        },
    )?;

    // 3. Wait for success
    match expect_json_response(&mut stream, &mut dec)? {
        JsonResponse::AuthSuccess => println!("✅ Authenticated"),
        JsonResponse::Error { message } => return Err(anyhow!("Auth failed: {}", message)),
        other => return Err(anyhow!("Expected AuthSuccess, got {:?}", other)),
    }

    // 4. List/Attach
    let pane_id = if let Some(p) = pane_id {
        p
    } else {
        send_json(&mut stream, &JsonRequest::ListPanes)?;
        let resp = expect_json_response(&mut stream, &mut dec)?;
        if let JsonResponse::ListPanes { panes } = resp {
            eprintln!("Panes:");
            for p in &panes {
                eprintln!("  {}  {}", p.pane_id, p.title);
            }
            panes
                .first()
                .map(|p| p.pane_id)
                .ok_or_else(|| anyhow!("no panes found"))?
        } else {
            return Err(anyhow!("Expected ListPanes response"));
        }
    };

    send_json(&mut stream, &JsonRequest::Attach { pane_id })?;
    match expect_json_response(&mut stream, &mut dec)? {
        JsonResponse::AttachOk { pane_id: p } => eprintln!("Attached to pane {p}"),
        JsonResponse::Error { message } => return Err(anyhow!("Attach error: {message}")),
        other => return Err(anyhow!("Unexpected response: {other:?}")),
    }

    // 5. Pipe I/O
    let read_stream = stream.try_clone()?;
    let write_stream = Arc::new(Mutex::new(stream));

    thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 8192];
        loop {
            let n = match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let frame = encode_frame(TYPE_PANE_INPUT, &buf[..n]);
            let mut w = write_stream.lock().unwrap();
            if w.write_all(&frame).is_err() {
                break;
            }
            w.flush().ok();
        }
    });

    let mut out = std::io::stdout();
    let mut reader = read_stream;
    loop {
        let frame = read_one_frame(&mut reader, &mut dec)?;
        match frame.typ {
            TYPE_PANE_OUTPUT => {
                out.write_all(&frame.payload)?;
                out.flush().ok();
            }
            TYPE_JSON => {
                if let Ok(resp) = serde_json::from_slice::<JsonResponse>(&frame.payload) {
                    eprintln!("server info: {resp:?}");
                }
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    match opts.cmd {
        Command::Pair {
            pairing_uri,
            identity,
        } => perform_pair(pairing_uri, identity),
        Command::Connect {
            identity,
            pane_id,
            addr,
        } => perform_connect(identity, pane_id, addr),
    }
}
