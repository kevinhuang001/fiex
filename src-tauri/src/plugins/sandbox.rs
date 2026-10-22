//! OS-level plugin confinement.
//!
//! All desktop builds delegate to [`arapuca`]: Landlock + seccomp on Linux,
//! a deny-default Seatbelt profile on macOS, and AppContainer + a Job Object on
//! Windows. Fiex refuses to launch a plugin if the sandbox cannot be applied.
//!
//! A failed sandbox setup is fatal: a plugin is never launched unsandboxed
//! when the host believes a policy is active.
use std::{
    io::{BufReader, Read, Write},
    path::Path,
};

#[derive(Clone, Debug)]
pub struct Policy {
    pub executable: std::path::PathBuf,
    pub install_dir: std::path::PathBuf,
    pub read_roots: Vec<std::path::PathBuf>,
    pub write_roots: Vec<std::path::PathBuf>,
    pub network: bool,
}

fn system_read_roots() -> Vec<std::path::PathBuf> {
    #[cfg(unix)]
    {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        [
            "/usr",
            "/lib",
            "/lib64",
            "/bin",
            "/sbin",
            "/etc",
            "/dev/urandom",
            "/dev/null",
        ]
        .iter()
        .map(|path| canonical(Path::new(path)))
        .collect()
    }
    #[cfg(windows)]
    {
        // AppContainer grants read/execute to System32 and Program Files
        // through the implicit ALL APPLICATION PACKAGES SID. Adding those
        // trees as explicit DACL roots both is unnecessary and can stall
        // CreateProcess while Windows propagates inheritance.
        Vec::new()
    }
}

fn media_roots() -> Vec<std::path::PathBuf> {
    #[cfg(unix)]
    {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        ["/media", "/mnt", "/run/media", "/Volumes"]
            .iter()
            .map(|path| canonical(Path::new(path)))
            .filter(|path| path.exists())
            .collect()
    }
    #[cfg(windows)]
    {
        Vec::new()
    }
}

impl Policy {
    pub fn new(
        executable: &Path,
        install_dir: &Path,
        grant: &crate::plugin_permissions::PluginGrant,
        declared: &std::collections::BTreeSet<String>,
    ) -> Self {
        let canonical = |path: &Path| dunce::canonicalize(path).unwrap_or_else(|_| path.into());
        let executable = canonical(executable);
        let install_dir = canonical(install_dir);
        let readable = declared.contains("filesystem.read");
        let writable = declared.contains("filesystem.write");
        let mut read_roots = vec![executable.clone(), install_dir.clone()];
        read_roots.extend(system_read_roots());
        let mut write_roots = vec![canonical(&std::env::temp_dir()), canonical(&install_dir)];
        if readable {
            if let Some(home) = dirs::home_dir() {
                read_roots.push(canonical(&home));
            }
            read_roots.extend(media_roots());
        }
        if writable {
            if let Some(home) = dirs::home_dir() {
                write_roots.push(canonical(&home));
            }
            write_roots.extend(media_roots());
        }
        read_roots.sort();
        read_roots.dedup();
        write_roots.sort();
        write_roots.dedup();
        Self {
            executable,
            install_dir,
            read_roots,
            write_roots,
            network: grant.granted.contains("network"),
        }
    }
}

pub struct SandboxChild {
    pub stdin: Box<dyn Write + Send>,
    pub stdout: BufReader<Box<dyn Read + Send>>,
    #[cfg(unix)]
    _process: arapuca::Process,
    #[cfg(unix)]
    _socket_dir: tempfile::TempDir,
    #[cfg(windows)]
    process: std::process::Child,
}

#[cfg(windows)]
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

#[cfg(unix)]
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

    let profile = Profile {
        read_paths: policy.read_roots.clone(),
        write_paths: policy.write_roots.clone(),
        allow_exec: false,
        use_netns: false,
        use_pidns: false,
        seccomp_profile: if policy.network {
            SeccompProfile::Baseline
        } else {
            SeccompProfile::Strict
        },
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
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into()),
        ),
        (
            "LANG".to_string(),
            std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".into()),
        ),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
    ];
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
    let config = Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id,
        phase: "plugin".into(),
        work_dir: None,
        stdin: Some(child_stdin.as_raw_fd()),
        stdout: Some(child_stdout.as_raw_fd()),
        stderr: None,
        extra_fds: Vec::new(),
        tty: false,
        network_proxy_socket: None,
        #[cfg(target_os = "linux")]
        allowed_hosts: Vec::new(),
        env,
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
        _process: process,
        _socket_dir: socket_dir,
    })
}

#[cfg(windows)]
pub fn spawn(policy: &Policy) -> Result<SandboxChild, String> {
    use std::process::{Command, Stdio};
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let payload = serde_json::to_string(&LaunchPolicy::from(policy)).map_err(|e| e.to_string())?;
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
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_else(|_| String::new()),
        ),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
    ];
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
    let config = arapuca::Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id: format!("plugin-{}", std::process::id()),
        phase: "plugin".into(),
        work_dir: None,
        network_proxy_socket: None,
        env,
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

#[cfg(all(test, windows))]
mod windows_tests {
    #[test]
    fn sandbox_system_roots_exist() {
        for root in super::system_read_roots() {
            assert!(root.exists(), "missing sandbox root: {}", root.display());
        }
    }
}
