//! Plugin command execution: spawn, capped capture, exec environment and teardown.
use super::rpc::Running;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXEC_OUTPUT_LIMIT: usize = 1024 * 1024;
const EXEC_TIMEOUT: Duration = Duration::from_secs(120);

fn read_capped(mut reader: impl Read) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if output.len() + read > EXEC_OUTPUT_LIMIT {
                    let keep = EXEC_OUTPUT_LIMIT.saturating_sub(output.len());
                    output.extend_from_slice(&buffer[..keep]);
                    truncated = true;
                    break;
                }
                output.extend_from_slice(&buffer[..read]);
            }
            Err(_) => break,
        }
    }
    (output, truncated)
}

pub(super) fn host_exec(
    plugin_id: &str,
    declared: &BTreeSet<String>,
    params: &Value,
) -> Result<Value, String> {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .ok_or("missing command")?;
    if !crate::plugin_permissions::valid_command(command)
        || !crate::plugin_permissions::allowed_command(plugin_id, declared, command)?
    {
        return Err("command was not approved by the user".into());
    }
    let args = params
        .get("args")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "command arguments must be strings".to_string())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if args.len() > 128
        || args
            .iter()
            .any(|arg| arg.len() > 8192 || arg.contains('\0'))
    {
        return Err("command arguments are too large".into());
    }
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .map(|path| dunce::canonicalize(path).map_err(|error| error.to_string()))
        .transpose()?;
    if cwd.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("command working directory is not a folder".into());
    }
    let executable =
        which::which(command).map_err(|error| format!("command unavailable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never");
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "TEMP",
        "TMP",
        "SystemRoot",
        "PATHEXT",
        "SSH_AUTH_SOCK",
        "GNUPGHOME",
        "GPG_TTY",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    if let Some(cwd) = &cwd {
        process.current_dir(cwd);
    }
    let mut child = process
        .spawn()
        .map_err(|error| format!("command failed to start: {error}"))?;
    if let Some(stdin) = params.get("stdin").and_then(Value::as_str) {
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(stdin.as_bytes())
                .map_err(|error| error.to_string())?;
        }
    } else {
        drop(child.stdin.take());
    }
    let out_thread = child
        .stdout
        .take()
        .map(|stream| thread::spawn(move || read_capped(stream)));
    let err_thread = child
        .stderr
        .take()
        .map(|stream| thread::spawn(move || read_capped(stream)));
    let deadline = Instant::now() + EXEC_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command timed out".into());
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let (stdout, out_truncated) = out_thread
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let (stderr, err_truncated) = err_thread
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    Ok(json!({
        "code": status.code(),
        "stdout": String::from_utf8_lossy(&stdout),
        "stdoutBase64": engine.encode(&stdout),
        "stderr": String::from_utf8_lossy(&stderr),
        "stderrBase64": engine.encode(&stderr),
        "truncated": out_truncated || err_truncated
    }))
}

pub fn run_command(
    plugin_id: &str,
    declared: &BTreeSet<String>,
    params: &Value,
    task_root: Option<&Path>,
) -> Result<Value, String> {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .ok_or("missing command")?;
    if !crate::plugin_permissions::valid_command(command)
        || !crate::plugin_permissions::allowed_command(plugin_id, declared, command)?
    {
        return Err("command was not approved by the user".into());
    }
    let args = params
        .get("args")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "command arguments must be strings".to_string())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if args.len() > 128
        || args
            .iter()
            .any(|arg| arg.len() > 8192 || arg.contains('\0'))
    {
        return Err("command arguments are too large".into());
    }
    let cwd = task_root
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    if !cwd.is_dir() {
        return Err("active directory is unavailable".into());
    }
    let executable =
        which::which(command).map_err(|error| format!("command unavailable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .args(&args)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("LC_ALL", "C");
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "TEMP",
        "TMP",
        "SystemRoot",
        "PATHEXT",
        "SSH_AUTH_SOCK",
        "GNUPGHOME",
        "GPG_TTY",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    let mut child = process
        .spawn()
        .map_err(|error| format!("command failed to start: {error}"))?;
    let input = params.get("stdin").and_then(Value::as_str).unwrap_or("");
    if input.len() > 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("command input is too large".into());
    }
    let (input_sender, input_receiver) = std::sync::mpsc::sync_channel(1);
    let input = input.to_owned();
    let input_pipe = child.stdin.take();
    thread::spawn(move || {
        let result = input_pipe.map_or(Ok(()), |mut pipe| pipe.write_all(input.as_bytes()));
        let _ = input_sender.send(result);
    });
    let capture = |stream: Option<std::process::ChildStdout>| {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let value = stream.map(read_capped).unwrap_or_default();
            let _ = sender.send(value);
        });
        receiver
    };
    let out_receiver = capture(child.stdout.take());
    let (err_sender, err_receiver) = std::sync::mpsc::sync_channel(1);
    let err_stream = child.stderr.take();
    thread::spawn(move || {
        let value = err_stream.map(read_capped).unwrap_or_default();
        let _ = err_sender.send(value);
    });
    let deadline = Instant::now() + EXEC_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command timed out".into());
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let remaining = || deadline.saturating_duration_since(Instant::now());
    input_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command input timed out")?
        .map_err(|error| error.to_string())?;
    let (stdout, out_truncated) = out_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command output timed out")?;
    let (stderr, err_truncated) = err_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command error output timed out")?;
    use base64::Engine;
    Ok(json!({
        "code": status.code(),
        "stdoutBase64": base64::engine::general_purpose::STANDARD.encode(&stdout),
        "stderrBase64": base64::engine::general_purpose::STANDARD.encode(&stderr),
        "truncated": out_truncated || err_truncated
    }))
}

impl Drop for Running {
    fn drop(&mut self) {
        // Dropping SandboxChild terminates the plugin and its process tree.
        crate::plugin_fs_access::cleanup(&self.resource_owner);
        crate::plugin_net::cleanup(&self.resource_owner);
        crate::plugin_ipc::cleanup(&self.resource_owner);
        let _ = &self.child;
    }
}
