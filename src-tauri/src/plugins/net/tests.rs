use super::request::send_streamed;

use super::*;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::collections::BTreeSet;
use std::io::Write;
use std::net::TcpListener;
use std::thread;

fn pipe_with(length: u64, timeout: Duration) -> Arc<BodyPipe> {
    Arc::new(BodyPipe::new(length, Instant::now() + timeout))
}

#[test]
fn streaming_pipe_enforces_chunk_accounting_and_short_bodies() {
    let pipe = pipe_with(4, Duration::from_secs(5));
    assert_eq!(pipe.push(b"ab".to_vec(), false).unwrap(), 2);
    // The declared contentLength is the hard upper bound.
    assert!(pipe.push(b"cde".to_vec(), false).is_err());
    assert_eq!(pipe.push(b"cd".to_vec(), true).unwrap(), 4);
    // Writes after eof are rejected.
    assert!(pipe.push(Vec::new(), true).is_err());
    assert!(pipe.push(b"x".to_vec(), false).is_err());

    let mut reader = BodyReader { pipe: pipe.clone() };
    let mut output = [0u8; 2];
    assert_eq!(reader.read(&mut output).unwrap(), 2);
    assert_eq!(&output, b"ab");
    let mut rest = [0u8; 8];
    assert_eq!(reader.read(&mut rest).unwrap(), 2);
    assert_eq!(&rest[..2], b"cd");
    // Exactly contentLength bytes: the reader stops without waiting for
    // another chunk.
    assert_eq!(reader.read(&mut rest).unwrap(), 0);

    // A short eof fails the body instead of hanging.
    let pipe = pipe_with(3, Duration::from_secs(5));
    pipe.push(b"ab".to_vec(), true).unwrap();
    let mut reader = BodyReader { pipe };
    let mut output = [0u8; 8];
    assert_eq!(reader.read(&mut output).unwrap(), 2);
    let error = reader.read(&mut output).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("2 of 3 bytes"));
}

#[test]
fn streaming_pipe_blocks_for_chunks_and_times_out_without_them() {
    let pipe = pipe_with(3, Duration::from_secs(5));
    let reader_pipe = pipe.clone();
    let task = thread::spawn(move || {
        let mut reader = BodyReader { pipe: reader_pipe };
        let mut output = [0u8; 8];
        reader.read(&mut output).map(|read| output[..read].to_vec())
    });
    thread::sleep(Duration::from_millis(50));
    pipe.push(b"xyz".to_vec(), true).unwrap();
    assert_eq!(task.join().unwrap().unwrap(), b"xyz");

    // A channel that never reaches contentLength fails with a deadline
    // error, and the failure is visible to the worker.
    let pipe = pipe_with(1, Duration::from_millis(80));
    let mut reader = BodyReader { pipe: pipe.clone() };
    let error = reader.read(&mut [0u8; 4]).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(pipe
        .read_error()
        .is_some_and(|message| message.contains("timed out")));
}

#[test]
fn streaming_parameters_are_validated_before_any_io() {
    let declared = BTreeSet::new();
    let policy = PermissionPolicy::new("fixture", &declared);
    let open = |params: Value| http_open("owner", &policy, &params);

    let error = open(json!({"url": "https://example.com", "bodyStream": true})).unwrap_err();
    assert!(error.contains("contentLength"));
    let error = open(json!({
        "url": "https://example.com",
        "bodyStream": true,
        "contentLength": 1,
        "bodyBase64": ""
    }))
    .unwrap_err();
    assert!(error.contains("bodyBase64"));
    let error = open(json!({
        "url": "https://example.com",
        "bodyStream": true,
        "contentLength": 1,
        "bodyReadHandle": "handle"
    }))
    .unwrap_err();
    assert!(error.contains("bodyReadHandle"));
    let error = open(json!({
        "url": "https://example.com",
        "bodyStream": true,
        "contentLength": MAX_STREAM_BODY + 1
    }))
    .unwrap_err();
    assert!(error.contains("4 GiB"));
}

fn test_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .unwrap()
}

fn serve_once(response: &'static [u8]) -> (reqwest::Url, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url =
        reqwest::Url::parse(&format!("http://{}/upload", listener.local_addr().unwrap())).unwrap();
    let task = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut head_end = 0;
        while head_end == 0 {
            let read = socket.read(&mut chunk).unwrap();
            if read == 0 {
                return request;
            }
            request.extend_from_slice(&chunk[..read]);
            head_end = request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| position + 4)
                .unwrap_or(0);
        }
        let head = String::from_utf8_lossy(&request[..head_end]).to_ascii_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        while request.len() < head_end + length {
            let read = socket.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..read]);
        }
        socket.write_all(response).unwrap();
        request
    });
    (url, task)
}

fn send_to(url: reqwest::Url, length: u64) -> Result<reqwest::blocking::Response, String> {
    let pipe = pipe_with(length, Duration::from_secs(5));
    if length > 0 {
        pipe.push(vec![7; length as usize], true).unwrap();
    }
    send_streamed(
        test_client(),
        "PUT".into(),
        url,
        reqwest::header::HeaderMap::new(),
        BodyReader { pipe },
        length,
    )
}

#[test]
fn streamed_requests_do_not_follow_redirects() {
    let (url, server) = serve_once(
            b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
    let error = send_to(url, 4).unwrap_err();
    assert!(error.contains("redirect"), "unexpected error: {error}");
    let request = server.join().unwrap();
    assert!(request.starts_with(b"PUT /upload"));
}

#[test]
fn streamed_308_without_location_is_a_normal_response() {
    let (url, server) = serve_once(
        b"HTTP/1.1 308 Permanent Redirect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let response = send_to(url, 4).unwrap();
    assert_eq!(response.status().as_u16(), 308);
    drop(response);
    server.join().unwrap();
}

#[test]
fn streaming_body_chunks_accept_the_final_eof_and_reject_late_writes() {
    // The SDK sends `eof` even when the worker already consumed exactly
    // contentLength; that final call must be a no-op, not an error.
    let upload = Upload::new(2, Duration::from_secs(5));
    assert_eq!(upload.pipe.push(b"hi".to_vec(), false).unwrap(), 2);
    upload.pipe.close();
    assert_eq!(upload.pipe.push(Vec::new(), true).unwrap(), 2);
    assert!(upload.pipe.push(b"x".to_vec(), false).is_err());
}

#[test]
fn body_method_enforces_owner_and_chunk_limits() {
    let upload = Arc::new(Upload::new(8, Duration::from_secs(5)));
    let id = http_id().unwrap();
    http_streams().lock().unwrap().insert(
        id.clone(),
        HttpEntry {
            owner: "owner".into(),
            response: HttpEntryKind::Uploading(upload),
        },
    );
    let chunk = STANDARD.encode(vec![1u8; MAX_BODY_CHUNK + 1]);
    let error = http_body(
        "owner",
        &json!({"handle": id, "base64": chunk, "eof": false}),
    )
    .unwrap_err();
    assert!(error.contains("4 MiB"));
    let error = http_body("other", &json!({"handle": id, "base64": "", "eof": true})).unwrap_err();
    assert!(error.contains("unavailable"));
    http_close("owner", &json!({"stream": id})).unwrap();
}
