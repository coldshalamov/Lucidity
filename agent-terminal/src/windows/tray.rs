use super::tray_state::TrayState;
use std::ffi::c_void;
use std::io;
use std::mem;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use winapi::shared::minwindef::{HINSTANCE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HICON, HWND, POINT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, GetWindowLongPtrW, LoadIconW, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    SetWindowLongPtrW, TrackPopupMenu, TranslateMessage, UnregisterClassW, CREATESTRUCTW,
    GWLP_USERDATA, HWND_MESSAGE, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, MSG, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};
use windows::Win32::Foundation::HWND as ShellHwnd;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETFOCUS,
    NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::HICON as ShellHicon;

const TRAY_ICON_ID: u32 = 1;
const TRAY_ICON_RESOURCE_ID: usize = 0x101;
const TRAY_CALLBACK_MESSAGE: UINT = WM_APP + 1;
const REQUEST_OPEN_MESSAGE: UINT = WM_APP + 2;
const REQUEST_QUIT_MESSAGE: UINT = WM_APP + 3;
const REQUEST_SHUTDOWN_MESSAGE: UINT = WM_APP + 4;
const OPEN_COMMAND_ID: usize = 0x1001;
const EXIT_COMMAND_ID: usize = 0x1002;
const NIN_KEYSELECT_MESSAGE: UINT = NIN_SELECT | 1;
const TOOLTIP: &str = "Lucidity Agent Terminal";

static TRAY_INSTANCE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Host lifecycle seam for the native tray thread.
///
/// Callbacks execute on the tray message-loop thread and must schedule work
/// onto the owning GUI/host loop. `request_open_existing_window` must restore
/// the existing mux window rather than create one. `request_quit_owned` must
/// terminate only Agent-owned Jobs, drain persistence, and request host exit.
pub trait TrayCallbacks: Send + Sync + 'static {
    fn request_open_existing_window(&self);
    fn request_quit_owned(&self);
}

/// Owns the process's single message-only tray window and its message loop.
pub struct TrayController {
    hwnd: isize,
    taskbar_created_message: UINT,
    thread: Option<JoinHandle<()>>,
}

impl TrayController {
    /// Start the one permitted tray owner for this process.
    pub fn start(callbacks: Arc<dyn TrayCallbacks>) -> io::Result<Self> {
        if TRAY_INSTANCE_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the Lucidity tray controller is already running",
            ));
        }

        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = match thread::Builder::new()
            .name("lucidity-tray".to_string())
            .spawn(move || {
                let _instance = TrayInstanceLease;
                run_message_loop(callbacks, ready_tx);
            }) {
            Ok(thread) => thread,
            Err(err) => {
                TRAY_INSTANCE_ACTIVE.store(false, Ordering::Release);
                return Err(err);
            }
        };

        match ready_rx.recv() {
            Ok(Ok(receipt)) => Ok(Self {
                hwnd: receipt.hwnd,
                taskbar_created_message: receipt.taskbar_created_message,
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread.join();
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "tray thread exited before reporting its HWND",
                ))
            }
        }
    }

    /// Schedule deterministic restoration/focus of the existing Agent window.
    pub fn request_open(&self) -> io::Result<()> {
        self.post(REQUEST_OPEN_MESSAGE)
    }

    /// Schedule the one explicit owned-Quit transition.
    pub fn request_quit_owned(&self) -> io::Result<()> {
        self.post(REQUEST_QUIT_MESSAGE)
    }

    /// The registered message that an existing top-level GUI window receives
    /// after Explorer recreates the taskbar.
    pub fn taskbar_created_message(&self) -> UINT {
        self.taskbar_created_message
    }

    /// Forward a window message from the existing top-level GUI HWND.
    ///
    /// A true message-only window does not receive broadcasts, so the existing
    /// GUI HWND must call this seam. No second hidden/top-level window is used.
    pub fn forward_window_message(&self, message: UINT) -> io::Result<bool> {
        if message != self.taskbar_created_message {
            return Ok(false);
        }
        self.post(message)?;
        Ok(true)
    }

    /// Stop the tray owner without selecting the host's owned-Quit policy.
    pub fn shutdown(mut self) -> io::Result<()> {
        self.stop_and_join()
    }

    fn post(&self, message: UINT) -> io::Result<()> {
        let posted = unsafe { PostMessageW(self.hwnd as HWND, message, 0, 0) };
        if posted == 0 {
            Err(last_error("PostMessageW to tray controller"))
        } else {
            Ok(())
        }
    }

    fn stop_and_join(&mut self) -> io::Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };

        let post_result = if thread.is_finished() {
            Ok(())
        } else {
            self.post(REQUEST_SHUTDOWN_MESSAGE)
        };

        if thread.thread().id() == thread::current().id() {
            return post_result;
        }
        let join_result = thread
            .join()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "tray message-loop thread panicked"));
        post_result.and(join_result)
    }
}

