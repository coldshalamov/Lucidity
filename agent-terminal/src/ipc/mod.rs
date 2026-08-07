//! Local IPC framing, flow control, and Windows named-pipe security helpers.

use agent_protocol::{
    HostEvent, IpcRequest, IpcResponse, MAX_FRAME_BYTES, MAX_PENDING_EVENTS_PER_SUBSCRIBER,
};
use anyhow::{anyhow, bail, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, ErrorKind, Read, Write};

pub mod service;

/// Stable local endpoint shared with the VS Code extension.
pub const PIPE_NAME: &str = r"\\.\pipe\lucidity-control-v1";

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    ZeroLength,
    Oversized { len: usize, max: usize },
    Truncated { expected: usize, actual: usize },
    InvalidUtf8(std::string::FromUtf8Error),
    Json(serde_json::Error),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubscriberPush {
    Queued,
    ResyncRequired,
}

#[derive(Clone, Debug)]
pub struct HostEventSubscriber {
    queue: VecDeque<HostEvent>,
    resync_required: bool,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::ZeroLength => write!(formatter, "zero-length IPC frame"),
            Self::Oversized { len, max } => {
                write!(formatter, "oversized IPC frame: {len} bytes exceeds {max}")
            }
            Self::Truncated { expected, actual } => {
                write!(
                    formatter,
                    "truncated IPC frame: expected {expected}, read {actual}"
                )
            }
            Self::InvalidUtf8(error) => write!(formatter, "invalid UTF-8 IPC frame: {error}"),
            Self::Json(error) => write!(formatter, "invalid IPC JSON: {error}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl HostEventSubscriber {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(MAX_PENDING_EVENTS_PER_SUBSCRIBER),
            resync_required: false,
        }
    }

    pub fn push(&mut self, event: HostEvent) -> SubscriberPush {
        if self.resync_required || self.queue.len() >= MAX_PENDING_EVENTS_PER_SUBSCRIBER {
            self.resync_required = true;
            self.queue.clear();
            return SubscriberPush::ResyncRequired;
        }
        self.queue.push_back(event);
        SubscriberPush::Queued
    }

    pub fn pop(&mut self) -> Option<HostEvent> {
        self.queue.pop_front()
    }

    pub fn resync_required(&self) -> bool {
        self.resync_required
    }

    pub fn clear_after_snapshot(&mut self) {
        self.queue.clear();
        self.resync_required = false;
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

impl Default for HostEventSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

pub fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, FrameError> {
    let json = serde_json::to_vec(value).map_err(FrameError::Json)?;
    if json.is_empty() {
        return Err(FrameError::ZeroLength);
    }
    if json.len() > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized {
            len: json.len(),
            max: MAX_FRAME_BYTES,
        });
    }
    let mut frame = Vec::with_capacity(4 + json.len());
    frame.extend_from_slice(&(json.len() as u32).to_le_bytes());
    frame.extend_from_slice(&json);
    Ok(frame)
}

pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> Result<(), FrameError> {
    let frame = encode_frame(value)?;
    writer.write_all(&frame)?;
    Ok(())
}

pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, FrameError> {
    let text = read_frame_string(reader)?;
    serde_json::from_str(&text).map_err(FrameError::Json)
}

pub fn read_ipc_request(reader: &mut impl Read) -> Result<IpcRequest, FrameError> {
    read_frame(reader)
}

pub fn write_ipc_response(
    writer: &mut impl Write,
    response: &IpcResponse,
) -> Result<(), FrameError> {
    write_frame(writer, response)
}

pub fn read_frame_string(reader: &mut impl Read) -> Result<String, FrameError> {
    let mut length_bytes = [0u8; 4];
    reader.read_exact(&mut length_bytes)?;
    let len = u32::from_le_bytes(length_bytes) as usize;
    if len == 0 {
        return Err(FrameError::ZeroLength);
    }
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::Oversized {
            len,
            max: MAX_FRAME_BYTES,
        });
    }
    let mut buffer = vec![0u8; len];
    let mut read = 0usize;
    while read < len {
        match reader.read(&mut buffer[read..]) {
            Ok(0) => {
                return Err(FrameError::Truncated {
                    expected: len,
                    actual: read,
                });
            }
            Ok(count) => read += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(FrameError::Io(error)),
        }
    }
    String::from_utf8(buffer).map_err(FrameError::InvalidUtf8)
}

