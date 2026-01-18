use crate::bridge::{PaneBridge, PaneInfo};
use crate::pairing_api::{current_pairing_payload, handle_pairing_submit, list_trusted_devices};
use crate::protocol::{TYPE_JSON, TYPE_PANE_INPUT, TYPE_PANE_OUTPUT};
use anyhow::{anyhow, Context};
use lucidity_proto::frame::{encode_frame, FrameDecoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

fn max_clients() -> usize {
    std::env::var("LUCIDITY_MAX_CLIENTS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(4)
}

struct ActiveClientGuard {
    counter: Arc<AtomicUsize>,
}

impl ActiveClientGuard {
    fn try_new(counter: Arc<AtomicUsize>, max: usize) -> Option<Self> {
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return None;
            }
            if counter
                .compare_exchange(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(Self { counter });
            }
        }
    }
}

impl Drop for ActiveClientGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub listen: SocketAddr,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:9797".parse().unwrap(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonRequest {
    ListPanes,
    Attach {
        pane_id: usize,
    },
    PairingPayload,
    PairingSubmit {
        request: lucidity_pairing::PairingRequest,
    },
    PairingListTrustedDevices,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum JsonResponse {
    ListPanes {
        panes: Vec<PaneInfo>,
    },
    AttachOk {
        pane_id: usize,
    },
    PairingPayload {
        payload: lucidity_pairing::PairingPayload,
    },
    PairingResponse {
        response: lucidity_pairing::PairingResponse,
    },
    PairingTrustedDevices {
        devices: Vec<lucidity_pairing::TrustedDevice>,
    },
    Error {
        message: String,
    },
}

fn write_json_frame(writer: &mut dyn Write, msg: &JsonResponse) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(msg)?;
    let frame = encode_frame(TYPE_JSON, &payload);
    writer.write_all(&frame)?;
    writer.flush().ok();
    Ok(())
}

fn handle_client(stream: TcpStream, bridge: Arc<dyn PaneBridge>) -> anyhow::Result<()> {
    stream.set_nodelay(true).ok();
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();

    let mut reader = stream.try_clone()?;
    let writer = Arc::new(Mutex::new(stream));

    let attached = Arc::new(Mutex::new(None::<usize>));
    let output_thread_dead = Arc::new(AtomicBool::new(false));

    let mut decoder = FrameDecoder::new();
    let mut buf = [0u8; 64 * 1024];

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => return Err(err).context("reading from client"),
        };

        decoder.push(&buf[..n]);
        while let Some(frame) = decoder.next_frame()? {
            match frame.typ {
                TYPE_JSON => {
                    let req: JsonRequest = match serde_json::from_slice(&frame.payload) {
                        Ok(r) => r,
                        Err(err) => {
                            let mut w = writer.lock().unwrap();
                            write_json_frame(
                                &mut *w,
                                &JsonResponse::Error {
                                    message: format!("invalid json request: {err}"),
                                },
                            )?;
                            continue;
                        }
                    };

                    match req {
                        JsonRequest::ListPanes => {
                            let panes = bridge.list_panes()?;
                            let mut w = writer.lock().unwrap();
                            write_json_frame(&mut *w, &JsonResponse::ListPanes { panes })?;
                        }
                        JsonRequest::Attach { pane_id } => {
                            {
                                let mut a = attached.lock().unwrap();
                                if a.is_some() {
                                    let mut w = writer.lock().unwrap();
                                    write_json_frame(
                                        &mut *w,
                                        &JsonResponse::Error {
                                            message: "already attached".to_string(),
                                        },
                                    )?;
                                    continue;
                                }
                                *a = Some(pane_id);
                            }

                            let sub = bridge.subscribe_output(pane_id)?;
                            let writer2 = Arc::clone(&writer);
                            let dead2 = Arc::clone(&output_thread_dead);
                            thread::spawn(move || {
                                while !dead2.load(Ordering::Relaxed) {
                                    let bytes = match sub.recv_timeout(Duration::from_millis(250)) {
                                        Ok(Some(b)) => b,
                                        Ok(None) => continue,
                                        Err(_) => break,
                                    };
                                    let frame = encode_frame(TYPE_PANE_OUTPUT, &bytes);
                                    let mut w = writer2.lock().unwrap();
                                    if w.write_all(&frame).is_err() {
                                        break;
                                    }
                                    w.flush().ok();
                                }
                            });

                            let mut w = writer.lock().unwrap();
                            write_json_frame(&mut *w, &JsonResponse::AttachOk { pane_id })?;
                        }
                        JsonRequest::PairingPayload => {
                            let payload = current_pairing_payload()?;
                            let mut w = writer.lock().unwrap();
                            write_json_frame(&mut *w, &JsonResponse::PairingPayload { payload })?;
                        }
                        JsonRequest::PairingSubmit { request } => {
                            let response = handle_pairing_submit(request)?;
                            let mut w = writer.lock().unwrap();
                            write_json_frame(&mut *w, &JsonResponse::PairingResponse { response })?;
                        }
                        JsonRequest::PairingListTrustedDevices => {
                            let devices = list_trusted_devices()?;
                            let mut w = writer.lock().unwrap();
                            write_json_frame(
                                &mut *w,
                                &JsonResponse::PairingTrustedDevices { devices },
                            )?;
                        }
                    }
                }
                TYPE_PANE_INPUT => {
                    let pane_id = attached
                        .lock()
                        .unwrap()
                        .ok_or_else(|| anyhow!("received input before attach"))?;
                    bridge.send_input(pane_id, &frame.payload)?;
                }
                other => {
                    let mut w = writer.lock().unwrap();
                    write_json_frame(
                        &mut *w,
                        &JsonResponse::Error {
                            message: format!("unsupported frame type: {other}"),
                        },
                    )?;
                }
            }
        }
    }

    output_thread_dead.store(true, Ordering::Relaxed);
    Ok(())
}

