//! Capture one plugin's stderr without touching its framed stdout JSON-RPC.
//! A bounded buffer prevents a plugin that omits newlines from consuming
//! unbounded memory. Excess bytes are dropped until the next newline.
use std::{
    io::{BufReader, Read},
    thread,
};

pub(super) fn spawn(plugin_id: String, mut stderr: BufReader<Box<dyn Read + Send>>) {
    thread::Builder::new()
        .name("fiex-plugin-stderr".into())
        .spawn(move || {
            let mut chunk = [0u8; 4096];
            let mut line = Vec::with_capacity(512);
            loop {
                match stderr.read(&mut chunk) {
                    Ok(0) => {
                        emit(&plugin_id, &line);
                        return;
                    }
                    Ok(size) => {
                        for &byte in &chunk[..size] {
                            if byte == b'\n' {
                                emit(&plugin_id, &line);
                                line.clear();
                            } else if line.len() < 4096 {
                                line.push(byte);
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return,
                }
            }
        })
        .expect("plugin stderr reader");
}

fn emit(plugin_id: &str, bytes: &[u8]) {
    let message = String::from_utf8_lossy(bytes);
    let message = message.trim();
    if !message.is_empty() {
        crate::infrastructure::logging::report_plugin_stderr(plugin_id, message);
    }
}