impl Drop for TrayController {
    fn drop(&mut self) {
        if let Err(err) = self.stop_and_join() {
            log::error!("failed to stop tray controller: {err:#}");
        }
    }
}

struct TrayInstanceLease;

impl Drop for TrayInstanceLease {
    fn drop(&mut self) {
        TRAY_INSTANCE_ACTIVE.store(false, Ordering::Release);
    }
}

struct StartupReceipt {
    hwnd: isize,
    taskbar_created_message: UINT,
}

fn run_message_loop(
    callbacks: Arc<dyn TrayCallbacks>,
    ready_tx: mpsc::SyncSender<io::Result<StartupReceipt>>,
) {
    let tray = match NativeTray::create(callbacks) {
        Ok(tray) => tray,
        Err(err) => {
            let _ = ready_tx.send(Err(err));
            return;
        }
    };

    if ready_tx
        .send(Ok(StartupReceipt {
            hwnd: tray.hwnd as isize,
            taskbar_created_message: tray.taskbar_created_message(),
        }))
        .is_err()
    {
        return;
    }

    let mut message: MSG = unsafe { mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result == -1 {
            log::error!("tray GetMessageW failed: {:#}", io::Error::last_os_error());
            break;
        }
        if result == 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct NativeTray {
    hwnd: HWND,
    state: *mut TrayWindowState,
    instance: HINSTANCE,
    class_name: Vec<u16>,
}

impl NativeTray {
    fn create(callbacks: Arc<dyn TrayCallbacks>) -> io::Result<Self> {
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        if instance.is_null() {
            return Err(last_error("GetModuleHandleW for tray owner"));
        }

        let taskbar_created_message =
            unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) };
        if taskbar_created_message == 0 {
            return Err(last_error("RegisterWindowMessageW(TaskbarCreated)"));
        }

        let icon = load_tray_icon(instance)?;
        let class_name = wide(&format!(
            "Lucidity.AgentTray.MessageOnly.{}",
            std::process::id()
        ));
        let mut window_class: WNDCLASSW = unsafe { mem::zeroed() };
        window_class.lpfnWndProc = Some(tray_window_proc);
        window_class.hInstance = instance;
        window_class.lpszClassName = class_name.as_ptr();

        if unsafe { RegisterClassW(&window_class) } == 0 {
            return Err(last_error("RegisterClassW for tray owner"));
        }

        let state = Box::into_raw(Box::new(TrayWindowState {
            model: TrayState::new(),
            callbacks,
            taskbar_created_message,
            icon,
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                instance,
                state.cast::<c_void>(),
            )
        };
        if hwnd.is_null() {
            let err = last_error("CreateWindowExW(HWND_MESSAGE) for tray owner");
            unsafe {
                drop(Box::from_raw(state));
                UnregisterClassW(class_name.as_ptr(), instance);
            }
            return Err(err);
        }

        if let Err(err) = unsafe { add_icon(hwnd, icon) } {
            unsafe {
                DestroyWindow(hwnd);
                drop(Box::from_raw(state));
                UnregisterClassW(class_name.as_ptr(), instance);
            }
            return Err(err);
        }
        unsafe {
            (*state).model.icon_added();
        }

        Ok(Self {
            hwnd,
            state,
            instance,
            class_name,
        })
    }

    fn taskbar_created_message(&self) -> UINT {
        unsafe { (*self.state).taskbar_created_message }
    }
}

impl Drop for NativeTray {
    fn drop(&mut self) {
        unsafe {
            if (*self.state).model.stop() {
                if let Err(err) = delete_icon(self.hwnd) {
                    log::warn!("failed to remove tray icon during shutdown: {err:#}");
                }
            }
            DestroyWindow(self.hwnd);
            drop(Box::from_raw(self.state));
            if UnregisterClassW(self.class_name.as_ptr(), self.instance) == 0 {
                log::warn!(
                    "failed to unregister tray window class: {:#}",
                    io::Error::last_os_error()
                );
            }
        }
    }
}

struct TrayWindowState {
    model: TrayState,
    callbacks: Arc<dyn TrayCallbacks>,
    taskbar_created_message: UINT,
    icon: HICON,
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    message: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
    }

    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayWindowState;
    if !state.is_null() {
        let taskbar_created_message = (*state).taskbar_created_message;
        if message == taskbar_created_message {
            handle_taskbar_created(hwnd, state);
            return 0;
        }

        match message {
            TRAY_CALLBACK_MESSAGE => {
                handle_tray_callback(hwnd, state, lparam);
                return 0;
            }
            REQUEST_OPEN_MESSAGE => {
                dispatch_open(state);
                return 0;
            }
            REQUEST_QUIT_MESSAGE => {
                dispatch_quit(hwnd, state);
                return 0;
            }
            REQUEST_SHUTDOWN_MESSAGE => {
                dispatch_shutdown(hwnd, state);
                return 0;
            }
            WM_COMMAND => {
                dispatch_command(hwnd, state, low_word(wparam as usize) as usize);
                return 0;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return 0;
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            _ => {}
        }
    }

    DefWindowProcW(hwnd, message, wparam, lparam)
}

unsafe fn handle_taskbar_created(hwnd: HWND, state: *mut TrayWindowState) {
    let should_readd = (*state).model.taskbar_created();
    let icon = (*state).icon;
    if !should_readd {
        return;
    }
    match add_icon(hwnd, icon) {
        Ok(()) => (*state).model.icon_added(),
        Err(err) => log::error!("failed to restore tray icon after TaskbarCreated: {err:#}"),
    }
}

unsafe fn handle_tray_callback(hwnd: HWND, state: *mut TrayWindowState, lparam: LPARAM) {
    match low_word(lparam as usize) as UINT {
        NIN_SELECT | NIN_KEYSELECT_MESSAGE | WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
            dispatch_open(state)
        }
        WM_CONTEXTMENU | WM_RBUTTONUP => match show_context_menu(hwnd) {
            Ok(Some(command)) => dispatch_command(hwnd, state, command),
            Ok(None) => focus_tray_icon(hwnd),
            Err(err) => log::error!("failed to show tray context menu: {err:#}"),
        },
        _ => {}
    }
}

