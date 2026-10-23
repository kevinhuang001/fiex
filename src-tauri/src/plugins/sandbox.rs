//! OS-level plugin confinement.
//!
//! Desktop builds use Landlock + seccomp on Linux, a deny-default Seatbelt
//! profile on macOS, and AppContainer + a Job Object on Windows. Fiex refuses to launch a plugin if the sandbox cannot be applied.
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
            "/usr/lib",
            "/lib",
            "/lib64",
            "/etc/ld.so.cache",
            "/dev/urandom",
            "/dev/null",
        ]
        .iter()
        .filter(|path| Path::new(path).exists())
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
        let mut read_roots = vec![executable.clone()];
        read_roots.extend(system_read_roots());
        let mut write_roots = Vec::new();
        let _ = (grant, declared);
        read_roots.sort();
        read_roots.dedup();
        write_roots.sort();
        write_roots.dedup();
        Self {
            executable,
            install_dir,
            read_roots,
            write_roots,
            network: false,
        }
    }
}

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

/// Arapuca permits ordinary fork/clone and System V IPC for general workloads.
/// Plugins only need threads and inherited pipes, so tighten those before exec.
#[cfg(target_os = "linux")]
fn restrict_plugin_process_creation() -> Result<(), String> {
    let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
    let mut code = vec![instruction(0x20, 0, 0, 0)]; // seccomp_data.nr
    let denied = [
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_semget,
        libc::SYS_semop,
        libc::SYS_semctl,
        libc::SYS_msgget,
        libc::SYS_msgsnd,
        libc::SYS_msgrcv,
        libc::SYS_msgctl,
        libc::SYS_kill,
    ];
    for number in denied {
        code.push(instruction(0x15, 0, 1, number as u32));
        code.push(instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32));
    }
    #[cfg(target_arch = "x86_64")]
    for number in [libc::SYS_fork, libc::SYS_vfork] {
        code.push(instruction(0x15, 0, 1, number as u32));
        code.push(instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32));
    }
    code.extend([
        instruction(0x15, 0, 3, libc::SYS_clone as u32),
        instruction(0x20, 0, 0, 16), // low word of clone flags
        instruction(0x45, 1, 0, libc::CLONE_THREAD as u32),
        instruction(0x06, 0, 0, 0x00050000 | libc::EPERM as u32),
        instruction(0x06, 0, 0, 0x7fff0000),
    ]);
    let program = libc::sock_fprog {
        len: code.len() as u16,
        filter: code.as_mut_ptr(),
    };
    // SAFETY: the kernel copies a live BPF instruction array synchronously.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &program) } != 0
    {
        return Err(format!(
            "plugin process restrictions unavailable: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
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
        (
            "TMPDIR".to_string(),
            scratch.path().to_string_lossy().into_owned(),
        ),
        (
            "TMP".to_string(),
            scratch.path().to_string_lossy().into_owned(),
        ),
        (
            "TEMP".to_string(),
            scratch.path().to_string_lossy().into_owned(),
        ),
    ];
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
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
        _scratch: scratch,
        _process: process,
        _socket_dir: socket_dir,
    })
}

