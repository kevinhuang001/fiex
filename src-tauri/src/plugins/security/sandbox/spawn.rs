//! Confined child processes.
//!
//! Each platform either launches the plugin directly through its sandbox
//! provider or re-executes Fiex as a short-lived wrapper that applies the
//! confinement and then hands control to the plugin. A failed setup is always
//! fatal: no plugin is ever launched unsandboxed.

#[cfg(target_os = "macos")]
use super::rules::macos_profile;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use super::rules::plugin_env;

#[cfg(target_os = "linux")]
use super::rules::restrict_plugin_process_creation;

use super::rules::Policy;

use std::io::{BufReader, Read, Write};

pub struct SandboxChild {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: BufReader<Box<dyn Read + Send>>,
    pub stderr: BufReader<Box<dyn Read + Send>>,
    /// Private scratch space. Some codecs (RAR) need temporary spool files;
    /// they still cannot write to the user's directory or install package.
    _scratch: tempfile::TempDir,
    #[cfg(target_os = "linux")]
    _process: arapuca::Process,
    #[cfg(target_os = "linux")]
    _socket_dir: tempfile::TempDir,
    #[cfg(any(windows, target_os = "macos"))]
    process: std::process::Child,
}

#[cfg(any(windows, target_os = "macos"))]
impl Drop for SandboxChild {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(not(any(unix, windows)))]
pub fn spawn(_policy: &Policy) -> Result<SandboxChild, String> {
    Err("this platform has no plugin sandbox implementation".into())
}

#[derive(Clone)]
pub struct KillHandle(std::sync::Arc<(u32, std::sync::atomic::AtomicBool)>);

impl KillHandle {
    pub fn terminate(&self) {
        if self.0 .1.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(self.0 .0 as i32, libc::SIGKILL);
        }
        #[cfg(windows)]
        unsafe {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
                fn TerminateProcess(handle: *mut std::ffi::c_void, code: u32) -> i32;
                fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
            }
            let handle = OpenProcess(1, 0, self.0 .0);
            if !handle.is_null() {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
}

impl SandboxChild {
    pub fn exit_diagnostic(&mut self) -> String {
        #[cfg(any(windows, target_os = "macos"))]
        {
            match self.process.try_wait() {
                Ok(Some(status)) => status.to_string(),
                Ok(None) => "control pipe closed before process exit".into(),
                Err(error) => error.to_string(),
            }
        }
        #[cfg(target_os = "linux")]
        {
            "control pipe closed".into()
        }
    }
    pub fn kill_handle(&self) -> KillHandle {
        #[cfg(target_os = "linux")]
        let pid = self._process.pid();
        #[cfg(any(windows, target_os = "macos"))]
        let pid = self.process.id();
        KillHandle(std::sync::Arc::new((
            pid,
            std::sync::atomic::AtomicBool::new(false),
        )))
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{run_wrapper_if_requested, spawn};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{run_wrapper_if_requested, spawn};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{run_wrapper_if_requested, spawn};
