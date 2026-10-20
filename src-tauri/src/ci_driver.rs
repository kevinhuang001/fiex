//! Opt-in loopback UI driver, compiled only into the separate CI test binary.
//! Release installers are built without `ci-driver` and never start a server.
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::Duration,
};
use tauri::Manager;
#[derive(Default)]
pub struct Replies(Mutex<HashMap<u64, mpsc::Sender<Value>>>);
static NEXT: AtomicU64 = AtomicU64::new(1);

pub fn reply(app: &tauri::AppHandle, id: u64, value: String) {
    if let Some(send) = app.state::<Replies>().0.lock().unwrap().remove(&id) {
        let _ =
            send.send(serde_json::from_str(&value).unwrap_or(json!({"error": "invalid reply"})));
    }
}
fn request(stream: &mut TcpStream, token: &str) -> Result<Value, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let mut buf = [0; 4096];
    let end = loop {
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 || bytes.len() > 1_048_576 {
            return Err("incomplete request".into());
        }
        bytes.extend_from_slice(&buf[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..end]);
    if !headers
        .lines()
        .any(|line| line == format!("Authorization: Bearer {token}"))
    {
        return Err("unauthorized".into());
    }
    let length: usize = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(k, _)| k.eq_ignore_ascii_case("Content-Length"))
                .and_then(|(_, v)| v.trim().parse().ok())
        })
        .ok_or("missing length")?;
    if length > 1_000_000 {
        return Err("request too large".into());
    }
    while bytes.len() < end + length {
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("incomplete body".into());
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    serde_json::from_slice(&bytes[end..end + length]).map_err(|e| e.to_string())
}
fn handle(app: &tauri::AppHandle, req: Value) -> Result<Value, String> {
    let label = req["window"].as_str().unwrap_or("main");
    let action = req["action"].as_str().unwrap_or("script");
    if action == "windows" {
        return Ok(json!(app.webview_windows().keys().collect::<Vec<_>>()));
    }
    if action == "images" {
        let dir = std::env::var("FIEX_CI_IMAGES").map_err(|e| e.to_string())?;
        let mut images = Vec::new();
        for file in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = file.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|e| e != "img") {
                continue;
            }
            let path = path.to_string_lossy();
            let kind = crate::usbfs::probe(&path)
                .ok_or_else(|| format!("unrecognized filesystem: {path}"))?;
            images.push(json!({"name": std::path::Path::new(path.as_ref()).file_stem().unwrap_or_default().to_string_lossy(), "path": crate::usbfs::virtual_root(&path, kind)}));
        }
        return Ok(json!(images));
    }
    let window = app.get_webview_window(label).ok_or("window missing")?;
    if action == "layout" {
        window
            .set_size(tauri::LogicalSize::new(
                req["width"].as_f64().unwrap_or(900.0),
                req["height"].as_f64().unwrap_or(800.0),
            ))
            .map_err(|e| e.to_string())?;
        window
            .set_position(tauri::LogicalPosition::new(
                req["x"].as_f64().unwrap_or(0.0),
                req["y"].as_f64().unwrap_or(0.0),
            ))
            .map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(json!(true));
    }
    if action == "focus" {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(json!(true));
    }
    let script = req["script"].as_str().ok_or("missing script")?;
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let (send, receive) = mpsc::channel();
    app.state::<Replies>().0.lock().unwrap().insert(id, send);
    let code = format!(
        r#"(async()=>{{{script}}})().then(value=>window.__TAURI_INTERNALS__.invoke('ci_reply',{{id:{id},value:JSON.stringify({{value}})}}),error=>window.__TAURI_INTERNALS__.invoke('ci_reply',{{id:{id},value:JSON.stringify({{error:String(error)}})}}));"#
    );
    window.eval(&code).map_err(|e| e.to_string())?;
    let result = receive.recv_timeout(Duration::from_secs(20));
    app.state::<Replies>().0.lock().unwrap().remove(&id);
    let value = result.map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let position = window.inner_position().map_err(|e| e.to_string())?;
    Ok(
        json!({"result": value, "origin": {"x": position.x as f64 / scale, "y": position.y as f64 / scale}, "scale": scale}),
    )
}
pub fn start(app: tauri::AppHandle) -> Result<(), String> {
    let Ok(port) = std::env::var("FIEX_CI_PORT") else {
        return Ok(());
    };
    let token = std::env::var("FIEX_CI_TOKEN").map_err(|_| "FIEX_CI_TOKEN required")?;
    if token.len() < 32 {
        return Err("CI token must contain at least 32 characters".into());
    }
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    app.manage(Replies::default());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let result = request(&mut stream, &token).and_then(|req| handle(&app, req));
            let (status, body) = match result {
                Ok(v) => ("200 OK", v),
                Err(e) => ("400 Bad Request", json!({"error": e})),
            };
            let body = body.to_string();
            let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    Ok(())
}
