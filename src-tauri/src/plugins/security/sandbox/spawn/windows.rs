use super::*;

#[cfg(windows)]
pub fn spawn(policy: &Policy) -> Result<SandboxChild, String> {
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
    let mut child = super::super::windows_stdio::spawn(&executable, &payload)?;
    let stdin = child.stdin.take().ok_or("plugin stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("plugin stdout unavailable")?;
    Ok(SandboxChild {
        stdin: Box::new(stdin),
        stdout: BufReader::new(Box::new(stdout) as Box<dyn Read + Send>),
        _scratch: scratch,
        process: child,
    })
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
