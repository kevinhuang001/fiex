use super::*;

#[cfg(target_os = "macos")]
pub fn run_wrapper_if_requested() {}

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