pub fn serve_blocking(listener: TcpListener, bridge: Arc<dyn PaneBridge>) -> anyhow::Result<()> {
    serve_blocking_with_limit(listener, bridge, max_clients())
}

pub fn serve_blocking_with_limit(
    listener: TcpListener,
    bridge: Arc<dyn PaneBridge>,
    max_clients: usize,
) -> anyhow::Result<()> {
    let active_clients = Arc::new(AtomicUsize::new(0));

    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(s) => s,
            Err(err) => {
                log::warn!("lucidity-host accept failed: {err:#}");
                continue;
            }
        };

        let max = max_clients;
        let guard = match ActiveClientGuard::try_new(Arc::clone(&active_clients), max) {
            Some(g) => g,
            None => {
                let peer = stream
                    .peer_addr()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string());
                log::warn!("lucidity-host rejecting client {peer}: max clients ({max}) reached");
                let _ = write_json_frame(
                    &mut stream,
                    &JsonResponse::Error {
                        message: format!("server busy: max clients ({max}) reached"),
                    },
                );
                continue;
            }
        };

        let peer = stream
            .peer_addr()
            .map(|p| p.to_string())
            .unwrap_or_else(|_| "<unknown>".to_string());
        log::info!("lucidity-host client connected: {peer} (max {max})");

        let bridge = Arc::clone(&bridge);
        thread::spawn(move || {
            let _guard = guard;
            match handle_client(stream, bridge) {
                Ok(()) => {
                    log::info!("lucidity-host client disconnected: {peer}");
                }
                Err(err) => {
                    log::info!("lucidity-host client disconnected: {peer}: {err:#}");
                }
            }
        });
    }
    Ok(())
}

static AUTOSTARTED: OnceLock<()> = OnceLock::new();

pub fn autostart_in_process() {
    AUTOSTARTED.get_or_init(|| {
        if std::env::var("LUCIDITY_DISABLE_HOST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            return;
        }

        let listen = std::env::var("LUCIDITY_LISTEN")
            .ok()
            .and_then(|s| s.parse::<SocketAddr>().ok())
            .unwrap_or_else(|| HostConfig::default().listen);

        // SECURITY WARNING: Alert users when binding to all interfaces
        if listen.ip().is_unspecified() {
            log::warn!(
                "SECURITY WARNING: Lucidity host listening on {} - anyone on your LAN can inject keystrokes! \
                 Set LUCIDITY_LISTEN=127.0.0.1:9797 for localhost-only.",
                listen
            );
        }

        let listener = match TcpListener::bind(listen) {
            Ok(l) => l,
            Err(err) => {
                log::error!("lucidity-host failed to bind {listen}: {err:#}");
                return;
            }
        };

        thread::Builder::new()
            .name("lucidity-host".to_string())
            .spawn(move || {
                let bridge: Arc<dyn PaneBridge> = Arc::new(crate::bridge::MuxPaneBridge::default());
                if let Err(err) = serve_blocking(listener, bridge) {
                    log::error!("lucidity-host server stopped: {err:#}");
                }
            })
            .ok();
    });
}
