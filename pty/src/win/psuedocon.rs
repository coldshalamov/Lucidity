use super::WinChild;
use crate::cmdbuilder::{CommandBuilder, WindowsProcessTreePolicy};
use crate::win::job::OwnedJob;
use crate::win::procthreadattr::ProcThreadAttributeList;
use anyhow::{bail, ensure, Context as _, Error};
use filedescriptor::{FileDescriptor, OwnedHandle};
use lazy_static::lazy_static;
use shared_library::shared_library;
use std::ffi::OsString;
use std::io::{Error as IoError, ErrorKind};
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::sync::Mutex;
use std::{mem, ptr};
use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::{HRESULT, S_OK};
use winapi::um::handleapi::*;
use winapi::um::processthreadsapi::*;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, WAIT_FAILED, WAIT_OBJECT_0,
};
use winapi::um::wincon::COORD;
use winapi::um::winnt::HANDLE;

pub type HPCON = HANDLE;

pub const PSUEDOCONSOLE_INHERIT_CURSOR: DWORD = 0x1;
pub const PSEUDOCONSOLE_RESIZE_QUIRK: DWORD = 0x2;
pub const PSEUDOCONSOLE_WIN32_INPUT_MODE: DWORD = 0x4;
#[allow(dead_code)]
pub const PSEUDOCONSOLE_PASSTHROUGH_MODE: DWORD = 0x8;

const SUSPENDED_PROCESS_CLEANUP_TIMEOUT_MS: DWORD = 5_000;

fn terminate_suspended_process(process: &OwnedHandle) -> std::io::Result<()> {
    let result = unsafe { TerminateProcess(process.as_raw_handle() as _, 1) };
    if result == 0 {
        return Err(IoError::last_os_error());
    }

    let wait_result = unsafe {
        WaitForSingleObject(
            process.as_raw_handle() as _,
            SUSPENDED_PROCESS_CLEANUP_TIMEOUT_MS,
        )
    };
    match wait_result {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_FAILED => Err(IoError::last_os_error()),
        _ => Err(IoError::new(
            ErrorKind::TimedOut,
            "timed out waiting for suspended process cleanup",
        )),
    }
}

shared_library!(ConPtyFuncs,
    pub fn CreatePseudoConsole(
        size: COORD,
        hInput: HANDLE,
        hOutput: HANDLE,
        flags: DWORD,
        hpc: *mut HPCON
    ) -> HRESULT,
    pub fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT,
    pub fn ClosePseudoConsole(hpc: HPCON),
);

fn load_conpty() -> ConPtyFuncs {
    // If the kernel doesn't export these functions then their system is
    // too old and we cannot run.
    let kernel = ConPtyFuncs::open(Path::new("kernel32.dll")).expect(
        "this system does not support conpty.  Windows 10 October 2018 or newer is required",
    );

    // We prefer to use a sideloaded conpty.dll and openconsole.exe host deployed
    // alongside the application.  We check for this after checking for kernel
    // support so that we don't try to proceed and do something crazy.
    if let Ok(sideloaded) = ConPtyFuncs::open(Path::new("conpty.dll")) {
        sideloaded
    } else {
        kernel
    }
}

lazy_static! {
    static ref CONPTY: ConPtyFuncs = load_conpty();
}

pub struct PsuedoCon {
    con: HPCON,
}

unsafe impl Send for PsuedoCon {}
unsafe impl Sync for PsuedoCon {}

impl Drop for PsuedoCon {
    fn drop(&mut self) {
        unsafe { (CONPTY.ClosePseudoConsole)(self.con) };
    }
}

impl PsuedoCon {
    pub fn new(size: COORD, input: FileDescriptor, output: FileDescriptor) -> Result<Self, Error> {
        let mut con: HPCON = INVALID_HANDLE_VALUE;
        let result = unsafe {
            (CONPTY.CreatePseudoConsole)(
                size,
                input.as_raw_handle() as _,
                output.as_raw_handle() as _,
                PSUEDOCONSOLE_INHERIT_CURSOR
                    | PSEUDOCONSOLE_RESIZE_QUIRK
                    | PSEUDOCONSOLE_WIN32_INPUT_MODE,
                &mut con,
            )
        };
        ensure!(
            result == S_OK,
            "failed to create psuedo console: HRESULT {}",
            result
        );
        Ok(Self { con })
    }