pub fn pipe_name_for_owner_sid(owner_sid: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"lucidity-agent-terminal-v1\0");
    hasher.update(owner_sid.as_bytes());
    let digest = hasher.finalize();
    let mut suffix = String::with_capacity(32);
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        let _ = write!(suffix, "{byte:02x}");
    }
    format!(r"\\.\pipe\lucidity-agent-{suffix}")
}

pub fn owner_dacl_sddl(owner_sid: &str) -> String {
    format!("D:P(A;;GA;;;{owner_sid})(A;;GA;;;SY)")
}

#[cfg(windows)]
pub mod named_pipe {
    use super::*;
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, BOOL, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, ERROR_PIPE_CONNECTED, HANDLE,
        INVALID_HANDLE_VALUE, PSID, WAIT_FAILED, WAIT_TIMEOUT,
    };
    use windows::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows::Win32::Security::{
        EqualSid, GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
        TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::Storage::FileSystem::{
        ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        PIPE_ACCESS_DUPLEX, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    use windows::Win32::System::Threading::{
        CreateEventW, GetCurrentProcess, OpenProcess, OpenProcessToken, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION, WAIT_OBJECT_0,
    };
    use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

    const IO_POLL_MILLIS: u32 = 25;

    pub struct PipeSecurity {
        descriptor: *mut SECURITY_DESCRIPTOR,
        attrs: SECURITY_ATTRIBUTES,
    }

    pub struct NamedPipeServer {
        handle: HANDLE,
        pub name: String,
        _security: PipeSecurity,
    }

    // Both resources are uniquely owned, moved into the service thread before
    // use, and closed/freed by their Drop implementations on that same thread.
    unsafe impl Send for PipeSecurity {}
    unsafe impl Send for NamedPipeServer {}

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ConnectOutcome {
        Connected,
        Stopped,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ReadChunkOutcome {
        Bytes(usize),
        TimedOut,
        Stopped,
    }

    pub(crate) struct ConnectedPipe<'a> {
        server: &'a NamedPipeServer,
        stop: &'a AtomicBool,
    }

    struct OverlappedEvent(HANDLE);

    impl PipeSecurity {
        pub fn owner_only(owner_sid: &str) -> Result<Self> {
            let sddl = owner_dacl_sddl(owner_sid);
            let wide = wide_null(&sddl);
            let mut descriptor: *mut SECURITY_DESCRIPTOR = null_mut();
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PCWSTR(wide.as_ptr()),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    null_mut(),
                )
                .ok()
                .map_err(|error| anyhow!("ConvertStringSecurityDescriptor failed: {error}"))?;
            }
            let attrs = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.cast::<c_void>(),
                bInheritHandle: BOOL(0),
            };
            Ok(Self { descriptor, attrs })
        }

        pub fn as_ptr(&self) -> *const SECURITY_ATTRIBUTES {
            &self.attrs
        }
    }

    impl Drop for PipeSecurity {
        fn drop(&mut self) {
            if !self.descriptor.is_null() {
                unsafe {
                    winapi::um::winbase::LocalFree(self.descriptor.cast());
                }
            }
        }
    }

    impl NamedPipeServer {
        pub fn create_for_current_user() -> Result<Self> {
            let owner_sid = current_user_sid_string()?;
            Self::create_for_owner_sid(&owner_sid)
        }

        pub fn create_for_owner_sid(owner_sid: &str) -> Result<Self> {
            let name = pipe_name_for_owner_sid(owner_sid);
            Self::create_named_for_owner_sid(name, owner_sid)
        }

        pub(crate) fn create_named_for_owner_sid(
            name: impl Into<String>,
            owner_sid: &str,
        ) -> Result<Self> {
            let name = name.into();
            if !name.to_ascii_lowercase().starts_with(r"\\.\pipe\") {
                bail!("named-pipe server path must be local: {name}");
            }
            let security = PipeSecurity::owner_only(owner_sid)?;
            let wide_name = wide_null(&name);
            let open_mode = PIPE_ACCESS_DUPLEX
                | FILE_FLAG_FIRST_PIPE_INSTANCE
                | FILE_FLAG_OVERLAPPED
                | SECURITY_SQOS_PRESENT
                | SECURITY_IDENTIFICATION;
            let pipe_mode =
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
            let handle = unsafe {
                CreateNamedPipeW(
                    PCWSTR(wide_name.as_ptr()),
                    open_mode,
                    pipe_mode,
                    1,
                    MAX_FRAME_BYTES as u32,
                    MAX_FRAME_BYTES as u32,
                    0,
                    security.as_ptr(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                bail!(
                    "CreateNamedPipeW failed for {}: {}",
                    name,
                    windows::core::Error::from_win32()
                );
            }
            Ok(Self {
                handle,
                name,
                _security: security,
            })
        }

        pub(crate) fn connect_interruptible(
            &self,
            stop: &AtomicBool,
        ) -> io::Result<ConnectOutcome> {
            if stop.load(Ordering::Acquire) {
                return Ok(ConnectOutcome::Stopped);
            }

            let event = OverlappedEvent::new()?;
            let mut overlapped = event.overlapped();
            let connected = unsafe { ConnectNamedPipe(self.handle, &mut overlapped) };
            if connected.as_bool() {
                return Ok(ConnectOutcome::Connected);
            }
            let error = io::Error::last_os_error();
            match error.raw_os_error().map(|code| code as u32) {
                Some(code) if code == ERROR_PIPE_CONNECTED.0 => Ok(ConnectOutcome::Connected),
                Some(code) if code == ERROR_IO_PENDING.0 => {
                    match self.wait_for_overlapped(&overlapped, stop)? {
                        Some(_) => Ok(ConnectOutcome::Connected),
                        None => Ok(ConnectOutcome::Stopped),
                    }
                }
                _ => Err(error),
            }
        }

        pub(crate) fn connected<'a>(&'a self, stop: &'a AtomicBool) -> ConnectedPipe<'a> {
            ConnectedPipe { server: self, stop }
        }

        pub(crate) fn disconnect(&self) {
            unsafe {
                let _ = DisconnectNamedPipe(self.handle);
            }
        }

        fn read_interruptible(&self, buffer: &mut [u8], stop: &AtomicBool) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            if stop.load(Ordering::Acquire) {
                return Err(stopped_io_error());
            }
            let event = OverlappedEvent::new()?;
            let mut overlapped = event.overlapped();
            let started = unsafe {
                ReadFile(
                    self.handle,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    null_mut(),
                    &mut overlapped,
                )
            };
            if started.as_bool() {
                return self.completed_bytes(&overlapped);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error().map(|code| code as u32) == Some(ERROR_IO_PENDING.0) {
                self.wait_for_overlapped(&overlapped, stop)?
                    .ok_or_else(stopped_io_error)
                    .map(|count| count as usize)
            } else {
                Err(error)
            }
        }

        /// Reads one byte-mode pipe chunk, returning periodically so the host
        /// service can flush subscribed events even when the client is idle.
        /// A timed-out overlapped read is always cancelled and reaped before
        /// this method returns, so the caller can safely reuse its buffer.
        pub(crate) fn read_chunk_interruptible_for(
            &self,
            buffer: &mut [u8],
            stop: &AtomicBool,
            wait_millis: u32,
        ) -> io::Result<ReadChunkOutcome> {
            if buffer.is_empty() {
                return Ok(ReadChunkOutcome::Bytes(0));
            }
            if stop.load(Ordering::Acquire) {
                return Ok(ReadChunkOutcome::Stopped);
            }

            let event = OverlappedEvent::new()?;
            let mut overlapped = event.overlapped();
            match unsafe { ReadFile(self.handle, Some(buffer), None, Some(&mut overlapped)) } {
                Ok(()) => self
                    .completed_bytes(&overlapped)
                    .map(ReadChunkOutcome::Bytes),
                Err(error) if is_win32_error(&error, ERROR_IO_PENDING.0) => {
                    let wait =
                        unsafe { WaitForSingleObject(overlapped.hEvent, wait_millis.max(1)) };
                    if wait == WAIT_OBJECT_0 {
                        return self
                            .completed_bytes(&overlapped)
                            .map(ReadChunkOutcome::Bytes);
                    }
                    if wait == WAIT_FAILED {
                        return Err(io::Error::last_os_error());
                    }
                    if wait != WAIT_TIMEOUT {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("unexpected named-pipe wait result {}", wait.0),
                        ));
                    }

                    let stopped = stop.load(Ordering::Acquire);
                    unsafe {
                        // The operation may win the race with cancellation. In
                        // that case GetOverlappedResult succeeds and the bytes
                        // must be retained rather than reported as a timeout.
                        let _ = CancelIoEx(self.handle, Some(&overlapped));
                        let reaped = WaitForSingleObject(overlapped.hEvent, u32::MAX);
                        if reaped == WAIT_FAILED {
                            return Err(io::Error::last_os_error());
                        }
                        let mut transferred = 0u32;
                        match GetOverlappedResult(self.handle, &overlapped, &mut transferred, false)
                        {
                            Ok(()) => Ok(ReadChunkOutcome::Bytes(transferred as usize)),
                            Err(error) if is_win32_error(&error, ERROR_OPERATION_ABORTED.0) => {
                                if stopped {
                                    Ok(ReadChunkOutcome::Stopped)
                                } else {
                                    Ok(ReadChunkOutcome::TimedOut)
                                }
                            }
                            Err(error) => Err(to_io_error(error)),
                        }
                    }
                }
                Err(error) => Err(to_io_error(error)),
            }
        }

        fn write_interruptible(&self, buffer: &[u8], stop: &AtomicBool) -> io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            if stop.load(Ordering::Acquire) {
                return Err(stopped_io_error());
            }
            let event = OverlappedEvent::new()?;
            let mut overlapped = event.overlapped();
            let started = unsafe {
                WriteFile(
                    self.handle,
                    buffer.as_ptr().cast(),
                    buffer.len() as u32,
                    null_mut(),
                    &mut overlapped,
                )
            };
            if started.as_bool() {
                return self.completed_bytes(&overlapped);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error().map(|code| code as u32) == Some(ERROR_IO_PENDING.0) {
                self.wait_for_overlapped(&overlapped, stop)?
                    .ok_or_else(stopped_io_error)
                    .map(|count| count as usize)
            } else {
                Err(error)
            }
        }

        fn completed_bytes(&self, overlapped: &OVERLAPPED) -> io::Result<usize> {
            let mut transferred = 0u32;
            unsafe {
                GetOverlappedResult(self.handle, overlapped, &mut transferred, false)
                    .ok()
                    .map_err(to_io_error)?;
            }
            Ok(transferred as usize)
        }

        /// Waits for an overlapped operation. `None` means the service stop flag
        /// won the race and the pending operation was cancelled and reaped.
        fn wait_for_overlapped(
            &self,
            overlapped: &OVERLAPPED,
            stop: &AtomicBool,
        ) -> io::Result<Option<u32>> {
            loop {
                if stop.load(Ordering::Acquire) {
                    unsafe {
                        let _ = CancelIoEx(self.handle, overlapped);
                        let _ = WaitForSingleObject(overlapped.hEvent, u32::MAX);
                        let mut ignored = 0u32;
                        let result =
                            GetOverlappedResult(self.handle, overlapped, &mut ignored, false);
                        if let Err(error) = result.ok() {
                            if !is_win32_error(&error, ERROR_OPERATION_ABORTED.0) {
                                log::debug!("named-pipe cancellation completed with {error}");
                            }
                        }
                    }
                    return Ok(None);
                }

                let wait = unsafe { WaitForSingleObject(overlapped.hEvent, IO_POLL_MILLIS) };
                if wait == WAIT_OBJECT_0 {
                    let mut transferred = 0u32;
                    unsafe {
                        GetOverlappedResult(self.handle, overlapped, &mut transferred, false)
                            .ok()
                            .map_err(to_io_error)?;
                    }
                    return Ok(Some(transferred));
                }
                if wait == WAIT_TIMEOUT.0 {
                    continue;
                }
                if wait == WAIT_FAILED.0 {
                    return Err(io::Error::last_os_error());
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("unexpected named-pipe wait result {wait}"),
                ));
            }
        }

        pub fn verify_connected_client_same_user(&self, owner_sid: &str) -> Result<u32> {
            let mut pid = 0u32;
            unsafe {
                GetNamedPipeClientProcessId(self.handle, &mut pid)
                    .ok()
                    .map_err(|error| anyhow!("GetNamedPipeClientProcessId failed: {error}"))?;
            }
            let client_sid = process_user_sid_string(pid)?;
            if client_sid != owner_sid {
                bail!("named-pipe client SID mismatch: owner={owner_sid}, client={client_sid}");
            }
            Ok(pid)
        }
    }

    impl Drop for NamedPipeServer {
        fn drop(&mut self) {
            if self.handle != INVALID_HANDLE_VALUE {
                unsafe {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }

    impl Read for ConnectedPipe<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.server.read_interruptible(buffer, self.stop)
        }
    }

    impl Write for ConnectedPipe<'_> {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.server.write_interruptible(buffer, self.stop)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl OverlappedEvent {
        fn new() -> io::Result<Self> {
            let handle = unsafe { CreateEventW(null(), false, false, PCWSTR(std::ptr::null())) };
            if handle == HANDLE(0) {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(handle))
        }

        fn overlapped(&self) -> OVERLAPPED {
            OVERLAPPED {
                hEvent: self.0,
                ..Default::default()
            }
        }
    }

    impl Drop for OverlappedEvent {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    pub fn current_user_sid_string() -> Result<String> {
        unsafe {
            let mut token = HANDLE(0);
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .ok()
                .map_err(|error| anyhow!("OpenProcessToken(current) failed: {error}"))?;
            let result = token_user_sid_string(token);
            let _ = CloseHandle(token);
            result
        }
    }

    pub fn process_user_sid_string(pid: u32) -> Result<String> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            if process == HANDLE(0) {
                bail!(
                    "OpenProcess({pid}) failed: {}",
                    windows::core::Error::from_win32()
                );
            }
            let mut token = HANDLE(0);
            let token_result = OpenProcessToken(process, TOKEN_QUERY, &mut token)
                .ok()
                .map_err(|error| anyhow!("OpenProcessToken({pid}) failed: {error}"));
            let result = match token_result {
                Ok(()) => {
                    let sid = token_user_sid_string(token);
                    let _ = CloseHandle(token);
                    sid
                }
                Err(error) => Err(error),
            };
            let _ = CloseHandle(process);
            result
        }
    }

    pub fn process_sid_equals_owner(pid: u32, owner_sid: &str) -> Result<bool> {
        let client_sid = process_user_sid_string(pid)?;
        Ok(client_sid == owner_sid)
    }

    unsafe fn token_user_sid_string(token: HANDLE) -> Result<String> {
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        if needed == 0 {
            bail!("GetTokenInformation(TokenUser) did not report a buffer size");
        }
        let mut buffer = vec![0u8; needed as usize];
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast::<c_void>(),
            needed,
            &mut needed,
        )
        .ok()
        .map_err(|error| anyhow!("GetTokenInformation(TokenUser) failed: {error}"))?;
        let token_user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
        sid_to_string(token_user.User.Sid)
    }

    unsafe fn sid_to_string(sid: PSID) -> Result<String> {
        let mut sid_string = PWSTR(null_mut());
        ConvertSidToStringSidW(sid, &mut sid_string)
            .ok()
            .map_err(|error| anyhow!("ConvertSidToStringSidW failed: {error}"))?;
        let text = pwstr_to_string(sid_string)?;
        winapi::um::winbase::LocalFree(sid_string.0.cast());
        Ok(text)
    }

    #[allow(dead_code)]
    unsafe fn same_sid(left: PSID, right: PSID) -> bool {
        EqualSid(left, right).as_bool()
    }

    unsafe fn pwstr_to_string(value: PWSTR) -> Result<String> {
        if value.0.is_null() {
            bail!("null PWSTR");
        }
        let mut len = 0usize;
        while *value.0.add(len) != 0 {
            len += 1;
        }
        Ok(String::from_utf16_lossy(std::slice::from_raw_parts(
            value.0, len,
        )))
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn is_win32_error(error: &windows::core::Error, code: u32) -> bool {
        error.code().0 == (0x8007_0000u32 | code) as i32
    }

    fn to_io_error(error: windows::core::Error) -> io::Error {
        let raw = error.code().0 as u32;
        if raw & 0xffff_0000 == 0x8007_0000 {
            io::Error::from_raw_os_error((raw & 0xffff) as i32)
        } else {
            io::Error::new(io::ErrorKind::Other, error)
        }
    }

    fn stopped_io_error() -> io::Error {
        io::Error::new(io::ErrorKind::Interrupted, "host service stopped")
    }
}

#[cfg(not(windows))]
pub mod named_pipe {
    use super::*;

    pub fn current_user_sid_string() -> Result<String> {
        bail!("Windows named-pipe SID probing is unavailable on this platform")
    }

    pub fn process_user_sid_string(_pid: u32) -> Result<String> {
        bail!("Windows named-pipe SID probing is unavailable on this platform")
    }

    pub fn process_sid_equals_owner(_pid: u32, _owner_sid: &str) -> Result<bool> {
        bail!("Windows named-pipe SID probing is unavailable on this platform")
    }
}
