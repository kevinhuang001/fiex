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

/// When Fiex itself is re-executed as Arapuca's wrapper, apply the sandbox
/// and `execve` the real plugin. Called before any runtime/thread starts.
#[cfg(target_os = "linux")]
pub fn run_wrapper_if_requested() {
    use std::{
        ffi::{CString, OsStr},
        os::unix::ffi::OsStrExt,
    };
    if std::env::var_os("ARAPUCA_WRAPPER").as_deref() != Some(OsStr::new("1")) {
        return;
    }
    if let Err(error) = restrict_plugin_process_creation() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    let args = std::env::args_os()
        .map(|arg| CString::new(arg.as_bytes()).unwrap_or_default())
        .collect::<Vec<_>>();
    let pointers = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();
    arapuca::selfexec::arapuca_handle_selfexec_if_wrapper(
        pointers.len() as libc::c_int,
        pointers.as_ptr(),
    );
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
pub fn run_wrapper_if_requested() {}

#[cfg(target_os = "linux")]
pub fn spawn(policy: &Policy) -> Result<SandboxChild, String> {
    use arapuca::platform::{new, Sandbox};
    use arapuca::{CgroupPolicy, Config, Profile, SeccompProfile};
    use std::os::fd::{AsRawFd, FromRawFd};

    fn pipe() -> Result<(std::fs::File, std::fs::File), String> {
        let mut fds = [0i32; 2];
        #[cfg(target_os = "linux")]
        let result = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        #[cfg(not(target_os = "linux"))]
        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if result != 0 {
            return Err(format!(
                "cannot create plugin IPC pipe: {}",
                std::io::Error::last_os_error()
            ));
        }
        #[cfg(not(target_os = "linux"))]
        for fd in fds {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
                return Err(format!(
                    "cannot mark plugin IPC pipe close-on-exec: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        // SAFETY: pipe2 returned two fresh descriptors owned by this process.
        Ok(unsafe {
            (
                std::fs::File::from_raw_fd(fds[0]),
                std::fs::File::from_raw_fd(fds[1]),
            )
        })
    }

    let (child_stdin, parent_stdin) = pipe()?;
    let (parent_stdout, child_stdout) = pipe()?;
    #[cfg(target_os = "linux")]
    arapuca::selfexec::enable_selfexec_mode();
    let sandbox = new().map_err(|e| format!("cannot initialize plugin sandbox: {e}"))?;
    sandbox
        .available()
        .map_err(|e| format!("plugin sandbox is unavailable: {e}"))?;

    // A private, empty scratch directory is the only writable path a plugin
    // receives. Several codecs spool to a temp file even when the final output
    // goes through the host file API.
    let scratch = tempfile::Builder::new()
        .prefix("fiex-plugin-scratch-")
        .tempdir()
        .map_err(|e| format!("plugin scratch directory unavailable: {e}"))?;
    let mut policy = policy.clone();
    policy.write_roots.push(scratch.path().to_path_buf());

    let profile = Profile {
        read_paths: policy.read_roots.clone(),
        write_paths: policy.write_roots.clone(),
        allow_exec: false,
        deny_unix_sockets: true,
        use_netns: false,
        use_pidns: false,
        seccomp_profile: SeccompProfile::Strict,
        cgroup_policy: CgroupPolicy::BestEffort,
        max_open_files: 1024,
        ..Profile::default()
    };

    let socket_dir = tempfile::Builder::new()
        .prefix("fiex-plugin-sandbox-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let task_id = format!(
        "plugin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    );
    let config = Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id,
        phase: "plugin".into(),
        work_dir: Some(scratch.path().to_path_buf()),
        stdin: Some(child_stdin.as_raw_fd()),
        stdout: Some(child_stdout.as_raw_fd()),
        stderr: None,
        extra_fds: Vec::new(),
        tty: false,
        network_proxy_socket: None,
        #[cfg(target_os = "linux")]
        allowed_hosts: Vec::new(),
        env: plugin_env(scratch.path()),
        audit_sink: None,
        audit_verbosity: arapuca::audit::AuditVerbosity::Standard,
        audit_principal: None,
        audit_correlation_id: None,
    };
    let process = sandbox
        .launch(&config, &policy.executable.to_string_lossy(), &[])
        .map_err(|e| format!("plugin start failed: {e}"))?;
    drop(child_stdin);
    drop(child_stdout);
    Ok(SandboxChild {
        stdin: Box::new(parent_stdin),
        stdout: BufReader::new(Box::new(parent_stdout) as Box<dyn Read + Send>),
        _scratch: scratch,
        _process: process,
        _socket_dir: socket_dir,
    })
}

#[cfg(target_os = "macos")]
pub fn spawn(policy: &Policy) -> Result<SandboxChild, String> {
    use std::process::{Command, Stdio};
    let scratch = tempfile::Builder::new()
        .prefix("fiex-plugin-scratch-")
        .tempdir()
        .map_err(|e| format!("plugin scratch directory unavailable: {e}"))?;
    let mut policy = policy.clone();
    policy.write_roots.push(scratch.path().to_path_buf());
    let profile = macos_profile(&policy);
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command
        .args(["-p", &profile])
        .arg(&policy.executable)
        .current_dir(scratch.path())
        .env_clear();
    for (key, value) in plugin_env(scratch.path()) {
        command.env(key, value);
    }
    let mut process = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("plugin sandbox launch failed: {e}"))?;
    let stdin = process.stdin.take().ok_or("missing plugin stdin")?;
    let stdout = process.stdout.take().ok_or("missing plugin stdout")?;
    Ok(SandboxChild {
        stdin: Box::new(stdin),
        stdout: BufReader::new(Box::new(stdout)),
        _scratch: scratch,
        process,
    })
}

#[cfg(windows)]
pub fn spawn(policy: &Policy) -> Result<SandboxChild, String> {
    use std::process::{Command, Stdio};
    let scratch = tempfile::Builder::new()
        .prefix("fiex-plugin-scratch-")
        .tempdir()
        .map_err(|e| format!("plugin scratch directory unavailable: {e}"))?;
    let mut policy = policy.clone();
    policy.write_roots.push(scratch.path().to_path_buf());
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut launch = LaunchPolicy::from(&policy);
    launch.scratch = scratch.path().to_string_lossy().into_owned();
    let payload = serde_json::to_string(&launch).map_err(|e| e.to_string())?;
    let mut child = Command::new(executable)
        .arg("--fiex-plugin-sandbox")
        .arg(payload)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("plugin sandbox launcher failed: {e}"))?;
    let stdin = child.stdin.take().ok_or("plugin stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("plugin stdout unavailable")?;
    Ok(SandboxChild {
        stdin: Box::new(stdin),
        stdout: BufReader::new(Box::new(stdout) as Box<dyn Read + Send>),
        _scratch: scratch,
        process: child,
    })
}

#[cfg(not(any(unix, windows)))]
pub fn spawn(_policy: &Policy) -> Result<SandboxChild, String> {
    Err("this platform has no plugin sandbox implementation".into())
}

#[cfg(windows)]
#[derive(serde::Serialize, serde::Deserialize)]
struct LaunchPolicy {
    executable: String,
    read_paths: Vec<String>,
    write_paths: Vec<String>,
    network: bool,
    scratch: String,
}

#[cfg(windows)]
impl From<&Policy> for LaunchPolicy {
    fn from(policy: &Policy) -> Self {
        Self {
            executable: policy.executable.to_string_lossy().into_owned(),
            read_paths: policy
                .read_roots
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            write_paths: policy
                .write_roots
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            network: policy.network,
            scratch: String::new(),
        }
    }
}

/// Windows: Fiex spawns a short-lived launcher of itself with inherited stdio,
/// and the launcher uses Arapuca's AppContainer/Job Object support. Fiex's IPC
/// pipes are inherited by the launcher and then by the plugin, so the control
/// protocol never passes through a shell or an absolute-path handoff.
#[cfg(windows)]
pub fn run_wrapper_if_requested() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) != Some("--fiex-plugin-sandbox") {
        return;
    }
    let Some(payload) = args.get(2) else {
        eprintln!("fiex-plugin-sandbox: missing policy");
        std::process::exit(1);
    };
    let policy = match serde_json::from_str::<LaunchPolicy>(payload) {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("fiex-plugin-sandbox: invalid policy: {error}");
            std::process::exit(1);
        }
    };
    let sandbox = match arapuca::platform::new() {
        Ok(sandbox) => sandbox,
        Err(error) => {
            eprintln!("fiex-plugin-sandbox: unavailable: {error}");
            std::process::exit(1);
        }
    };
    use arapuca::platform::Sandbox;
    if let Err(error) = sandbox.available() {
        eprintln!("fiex-plugin-sandbox: unavailable: {error}");
        std::process::exit(1);
    }
    let profile = arapuca::Profile {
        read_paths: policy.read_paths.into_iter().map(Into::into).collect(),
        write_paths: policy.write_paths.into_iter().map(Into::into).collect(),
        allow_exec: false,
        // On Windows, `use_netns` controls the AppContainer internet capability.
        use_netns: !policy.network,
        cgroup_policy: arapuca::CgroupPolicy::BestEffort,
        max_open_files: 1024,
        ..arapuca::Profile::default()
    };
    let socket_dir = match tempfile::Builder::new()
        .prefix("fiex-plugin-sandbox-")
        .tempdir()
    {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("fiex-plugin-sandbox: temp dir: {error}");
            std::process::exit(1);
        }
    };
    let config = arapuca::Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id: format!("plugin-{}", std::process::id()),
        phase: "plugin".into(),
        work_dir: (!policy.scratch.is_empty()).then(|| policy.scratch.clone().into()),
        network_proxy_socket: None,
        env: plugin_env(&policy.scratch),
        audit_sink: None,
        audit_verbosity: arapuca::audit::AuditVerbosity::Standard,
        audit_principal: None,
        audit_correlation_id: None,
    };
    let mut process = match sandbox.launch(&config, &policy.executable, &[]) {
        Ok(process) => process,
        Err(error) => {
            eprintln!("fiex-plugin-sandbox: launch failed: {error}");
            std::process::exit(1);
        }
    };
    let status = process.wait();
    process.cleanup();
    drop(socket_dir);
    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("fiex-plugin-sandbox: wait failed: {error}");
            std::process::exit(1);
        }
    }
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