unsafe fn dispatch_command(hwnd: HWND, state: *mut TrayWindowState, command: usize) {
    match command {
        OPEN_COMMAND_ID => dispatch_open(state),
        EXIT_COMMAND_ID => dispatch_quit(hwnd, state),
        _ => {}
    }
}

unsafe fn dispatch_open(state: *mut TrayWindowState) {
    let callbacks = {
        let state = &mut *state;
        if !state.model.begin_open() {
            return;
        }
        Arc::clone(&state.callbacks)
    };

    if catch_unwind(AssertUnwindSafe(|| {
        callbacks.request_open_existing_window()
    }))
    .is_err()
    {
        log::error!("tray Open callback panicked");
    }
    (*state).model.finish_open();
}

unsafe fn dispatch_quit(hwnd: HWND, state: *mut TrayWindowState) {
    let (delete, callbacks) = {
        let state = &mut *state;
        let Some(delete) = state.model.begin_quit() else {
            return;
        };
        (delete, Arc::clone(&state.callbacks))
    };

    if delete {
        if let Err(err) = delete_icon(hwnd) {
            log::warn!("failed to remove tray icon before owned Quit: {err:#}");
        }
    }
    if catch_unwind(AssertUnwindSafe(|| callbacks.request_quit_owned())).is_err() {
        log::error!("tray owned-Quit callback panicked");
    }
    PostQuitMessage(0);
}