// dyld requires reading the root directory itself during bootstrap. This is a
// literal grant, not recursive access to user files or arbitrary Mach services.
#[cfg(any(test, target_os = "macos"))]
fn macos_profile(policy: &Policy) -> String {
    let quote = |path: &Path| {
        format!(
            "\"{}\"",
            path.to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        )
    };
    let mut profile = String::from("(version 1)(deny default)(allow process-info* (target self))(allow sysctl-read)(allow signal (target self))(allow file-read* (literal \"/\"))");
    profile.push_str(&format!(
        "(allow process-exec (literal {}))",
        quote(&policy.executable)
    ));
    for path in &policy.read_roots {
        profile.push_str(&format!("(allow file-read* (subpath {}))", quote(path)));
    }
    // The only writable path is the private scratch directory created by
    // `spawn`; user directories and the install package remain read-only.
    for path in &policy.write_roots {
        profile.push_str(&format!(
            "(allow file-read* file-write* (subpath {}))",
            quote(path)
        ));
        for ancestor in path.ancestors().skip(1) {
            profile.push_str(&format!(
                "(allow file-read-metadata (literal {}))",
                quote(ancestor)
            ));
        }
    }
    // Resolve only the allowed paths' ancestors; never grant blanket metadata.
    for path in &policy.read_roots {
        for ancestor in path.ancestors().skip(1) {
            profile.push_str(&format!(
                "(allow file-read-metadata (literal {}))",
                quote(ancestor)
            ));
        }
    }
    for ancestor in [
        "/System",
        "/Library",
        "/Library/Apple",
        "/Library/Apple/System",
    ] {
        profile.push_str(&format!(
            "(allow file-read-metadata (literal {}))",
            quote(Path::new(ancestor))
        ));
    }
    // Runtime system resources only; never permit sockets, forks or user paths.
    profile.push_str("(allow file-read* (subpath \"/System/Library\") (subpath \"/System/Cryptexes\") (subpath \"/Library/Apple/System/Library\") (literal \"/dev/null\") (literal \"/dev/urandom\"))");
    profile
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
    let mut process = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", &profile])
        .arg(&policy.executable)
        .current_dir(scratch.path())
        .env_clear()
        .env("LANG", "en_US.UTF-8")
        .env("TMPDIR", scratch.path())
        .env("TMP", scratch.path())
        .env("TEMP", scratch.path())
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
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_else(|_| String::new()),
        ),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
    ];
    if !policy.scratch.is_empty() {
        for key in ["TMPDIR", "TMP", "TEMP"] {
            env.push((key.to_string(), policy.scratch.clone()));
        }
    }
    if let Some(home) = dirs::home_dir() {
        env.push(("HOME".to_string(), home.to_string_lossy().into_owned()));
    }
    let config = arapuca::Config {
        profile,
        socket_dir: socket_dir.path().to_path_buf(),
        task_id: format!("plugin-{}", std::process::id()),
        phase: "plugin".into(),
        work_dir: (!policy.scratch.is_empty()).then(|| policy.scratch.clone().into()),
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

#[cfg(all(test, windows))]
mod windows_tests {
    #[test]
    fn sandbox_system_roots_exist() {
        for root in super::system_read_roots() {
            assert!(root.exists(), "missing sandbox root: {}", root.display());
        }
    }
}

#[cfg(test)]
mod policy_tests {
    #[test]
    fn macos_profile_only_grants_runtime_paths_and_initial_executable() {
        let grant = crate::plugin_permissions::PluginGrant::default();
        let policy = super::Policy::new(
            std::path::Path::new("/plugin/bin"),
            std::path::Path::new("/plugin"),
            &grant,
            &Default::default(),
        );
        let profile = super::macos_profile(&policy);
        assert!(profile.contains("(allow file-read* (literal \"/\"))"));
        assert!(!profile.contains("(allow file-read-metadata)"));
        assert!(!profile.contains("(allow process-fork"));
        assert!(!profile.contains("(allow network"));
        assert!(!profile.contains("(allow mach-lookup"));
        assert!(!profile.contains("(allow process-exec (subpath"));
    }
    #[test]
    fn user_grants_never_open_native_files_or_network() {
        let grant = crate::plugin_permissions::PluginGrant {
            granted: ["network".into(), "files.write".into()].into(),
            ..Default::default()
        };
        let policy = super::Policy::new(
            std::path::Path::new("/plugin/bin"),
            std::path::Path::new("/plugin"),
            &grant,
            &grant.granted,
        );
        assert!(!policy.network);
        assert!(policy.write_roots.is_empty());
        if let Some(home) = dirs::home_dir() {
            assert!(!policy.read_roots.contains(&home));
        }
    }
}
