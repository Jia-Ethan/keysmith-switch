use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::error::{Error, Result};
use crate::paths::AppPaths;

/// Exclusive write lock for `KEYSMITH_SWITCH_HOME/.lock`.
#[derive(Debug)]
pub struct HomeLock {
    file: File,
}

impl HomeLock {
    pub fn acquire(paths: &AppPaths) -> Result<Self> {
        paths.ensure()?;
        let file = open_lock_file(paths)?;
        lock_exclusive(&file, false)?;
        write_holder(&file)?;
        Ok(Self { file })
    }

    pub fn try_acquire(paths: &AppPaths) -> Result<Self> {
        paths.ensure()?;
        let file = open_lock_file(paths)?;
        lock_exclusive(&file, true)?;
        write_holder(&file)?;
        Ok(Self { file })
    }
}

impl Drop for HomeLock {
    fn drop(&mut self) {
        let _ = unlock_file(&self.file);
    }
}

fn open_lock_file(paths: &AppPaths) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&paths.lock)
        .map_err(Error::from)
}

fn write_holder(file: &File) -> Result<()> {
    let mut file = file.try_clone()?;
    file.set_len(0)?;
    let payload = format!(
        "{{\"pid\":{},\"acquired_at\":\"{}\"}}\n",
        std::process::id(),
        crate::models::now_rfc3339()
    );
    file.write_all(payload.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[cfg(unix)]
fn lock_exclusive(file: &File, non_blocking: bool) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let mut flags = libc::LOCK_EX;
    if non_blocking {
        flags |= libc::LOCK_NB;
    }
    let rc = unsafe { libc::flock(file.as_raw_fd(), flags) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if non_blocking
        && matches!(
            err.raw_os_error(),
            Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN
        )
    {
        return Err(Error::lock("home lock is held by another process"));
    }
    Err(Error::lock(err.to_string()))
}

#[cfg(unix)]
fn unlock_file(file: &File) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    if rc == 0 {
        Ok(())
    } else {
        Err(Error::lock(std::io::Error::last_os_error().to_string()))
    }
}

#[cfg(windows)]
const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x0000_0001;
#[cfg(windows)]
const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;

#[cfg(windows)]
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: *mut core::ffi::c_void,
}

#[cfg(windows)]
impl Overlapped {
    fn zeroed() -> Self {
        Self {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event: std::ptr::null_mut(),
        }
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LockFileEx(
        file: *mut core::ffi::c_void,
        flags: u32,
        reserved: u32,
        bytes_low: u32,
        bytes_high: u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn UnlockFileEx(
        file: *mut core::ffi::c_void,
        reserved: u32,
        bytes_low: u32,
        bytes_high: u32,
        overlapped: *mut Overlapped,
    ) -> i32;
}

#[cfg(windows)]
fn lock_exclusive(file: &File, non_blocking: bool) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    let handle = file.as_raw_handle();
    let mut flags = LOCKFILE_EXCLUSIVE_LOCK;
    if non_blocking {
        flags |= LOCKFILE_FAIL_IMMEDIATELY;
    }
    let mut overlapped = Overlapped::zeroed();
    let ok = unsafe { LockFileEx(handle, flags, 0, u32::MAX, u32::MAX, &mut overlapped) };
    if ok == 0 {
        if non_blocking {
            return Err(Error::lock("home lock is held by another process"));
        }
        return Err(Error::lock(std::io::Error::last_os_error().to_string()));
    }
    Ok(())
}

#[cfg(windows)]
fn unlock_file(file: &File) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    let handle = file.as_raw_handle();
    let mut overlapped = Overlapped::zeroed();
    let ok = unsafe { UnlockFileEx(handle, 0, u32::MAX, u32::MAX, &mut overlapped) };
    if ok == 0 {
        Err(Error::lock(std::io::Error::last_os_error().to_string()))
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn lock_exclusive(_file: &File, _non_blocking: bool) -> Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn unlock_file(_file: &File) -> Result<()> {
    Ok(())
}