    pub fn resize(&self, size: COORD) -> Result<(), Error> {
        let result = unsafe { (CONPTY.ResizePseudoConsole)(self.con, size) };
        ensure!(
            result == S_OK,
            "failed to resize console to {}x{}: HRESULT: {}",
            size.X,
            size.Y,
            result
        );
        Ok(())
    }

    pub fn spawn_command(&self, cmd: CommandBuilder) -> anyhow::Result<WinChild> {
        let mut si: STARTUPINFOEXW = unsafe { mem::zeroed() };
        si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        // Explicitly set the stdio handles as invalid handles otherwise
        // we can end up with a weird state where the spawned process can
        // inherit the explicitly redirected output handles from its parent.
        // For example, when daemonizing wezterm-mux-server, the stdio handles
        // are redirected to a log file and the spawned process would end up
        // writing its output there instead of to the pty we just created.
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;

        let mut attrs = ProcThreadAttributeList::with_capacity(1)?;
        attrs.set_pty(self.con)?;
        si.lpAttributeList = attrs.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let (mut exe, mut cmdline) = cmd.cmdline()?;
        let cmd_os = OsString::from_wide(&cmdline);

        let cwd = cmd.current_directory();

        let process_tree_policy = cmd.windows_process_tree_policy();
        let owned_job = match process_tree_policy {
            WindowsProcessTreePolicy::DirectChild => None,
            WindowsProcessTreePolicy::OwnedJob => Some(
                OwnedJob::new_kill_on_close()
                    .context("failed to create/configure process-tree Job Object")?,
            ),
        };
        let creation_flags = match process_tree_policy {
            WindowsProcessTreePolicy::DirectChild => {
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT
            }
            WindowsProcessTreePolicy::OwnedJob => {
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED
            }
        };

        let res = unsafe {
            CreateProcessW(
                exe.as_mut_slice().as_mut_ptr(),
                cmdline.as_mut_slice().as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                creation_flags,
                cmd.environment_block().as_mut_slice().as_mut_ptr() as *mut _,
                cwd.as_ref()
                    .map(|c| c.as_slice().as_ptr())
                    .unwrap_or(ptr::null()),
                &mut si.StartupInfo,
                &mut pi,
            )
        };
        if res == 0 {
            let err = IoError::last_os_error();
            let msg = format!(
                "CreateProcessW `{:?}` in cwd `{:?}` failed: {}",
                cmd_os,
                cwd.as_ref().map(|c| OsString::from_wide(c)),
                err
            );
            log::error!("{}", msg);
            bail!("{}", msg);
        }

        // Make sure we close out the thread handle so we don't leak it;
        // we do this simply by making it owned
        let main_thread = unsafe { OwnedHandle::from_raw_handle(pi.hThread as _) };
        let proc = unsafe { OwnedHandle::from_raw_handle(pi.hProcess as _) };

        if let Some(job) = &owned_job {
            if let Err(assign_error) = job.assign_process(&proc) {
                if let Err(cleanup_error) = terminate_suspended_process(&proc) {
                    bail!(
                        "failed to assign suspended process `{:?}` to its Job Object: {}; \
                         launch aborted but suspended-process cleanup also failed: {}",
                        cmd_os,
                        assign_error,
                        cleanup_error
                    );
                }
                bail!(
                    "failed to assign suspended process `{:?}` to its Job Object; \
                     launch aborted: {}",
                    cmd_os,
                    assign_error
                );
            }

            let resume_result = unsafe { ResumeThread(main_thread.as_raw_handle() as _) };
            if resume_result == u32::MAX {
                let resume_error = IoError::last_os_error();
                if let Err(cleanup_error) = terminate_suspended_process(&proc) {
                    bail!(
                        "failed to resume Job-owned process `{:?}`: {}; \
                         launch aborted but suspended-process cleanup also failed: {}",
                        cmd_os,
                        resume_error,
                        cleanup_error
                    );
                }
                bail!(
                    "failed to resume Job-owned process `{:?}`; launch aborted: {}",
                    cmd_os,
                    resume_error
                );
            }
        }

        Ok(WinChild {
            proc: Mutex::new(proc),
            owned_job,
        })
    }
}
