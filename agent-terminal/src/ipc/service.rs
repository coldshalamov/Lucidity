//! Product-owned Windows named-pipe host service.
//!
//! The service owns the accept loop while the shared [`HostController`]
//! remains available to native UI and tray bindings. Every connection is
//! authenticated against the pipe owner's user SID before protocol bytes are
//! accepted.

use crate::host::HostController;
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use std::sync::Arc;

#[cfg(windows)]
const SUBSCRIBED_READ_WAIT_MILLIS: u32 = 25;

#[cfg(windows)]
const READ_CHUNK_BYTES: usize = 16 * 1024;

pub type SharedHostController = Arc<Mutex<HostController>>;

pub struct HostService {
    controller: SharedHostController,
    pipe_name: String,
    #[cfg(windows)]
    stop: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(windows)]
    thread: Option<std::thread::JoinHandle<Result<()>>>,
}

impl HostService {
    /// Starts the production host on [`super::PIPE_NAME`].
    pub fn spawn(controller: HostController) -> Result<Self> {
        Self::spawn_shared(Arc::new(Mutex::new(controller)))
    }

    /// Starts the service around authority already shared with product UI.
    #[cfg(windows)]
    pub fn spawn_shared(controller: SharedHostController) -> Result<Self> {
        use super::named_pipe::{current_user_sid_string, NamedPipeServer};

        let owner_sid = current_user_sid_string()?;
        let pipe_name = super::PIPE_NAME.to_owned();
        let initial = NamedPipeServer::create_named_for_owner_sid(pipe_name.clone(), &owner_sid)?;
        Self::spawn_with_initial(controller, owner_sid, pipe_name, initial)
    }

    #[cfg(not(windows))]
    pub fn spawn_shared(_controller: SharedHostController) -> Result<Self> {
        bail!("Lucidity host IPC requires Windows named pipes")
    }

    /// Returns the exact local path clients use to connect.
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    /// Shares host authority without creating another controller or catalog.
    pub fn controller(&self) -> SharedHostController {
        Arc::clone(&self.controller)
    }

    /// Signals an idle accept or connected read and joins the service thread.
    #[cfg(windows)]
    pub fn stop(&mut self) -> Result<()> {
        use std::sync::atomic::Ordering;

        self.stop.store(true, Ordering::Release);
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        match thread.join() {
            Ok(result) => result,
            Err(payload) => Err(anyhow!("host IPC thread panicked: {}", panic_text(payload))),
        }
    }

    #[cfg(not(windows))]
    pub fn stop(&mut self) -> Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn spawn_with_initial(
        controller: SharedHostController,
        owner_sid: String,
        pipe_name: String,
        initial: super::named_pipe::NamedPipeServer,
    ) -> Result<Self> {
        use std::sync::atomic::AtomicBool;

        let stop = Arc::new(AtomicBool::new(false));
        let thread_controller = Arc::clone(&controller);
        let thread_stop = Arc::clone(&stop);
        let thread_pipe_name = pipe_name.clone();
        let thread = std::thread::Builder::new()
            .name("lucidity-host-ipc".to_owned())
            .spawn(move || {
                run_accept_loop(
                    initial,
                    &owner_sid,
                    &thread_pipe_name,
                    &thread_controller,
                    &thread_stop,
                )
            })?;

        Ok(Self {
            controller,
            pipe_name,
            stop,
            thread: Some(thread),
        })
    }
}

#[cfg(windows)]
impl Drop for HostService {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            log::error!("failed to stop Lucidity host IPC service: {error:#}");
        }
    }
}

#[cfg(not(windows))]
impl Drop for HostService {
    fn drop(&mut self) {}
}

#[cfg(windows)]
fn run_accept_loop(
    initial: super::named_pipe::NamedPipeServer,
    owner_sid: &str,
    pipe_name: &str,
    controller: &SharedHostController,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use super::named_pipe::{ConnectOutcome, NamedPipeServer};
    use std::sync::atomic::Ordering;

    let mut next_server = Some(initial);
    while !stop.load(Ordering::Acquire) {
        let server = next_server
            .take()
            .expect("accept loop always carries its next pipe instance");
        match server.connect_interruptible(stop) {
            Ok(ConnectOutcome::Stopped) => return Ok(()),
            Ok(ConnectOutcome::Connected) => {}
            Err(_) if stop.load(Ordering::Acquire) => return Ok(()),
            Err(error) => {
                log::warn!("Lucidity named-pipe accept failed: {error}");
                drop(server);
                next_server = Some(NamedPipeServer::create_named_for_owner_sid(
                    pipe_name, owner_sid,
                )?);
                continue;
            }
        }

        match server.verify_connected_client_same_user(owner_sid) {
            Ok(pid) => log::debug!("Lucidity IPC client connected: pid={pid}"),
            Err(error) => {
                log::warn!("rejecting Lucidity IPC client: {error:#}");
                server.disconnect();
                drop(server);
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                next_server = Some(NamedPipeServer::create_named_for_owner_sid(
                    pipe_name, owner_sid,
                )?);
                continue;
            }
        }

        if let Err(error) = serve_connection(&server, controller, stop) {
            if !stop.load(Ordering::Acquire) {
                log::warn!("Lucidity IPC client disconnected after error: {error:#}");
            }
        }
        server.disconnect();
        drop(server);

        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        next_server = Some(NamedPipeServer::create_named_for_owner_sid(
            pipe_name, owner_sid,
        )?);
    }
    Ok(())
}