unsafe fn dispatch_shutdown(hwnd: HWND, state: *mut TrayWindowState) {
    let Some(delete) = (*state).model.begin_shutdown() else {
        return;
    };
    if delete {
        if let Err(err) = delete_icon(hwnd) {
            log::warn!("failed to remove tray icon during controller shutdown: {err:#}");
        }
    }
    PostQuitMessage(0);
}

unsafe fn show_context_menu(hwnd: HWND) -> io::Result<Option<usize>> {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return Err(last_error("CreatePopupMenu for tray"));
    }

    let result = (|| {
        let open = wide("Open Lucidity");
        let exit = wide("Exit");
        if AppendMenuW(menu, MF_STRING, OPEN_COMMAND_ID, open.as_ptr()) == 0
            || AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null()) == 0
            || AppendMenuW(menu, MF_STRING, EXIT_COMMAND_ID, exit.as_ptr()) == 0
        {
            return Err(last_error("AppendMenuW for tray"));
        }

        let mut cursor: POINT = mem::zeroed();
        if GetCursorPos(&mut cursor) == 0 {
            return Err(last_error("GetCursorPos for tray menu"));
        }

        SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_NONOTIFY | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            0,
            hwnd,
            ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        if command == 0 {
            Ok(None)
        } else {
            Ok(Some(command as usize))
        }
    })();

    DestroyMenu(menu);
    result
}

unsafe fn add_icon(hwnd: HWND, icon: HICON) -> io::Result<()> {
    let mut data = notify_icon_data(hwnd, icon);
    if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
        return Err(last_error("Shell_NotifyIconW(NIM_ADD)"));
    }

    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    if !Shell_NotifyIconW(NIM_SETVERSION, &data).as_bool() {
        Shell_NotifyIconW(NIM_DELETE, &data);
        return Err(last_error("Shell_NotifyIconW(NIM_SETVERSION)"));
    }
    Ok(())
}

unsafe fn delete_icon(hwnd: HWND) -> io::Result<()> {
    let data = notify_icon_data(hwnd, ptr::null_mut());
    if Shell_NotifyIconW(NIM_DELETE, &data).as_bool() {
        Ok(())
    } else {
        Err(last_error("Shell_NotifyIconW(NIM_DELETE)"))
    }
}

unsafe fn focus_tray_icon(hwnd: HWND) {
    let data = notify_icon_data(hwnd, ptr::null_mut());
    if !Shell_NotifyIconW(NIM_SETFOCUS, &data).as_bool() {
        log::debug!("Shell_NotifyIconW(NIM_SETFOCUS) did not find the tray icon");
    }
}

fn notify_icon_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW::default();
    data.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = ShellHwnd(hwnd as isize);
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY_CALLBACK_MESSAGE;
    data.hIcon = ShellHicon(icon as isize);
    write_wide(&mut data.szTip, TOOLTIP);
    data
}

fn load_tray_icon(instance: HINSTANCE) -> io::Result<HICON> {
    let mut icon = unsafe { LoadIconW(instance, TRAY_ICON_RESOURCE_ID as *const u16) };
    if icon.is_null() {
        icon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
    }
    if icon.is_null() {
        Err(last_error("LoadIconW for tray"))
    } else {
        Ok(icon)
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn write_wide(destination: &mut [u16], value: &str) {
    let max_chars = destination.len().saturating_sub(1);
    for (slot, value) in destination
        .iter_mut()
        .take(max_chars)
        .zip(value.encode_utf16())
    {
        *slot = value;
    }
}

fn low_word(value: usize) -> u16 {
    (value & 0xffff) as u16
}

fn last_error(action: &str) -> io::Error {
    let source = io::Error::last_os_error();
    io::Error::new(source.kind(), format!("{action}: {source}"))
}
