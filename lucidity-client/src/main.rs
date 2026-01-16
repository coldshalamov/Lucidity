use anyhow::{anyhow, Context};
use clap::Parser;
use lucidity_host::{TYPE_JSON, TYPE_PANE_INPUT, TYPE_PANE_OUTPUT};
use lucidity_proto::frame::{encode_frame, Frame, FrameDecoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Parser)]
#[command(about = "Lucidity Phase 1 test client (connects to lucidity-host)")]
struct Opts {
    #[arg(long, default_value = "127.0.0.1:9797")]
    addr: SocketAddr,

    #[arg(long)]
    pane_id: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonRequest {
    ListPanes,
    Attach { pane_id: usize },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonResponse {
    ListPanes { panes: Vec<lucidity_host::PaneInfo> },
    AttachOk { pane_id: usize },
    Error { message: String },
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

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    let mut stream = TcpStream::connect(opts.addr).context("connect")?;
    stream.set_nodelay(true).ok();

    let mut dec = FrameDecoder::new();

    let pane_id = if let Some(p) = opts.pane_id {
        p
    } else {
        send_json(&mut stream, &JsonRequest::ListPanes)?;
        let frame = read_one_frame(&mut stream, &mut dec)?;
        if frame.typ != TYPE_JSON {
            return Err(anyhow!("expected json response, got type {}", frame.typ));
        }
        let resp: JsonResponse = serde_json::from_slice(&frame.payload)?;
        match resp {
            JsonResponse::ListPanes { panes } => {
                eprintln!("Panes:");
                for p in &panes {
                    eprintln!("  {}  {}", p.pane_id, p.title);
                }
                panes
                    .first()
                    .map(|p| p.pane_id)
                    .ok_or_else(|| anyhow!("no panes found"))?
            }
            JsonResponse::Error { message } => return Err(anyhow!("server error: {message}")),
            other => return Err(anyhow!("unexpected response: {other:?}")),
        }
    };

    send_json(&mut stream, &JsonRequest::Attach { pane_id })?;
    let frame = read_one_frame(&mut stream, &mut dec)?;
    if frame.typ != TYPE_JSON {
        return Err(anyhow!("expected json response, got type {}", frame.typ));
    }
    match serde_json::from_slice::<JsonResponse>(&frame.payload)? {
        JsonResponse::AttachOk { pane_id: p } => eprintln!("Attached to pane {p}"),
        JsonResponse::Error { message } => return Err(anyhow!("server error: {message}")),
        other => return Err(anyhow!("unexpected response: {other:?}")),
    }

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
    let mut dec = FrameDecoder::new();
    loop {
        let frame = read_one_frame(&mut reader, &mut dec)?;
        match frame.typ {
            TYPE_PANE_OUTPUT => {
                out.write_all(&frame.payload)?;
                out.flush().ok();
            }
            TYPE_JSON => {
                if let Ok(resp) = serde_json::from_slice::<JsonResponse>(&frame.payload) {
                    eprintln!("server: {resp:?}");
                }
            }
            _ => {}
        }
    }
}