#[cfg(windows)]
fn serve_connection(
    server: &super::named_pipe::NamedPipeServer,
    controller: &SharedHostController,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    use super::named_pipe::ReadChunkOutcome;
    use super::{write_frame, write_ipc_response, FrameError};
    use agent_protocol::HostRequest;
    use std::sync::atomic::Ordering;

    let mut stream = server.connected(stop);
    let mut reader = BufferedRequestReader::new();
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    let mut subscribed = false;
    while !stop.load(Ordering::Acquire) {
        if let Some(request) = reader.next_request()? {
            let quit_requested = matches!(&request.request, HostRequest::HostQuit);
            let subscribe_requested = matches!(&request.request, HostRequest::EventSubscribe);
            if quit_requested {
                // Keep host authority locked until the acceptance frame has
                // reached the pipe. Otherwise the product action pump can
                // observe `Quitting` and tear down this service mid-response.
                {
                    let mut host = controller.lock();
                    let response = host.handle_ipc_request(request);
                    write_ipc_response(&mut stream, &response)?;
                    // A named-pipe disconnect discards unread buffered bytes.
                    // Drain the accepted response before publishing the stop
                    // flag that allows the product and accept loop to exit.
                    std::io::Write::flush(&mut stream)?;
                }
                stop.store(true, Ordering::Release);
                return Ok(());
            }
            let (response, events) =
                dispatch_request(controller, request, subscribed || subscribe_requested);

            // The response for the request that established the subscription
            // is always written before any already-queued event frames.
            write_ipc_response(&mut stream, &response)?;
            if subscribe_requested {
                subscribed = true;
            }
            for event in events {
                write_frame(&mut stream, &event)?;
            }
            continue;
        }

        if subscribed {
            write_queued_events(&mut stream, controller)?;
        }

        match server.read_chunk_interruptible_for(&mut chunk, stop, SUBSCRIBED_READ_WAIT_MILLIS) {
            Ok(ReadChunkOutcome::Bytes(0)) => return Ok(()),
            Ok(ReadChunkOutcome::Bytes(count)) => reader.push(&chunk[..count]),
            Ok(ReadChunkOutcome::TimedOut) => {}
            Ok(ReadChunkOutcome::Stopped) => return Ok(()),
            Err(error) if is_peer_disconnect(&error) => return Ok(()),
            Err(error) => return Err(FrameError::Io(error).into()),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn write_queued_events(
    stream: &mut impl std::io::Write,
    controller: &SharedHostController,
) -> Result<()> {
    use super::write_frame;

    let events = drain_events(controller);
    for event in events {
        write_frame(stream, &event)?;
    }
    Ok(())
}

#[cfg(windows)]
fn drain_events(controller: &SharedHostController) -> Vec<agent_protocol::HostEvent> {
    let mut host = controller.lock();
    let mut events = Vec::new();
    while let Some(event) = host.pop_event() {
        events.push(event);
    }
    events
}

#[cfg(windows)]
fn dispatch_request(
    controller: &SharedHostController,
    request: agent_protocol::IpcRequest,
    drain_events: bool,
) -> (agent_protocol::IpcResponse, Vec<agent_protocol::HostEvent>) {
    let mut host = controller.lock();
    let response = host.handle_ipc_request(request);
    let events = if drain_events {
        let mut events = Vec::new();
        while let Some(event) = host.pop_event() {
            events.push(event);
        }
        events
    } else {
        Vec::new()
    };
    (response, events)
}

#[cfg(windows)]
struct BufferedRequestReader {
    bytes: Vec<u8>,
}

#[cfg(windows)]
impl BufferedRequestReader {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn push(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn next_request(&mut self) -> Result<Option<agent_protocol::IpcRequest>, super::FrameError> {
        use agent_protocol::MAX_FRAME_BYTES;

        if self.bytes.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_le_bytes(self.bytes[..4].try_into().expect("four-byte frame length"))
            as usize;
        if len == 0 {
            return Err(super::FrameError::ZeroLength);
        }
        if len > MAX_FRAME_BYTES {
            return Err(super::FrameError::Oversized {
                len,
                max: MAX_FRAME_BYTES,
            });
        }
        let frame_len = 4 + len;
        if self.bytes.len() < frame_len {
            return Ok(None);
        }

        let frame: Vec<u8> = self.bytes.drain(..frame_len).collect();
        let mut frame = frame.as_slice();
        super::read_ipc_request(&mut frame).map(Some)
    }
}

#[cfg(windows)]
fn is_peer_disconnect(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::UnexpectedEof
    )
}

#[cfg(windows)]
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::catalog::CatalogStore;
    use agent_protocol::{
        ConversationId, HostEvent, HostInstanceId, HostRequest, HostResult, IpcRequest,
        IpcResponse, IpcResponsePayload,
    };
    use std::fs::{File, OpenOptions};
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    fn open_pipe_retry(path: &str) -> File {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(file) => return file,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to connect test pipe {path}: {error}"),
            }
        }
    }

    fn request_status(pipe: &mut File, id: u64) -> IpcResponse {
        super::super::write_frame(
            pipe,
            &IpcRequest {
                id,
                request: HostRequest::HostStatus,
            },
        )
        .unwrap();
        super::super::read_frame(pipe).unwrap()
    }

    #[test]
    fn service_handles_framed_requests_reconnects_and_stops_while_idle() {
        use super::super::named_pipe::{current_user_sid_string, NamedPipeServer};

        let owner_sid = current_user_sid_string().unwrap();
        let pipe_name = format!(r"\\.\pipe\lucidity-host-test-{}", Uuid::new_v4());
        let initial = NamedPipeServer::create_named_for_owner_sid(&pipe_name, &owner_sid).unwrap();
        let host_id = HostInstanceId(Uuid::new_v4());
        let controller = HostController::new(CatalogStore::in_memory().unwrap(), host_id).unwrap();
        let shared = Arc::new(Mutex::new(controller));
        let mut service = HostService::spawn_with_initial(
            Arc::clone(&shared),
            owner_sid,
            pipe_name.clone(),
            initial,
        )
        .unwrap();

        let mut first = open_pipe_retry(&pipe_name);
        let response = request_status(&mut first, 7);
        assert_eq!(response.id, 7);
        assert!(matches!(
            response.payload,
            IpcResponsePayload::Ok(HostResult::Status {
                host_instance_id,
                shutting_down: false
            }) if host_instance_id == host_id
        ));
        drop(first);

        let mut second = open_pipe_retry(&pipe_name);
        let response = request_status(&mut second, 8);
        assert_eq!(response.id, 8);

        let started = Instant::now();
        service.stop().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(second);
    }

    #[test]
    fn subscribed_client_receives_ordered_events_without_another_request() {
        use super::super::named_pipe::{current_user_sid_string, NamedPipeServer};

        let owner_sid = current_user_sid_string().unwrap();
        let pipe_name = format!(r"\\.\pipe\lucidity-host-test-{}", Uuid::new_v4());
        let initial = NamedPipeServer::create_named_for_owner_sid(&pipe_name, &owner_sid).unwrap();
        let host_id = HostInstanceId(Uuid::new_v4());
        let controller = HostController::new(CatalogStore::in_memory().unwrap(), host_id).unwrap();
        let shared = Arc::new(Mutex::new(controller));
        let mut service = HostService::spawn_with_initial(
            Arc::clone(&shared),
            owner_sid,
            pipe_name.clone(),
            initial,
        )
        .unwrap();

        let mut client = open_pipe_retry(&pipe_name);
        super::super::write_frame(
            &mut client,
            &IpcRequest {
                id: 11,
                request: HostRequest::EventSubscribe,
            },
        )
        .unwrap();
        let response: IpcResponse = super::super::read_frame(&mut client).unwrap();
        assert_eq!(response.id, 11);
        assert_eq!(
            response.payload,
            IpcResponsePayload::Ok(HostResult::Accepted)
        );

        let conversation_id = ConversationId(Uuid::new_v4());
        let expected = vec![
            HostEvent::CatalogChanged {
                catalog_snapshot_version: 42,
            },
            HostEvent::ConversationChanged { conversation_id },
        ];
        {
            let mut host = shared.lock();
            for event in &expected {
                host.push_event(event.clone());
            }
        }

        // No request follows event.subscribe. A bounded reader proves the host
        // initiates both writes and preserves the controller queue order.
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            let first = super::super::read_frame::<HostEvent>(&mut client);
            let second = super::super::read_frame::<HostEvent>(&mut client);
            let _ = sender.send((first, second));
        });
        let received = receiver.recv_timeout(Duration::from_secs(2));

        // Stop before asserting so a failed delivery also releases the reader.
        service.stop().unwrap();
        reader.join().unwrap();
        let (first, second) = received.expect("subscribed event delivery timed out");
        assert_eq!(vec![first.unwrap(), second.unwrap()], expected);
    }
}
