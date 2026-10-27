use super::{http_streams, text_param};
use crate::{plugin_permission_policy::PermissionPolicy, plugin_permissions};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    sync::{Mutex, OnceLock},
    time::Duration,
};
#[path = "tcp/deadline.rs"]
mod deadline;
use deadline::{io_error, Deadline};

struct TcpEntry {
    owner: String,
    stream: std::sync::Arc<TcpDuplex>,
}

/// Socket clones share a TCP connection, but readers and writers have
/// independent locks. A blocked read must never prevent a concurrent write.
struct TcpDuplex {
    reader: Mutex<TcpStream>,
    writer: Mutex<TcpStream>,
    control: TcpStream,
}

fn streams() -> &'static Mutex<HashMap<String, TcpEntry>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, TcpEntry>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn tcp_open(
    owner: &str,
    policy: &PermissionPolicy<'_>,
    params: &Value,
) -> Result<Value, String> {
    let host = text_param(params, "host")?;
    let port = params
        .get("port")
        .and_then(Value::as_u64)
        .filter(|p| (1..=65535).contains(p))
        .ok_or("invalid TCP port")? as u16;
    let normalized = plugin_permissions::normalize_domain(host)?;
    if !policy.domain_allowed(&normalized)? {
        return Err("TCP target not granted".into());
    }
    let deadline = Deadline::new(params, Duration::from_secs(30))?;
    let addresses = deadline.resolve(normalized, port)?;
    let mut connected = None;
    let mut failure = "TCP target has no address".to_string();
    for address in addresses {
        match TcpStream::connect_timeout(&address, deadline.remaining()?) {
            Ok(stream) => {
                connected = Some((stream, address));
                break;
            }
            Err(error) => failure = io_error(error),
        }
    }
    let (stream, address) = connected.ok_or(failure)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(120)))
        .map_err(|error| error.to_string())?;
    let id = format!(
        "tcp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    );
    let mut entries = streams().lock().map_err(|_| "TCP registry unavailable")?;
    if entries
        .values()
        .filter(|entry| entry.owner == owner)
        .count()
        >= 32
    {
        return Err("too many TCP connections".into());
    }
    let reader = stream.try_clone().map_err(|error| error.to_string())?;
    let writer = stream.try_clone().map_err(|error| error.to_string())?;
    entries.insert(
        id.clone(),
        TcpEntry {
            owner: owner.into(),
            stream: std::sync::Arc::new(TcpDuplex {
                reader: Mutex::new(reader),
                writer: Mutex::new(writer),
                control: stream,
            }),
        },
    );
    Ok(json!({"stream": id, "peerAddress": address.to_string()}))
}

fn tcp_stream(owner: &str, id: &str) -> Result<std::sync::Arc<TcpDuplex>, String> {
    let entries = streams().lock().map_err(|_| "TCP registry unavailable")?;
    entries
        .get(id)
        .filter(|entry| entry.owner == owner)
        .map(|entry| entry.stream.clone())
        .ok_or_else(|| "TCP stream unavailable".into())
}
fn read(stream: &TcpDuplex, params: &Value, bytes: &mut [u8]) -> Result<usize, String> {
    let deadline = Deadline::new(params, Duration::from_secs(120))?;
    let mut reader = deadline.lock(&stream.reader)?;
    reader
        .set_read_timeout(Some(deadline.remaining()?))
        .map_err(io_error)?;
    reader.read(bytes).map_err(io_error)
}
fn write(stream: &TcpDuplex, params: &Value, mut bytes: &[u8]) -> Result<(), String> {
    let deadline = Deadline::new(params, Duration::from_secs(120))?;
    let mut writer = deadline.lock(&stream.writer)?;
    while !bytes.is_empty() {
        writer
            .set_write_timeout(Some(deadline.remaining()?))
            .map_err(io_error)?;
        let size = writer.write(bytes).map_err(io_error)?;
        if size == 0 {
            return Err("TCP write returned zero".into());
        }
        bytes = &bytes[size..];
    }
    Ok(())
}
pub fn tcp_shutdown(owner: &str, params: &Value) -> Result<(), String> {
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    let how = match text_param(params, "direction")? {
        "read" => Shutdown::Read,
        "write" => Shutdown::Write,
        "both" => Shutdown::Both,
        _ => return Err("invalid TCP shutdown direction".into()),
    };
    stream
        .control
        .shutdown(how)
        .map_err(|error| error.to_string())
}

pub fn tcp_close(owner: &str, params: &Value) -> Result<(), String> {
    let id = text_param(params, "stream")?;
    let mut guard = streams().lock().map_err(|_| "TCP registry lock poisoned")?;
    let entry = guard.get(id).ok_or("TCP stream not found")?;
    if entry.owner != owner {
        return Err("TCP stream belongs to another plugin".into());
    }
    let entry = guard.remove(id).ok_or("TCP stream not found")?;
    drop(guard);
    // Wake any pending read, including operations holding cloned sockets.
    let _ = entry.stream.control.shutdown(Shutdown::Both);
    Ok(())
}

pub fn cleanup(owner: &str) {
    if let Ok(mut entries) = http_streams().lock() {
        entries.retain(|_, entry| entry.owner != owner);
    }
    if let Ok(mut entries) = streams().lock() {
        for entry in entries.values().filter(|entry| entry.owner == owner) {
            let _ = entry.stream.control.shutdown(Shutdown::Both);
        }
        entries.retain(|_, entry| entry.owner != owner);
    }
}

pub fn tcp_read_bytes(owner: &str, params: &Value) -> Result<Value, String> {
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(65536)
        .clamp(1, 262144) as usize;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    let mut bytes = vec![0; size];
    let read = read(&stream, params, &mut bytes)?;
    bytes.truncate(read);
    use base64::Engine;
    Ok(
        json!({"base64":base64::engine::general_purpose::STANDARD.encode(bytes),"size":read,"eof":read==0}),
    )
}
pub fn tcp_write_bytes(owner: &str, params: &Value) -> Result<Value, String> {
    let encoded = text_param(params, "base64")?;
    if encoded.len() > 349528 {
        return Err("TCP chunk too large".into());
    }
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    write(&stream, params, &bytes)?;
    Ok(json!({"size":bytes.len()}))
}

#[cfg(test)]
mod tcp_tests {
    use super::*;
    use std::{net::TcpListener, sync::mpsc, thread};

    #[test]
    fn reader_does_not_block_a_concurrent_writer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let duplex = std::sync::Arc::new(TcpDuplex {
            reader: Mutex::new(client.try_clone().unwrap()),
            writer: Mutex::new(client.try_clone().unwrap()),
            control: client,
        });
        let (ready_tx, ready_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let reader = duplex.clone();
        let read_task = thread::spawn(move || {
            let mut stream = reader.reader.lock().unwrap();
            ready_tx.send(()).unwrap();
            let mut bytes = [0; 4];
            stream.read_exact(&mut bytes).unwrap();
            result_tx.send(bytes).unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        duplex.writer.lock().unwrap().write_all(b"ping").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut received = [0; 4];
        server.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"ping");
        server.write_all(b"pong").unwrap();
        assert_eq!(
            result_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            *b"pong"
        );
        read_task.join().unwrap();
    }
}
