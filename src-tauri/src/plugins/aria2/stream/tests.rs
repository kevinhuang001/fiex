use super::{close, read};
use crate::plugins::aria2::{Shared, Stream, next_id, registry};
use base64::Engine;
use serde_json::json;
use std::{
    fs,
    sync::{Arc, Mutex},
    time::Instant,
};

fn register_stream(owner: &str, bytes: &[u8], segmented: bool) -> String {
    let directory = tempfile::Builder::new()
        .prefix("fiex-aria2-test-")
        .tempdir()
        .unwrap();
    let payload = directory.path().join("payload");
    fs::write(&payload, bytes).unwrap();
    let shared = Arc::new(Mutex::new(Shared {
        done: true,
        error: None,
        total: bytes.len() as u64,
    }));
    let (cancel, _receiver) = tokio::sync::watch::channel(false);
    let id = next_id();
    registry().lock().unwrap().insert(
        id.clone(),
        Arc::new(Mutex::new(Stream {
            owner: owner.to_string(),
            _directory: directory,
            payload,
            file: None,
            offset: 0,
            segmented,
            started: Instant::now(),
            shared,
            cancel,
        })),
    );
    id
}

#[test]
fn another_plugin_cannot_read_or_close_a_stream() {
    let id = register_stream("owner", b"secret", false);

    assert_eq!(
        read("other-plugin", &json!({"streamId": id, "size": 3})).unwrap_err(),
        "unknown aria2 stream"
    );
    assert_eq!(
        close("other-plugin", &json!({"streamId": id})).unwrap_err(),
        "unknown aria2 stream"
    );
    assert!(registry().lock().unwrap().contains_key(&id));

    close("owner", &json!({"streamId": id})).unwrap();
    assert!(!registry().lock().unwrap().contains_key(&id));
}

#[test]
fn reads_return_sequential_chunks_and_signal_eof() {
    let id = register_stream("owner", b"hello", true);

    let first = read("owner", &json!({"streamId": id, "size": 2})).unwrap();
    let second = read("owner", &json!({"streamId": id, "size": 3})).unwrap();
    let end = read("owner", &json!({"streamId": id, "size": 3})).unwrap();

    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(first["base64"].as_str().unwrap())
            .unwrap(),
        b"he"
    );
    assert!(!first["eof"].as_bool().unwrap());
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(second["base64"].as_str().unwrap())
            .unwrap(),
        b"llo"
    );
    assert!(second["eof"].as_bool().unwrap());
    assert_eq!(end, json!({"base64": "", "eof": true}));

    close("owner", &json!({"streamId": id})).unwrap();
}
