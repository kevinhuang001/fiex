//! Plugin command execution: spawn, capped capture, exec environment and teardown.
use super::rpc::Running;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXEC_OUTPUT_LIMIT: usize = 1024 * 1024;
const EXEC_TIMEOUT: Duration = Duration::from_secs(120);

/// Plugin commands are headless. On Windows a console-subsystem child would
/// otherwise flash a black window when the plugin shells out (GPG, Git, ...).
#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x08000000);
}

#[cfg(not(windows))]
fn hide_console(_: &mut Command) {}

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
    if !crate::plugins::security::permissions::valid_command(command)
        || !crate::plugins::security::permissions::allowed_command(plugin_id, declared, command)?
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
    hide_console(&mut process);
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
    // Command output is returned only to the owning plugin; the diagnostic
    // log records timing/status/byte counts, never arguments, stdin or output.
    crate::plugins::errors::report_log_limited(
        plugin_id,
        if status.success() { "info" } else { "warn" },
        &format!(
            "host.process.run {command} exit={:?} stdout={}B stderr={}B truncated={}",
            status.code(),
            stdout.len(),
            stderr.len(),
            out_truncated || err_truncated,
        ),
    );
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
        self.stop();
    }
}
