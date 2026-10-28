//! Built-in terminals: a real PTY through the system OpenSSH client or the
//! user's local shell, streamed as bytes to the WebView's xterm.js instance.
//!
//! OpenSSH keeps host-key verification, `~/.ssh/config`, agents, ProxyJump,
//! keyboard-interactive/MFA, algorithms and known_hosts behavior in the same
//! trusted implementation the user already uses. The WebView only receives the
//! byte stream; it never receives credentials.

use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};
use tauri::ipc::Channel;

mod plugin_pty;
pub(crate) use plugin_pty::*;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SshEvent {
    Ready,
    Data { data: Vec<u8> },
    Exit { code: Option<i32> },
    Error { message: String },
}

struct Session {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send>,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static VALUES: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn string_setting(settings: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    settings
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn expanded_path(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    value.to_string()
}

pub(super) fn home() -> String {
    dirs::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}

pub(super) fn start_session(
    command: CommandBuilder,
    cols: u16,
    rows: u16,
    on_event: Channel<SshEvent>,
) -> Result<String, String> {
    let cols = cols.max(20);
    let rows = rows.max(4);
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())?;
    let child = pair.slave.spawn_command(command).map_err(|error| {
        crate::i18n::format("无法启动终端：{error}", &[("error", format!("{}", error))])
    })?;
    drop(pair.slave);
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let session_id = crate::plugins::mounts::new_id()?;
    sessions()
        .lock()
        .map_err(|_| crate::i18n::t("终端会话表不可用"))?
        .insert(
            session_id.clone(),
            Session {
                writer,
                master: pair.master,
                child,
            },
        );
    let id = session_id.clone();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if on_event
                        .send(SshEvent::Data {
                            data: buffer[..size].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = on_event.send(SshEvent::Error {
                        message: error.to_string(),
                    });
                    break;
                }
            }
        }
        let _ = on_event.send(SshEvent::Exit { code: None });
        if let Ok(mut values) = sessions().lock() {
            if let Some(mut session) = values.remove(&id) {
                let _ = session.child.kill();
            }
        }
    });
    Ok(session_id)
}

#[tauri::command]
pub(crate) async fn ssh_open(
    connection_id: String,
    cols: u16,
    rows: u16,
    on_event: Channel<SshEvent>,
) -> Result<String, String> {
    let connection = crate::remote::load_stored()
        .into_iter()
        .find(|connection| connection.id == connection_id)
        .ok_or(crate::i18n::t("SSH 连接不存在"))?;
    if !matches!(
        connection.plugin_id.as_str(),
        "__builtin__" | "org.fiex.ssh"
    ) || (connection.plugin_id == "__builtin__" && connection.kind != "ssh")
    {
        return Err(crate::i18n::t("不是 SSH 连接").into());
    }
    if connection.plugin_id == "org.fiex.ssh" {
        crate::remote::ensure_provider_access(&connection)?;
        return plugin_ssh_open(connection, cols, rows, on_event).await;
    }
    let host = string_setting(&connection.settings, "host");
    if host.is_empty() {
        return Err(crate::i18n::t("SSH 主机为空").into());
    }
    let username = string_setting(&connection.settings, "username");
    let port = connection
        .settings
        .get("port")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(22)
        .clamp(1, 65535) as u16;
    let identity = string_setting(&connection.settings, "identityFile");
    let terminal = string_setting(&connection.settings, "terminalType");
    let terminal = if terminal.is_empty() {
        "xterm-256color".into()
    } else {
        terminal
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = CommandBuilder::new("ssh");
        command.arg("-tt");
        command.arg("-p");
        command.arg(port.to_string());
        command.arg("-o");
        command.arg("ServerAliveInterval=30");
        command.arg("-o");
        command.arg("ServerAliveCountMax=3");
        if !username.is_empty() {
            command.arg("-l");
            command.arg(&username);
        }
        if !identity.is_empty() {
            command.arg("-i");
            command.arg(expanded_path(&identity));
        }
        command.arg(&host);
        command.env("TERM", &terminal);
        command.env("COLORTERM", "truecolor");
        start_session(command, cols, rows, on_event)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn ssh_write(session_id: String, data: Vec<u8>) -> Result<(), String> {
    if let Ok(mut plugin) = plugin_sessions().lock() {
        if let Some(session) = plugin.get_mut(&session_id) {
            session
                .input
                .lock()
                .map_err(|_| crate::i18n::t("终端输入不可用"))?
                .extend(data);
            return Ok(());
        }
    }
    let mut values = sessions()
        .lock()
        .map_err(|_| crate::i18n::t("终端会话表不可用"))?;
    let session = values
        .get_mut(&session_id)
        .ok_or(crate::i18n::t("终端会话已结束"))?;
    session.writer.write_all(&data).map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn ssh_resize(session_id: String, cols: u16, rows: u16) -> Result<(), String> {
    if let Ok(plugin) = plugin_sessions().lock() {
        if let Some(session) = plugin.get(&session_id) {
            *session
                .size
                .lock()
                .map_err(|_| crate::i18n::t("终端尺寸不可用"))? = (cols.max(20), rows.max(4));
            return Ok(());
        }
    }
    let values = sessions()
        .lock()
        .map_err(|_| crate::i18n::t("终端会话表不可用"))?;
    let session = values
        .get(&session_id)
        .ok_or(crate::i18n::t("终端会话已结束"))?;
    session
        .master
        .resize(PtySize {
            rows: rows.max(4),
            cols: cols.max(20),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn ssh_close(session_id: String) -> Result<(), String> {
    if let Ok(mut plugin) = plugin_sessions().lock() {
        if let Some(session) = plugin.remove(&session_id) {
            session.closed.store(true, Ordering::Release);
            let _ = session.channel.send(SshEvent::Exit { code: None });
            return Ok(());
        }
    }
    if let Ok(mut values) = sessions().lock() {
        if let Some(mut session) = values.remove(&session_id) {
            let _ = session.child.kill();
        }
    }
    Ok(())
}
