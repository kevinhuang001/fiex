//! Local PTY lifecycle. Connections and remote terminals are plugin-owned.
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{Mutex, OnceLock},
};
use tauri::ipc::Channel;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
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

pub(super) fn home() -> String {
    dirs::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".into())
}

pub(super) fn start_session(
    command: CommandBuilder,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
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
    let _ = on_event.send(TerminalEvent::Ready);
    let id = session_id.clone();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if on_event
                        .send(TerminalEvent::Data {
                            data: buffer[..size].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = on_event.send(TerminalEvent::Error {
                        message: error.to_string(),
                    });
                    break;
                }
            }
        }
        let _ = on_event.send(TerminalEvent::Exit { code: None });
        if let Ok(mut values) = sessions().lock() {
            if let Some(mut session) = values.remove(&id) {
                let _ = session.child.kill();
            }
        }
    });
    Ok(session_id)
}

#[tauri::command]
pub(crate) fn terminal_write(session_id: String, data: Vec<u8>) -> Result<(), String> {
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
pub(crate) fn terminal_resize(session_id: String, cols: u16, rows: u16) -> Result<(), String> {
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
pub(crate) fn terminal_close(session_id: String) -> Result<(), String> {
    if let Ok(mut values) = sessions().lock() {
        if let Some(mut session) = values.remove(&session_id) {
            let _ = session.child.kill();
        }
    }
    Ok(())
}
