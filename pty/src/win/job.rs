use filedescriptor::OwnedHandle;
use std::io::{Error as IoError, Result as IoResult};
use std::mem;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr;
use winapi::um::jobapi2::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject, TerminateJobObject,
};
use winapi::um::winnt::{
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[derive(Debug)]
pub(super) struct OwnedJob {
    handle: OwnedHandle,
}

impl OwnedJob {
    pub(super) fn new_kill_on_close() -> IoResult<Self> {
        let handle = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
        if handle.is_null() {
            return Err(IoError::last_os_error());
        }

        let handle = unsafe { OwnedHandle::from_raw_handle(handle as _) };
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let result = unsafe {
            SetInformationJobObject(
                handle.as_raw_handle() as _,
                JobObjectExtendedLimitInformation,
                &mut limits as *mut _ as _,
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if result == 0 {
            return Err(IoError::last_os_error());
        }

        Ok(Self { handle })
    }

    pub(super) fn assign_process(&self, process: &OwnedHandle) -> IoResult<()> {
        let result = unsafe {
            AssignProcessToJobObject(
                self.handle.as_raw_handle() as _,
                process.as_raw_handle() as _,
            )
        };
        if result == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn terminate(&self, exit_code: u32) -> IoResult<()> {
        let result = unsafe { TerminateJobObject(self.handle.as_raw_handle() as _, exit_code) };
        if result == 0 {
            Err(IoError::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn try_clone(&self) -> filedescriptor::Result<Self> {
        Ok(Self {
            handle: self.handle.try_clone()?,
        })
    }
}
