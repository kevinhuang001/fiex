use super::*;

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
    let (parent_stderr, child_stderr) = pipe()?;
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
    // Several plugins auto-start together. A millisecond-only id can collide,
    // and arapuca uses it to name the sandbox task/cgroup, so give every launch
    // a process-wide sequence number as well.
    static TASK_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let task_id = format!(
        "plugin-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default(),
        TASK_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let config = Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id,
        phase: "plugin".into(),
        work_dir: Some(scratch.path().to_path_buf()),
        stdin: Some(child_stdin.as_raw_fd()),
        stdout: Some(child_stdout.as_raw_fd()),
        stderr: Some(child_stderr.as_raw_fd()),
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
    drop(child_stderr);
    Ok(SandboxChild {
        stdin: Box::new(parent_stdin),
        stdout: BufReader::new(Box::new(parent_stdout) as Box<dyn Read + Send>),
        stderr: BufReader::new(Box::new(parent_stderr) as Box<dyn Read + Send>),
        _scratch: scratch,
        _process: process,
        _socket_dir: socket_dir,
    })
}
