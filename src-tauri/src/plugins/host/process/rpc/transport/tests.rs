use super::{reader::parse_response, wait_for_response};
use serde_json::{Value, json};
use std::{sync::mpsc, time::Duration};

#[test]
fn plugin_responses_require_core_numeric_ids_and_jsonrpc_2() {
    assert!(parse_response(&json!({"jsonrpc":"2.0","id":"1","result":null})).is_none());

    let (id, result) = parse_response(&json!({
        "jsonrpc":"2.0",
        "id":7,
        "result":{"ready":true}
    }))
    .unwrap();
    assert_eq!(id, 7);
    assert_eq!(result.unwrap(), json!({"ready":true}));

    let (id, error) = parse_response(&json!({"jsonrpc":"1.0","id":8,"result":null})).unwrap();
    assert_eq!(id, 8);
    let error = error.unwrap_err();
    assert!(error.fatal);
    assert_eq!(error.message, "invalid plugin JSON-RPC version");
}

#[test]
fn malformed_plugin_response_envelopes_fail_the_request() {
    let invalid = [
        json!({"jsonrpc":"2.0","id":1}),
        json!({"jsonrpc":"2.0","id":2,"result":null,"error":{"message":"bad"}}),
        json!({"jsonrpc":"2.0","id":3,"error":{"code":-1}}),
        json!({"jsonrpc":"2.0","id":4,"error":{"code":-1.5,"message":"bad"}}),
    ];
    for frame in invalid {
        let (_, error) = parse_response(&frame).unwrap();
        assert!(error.unwrap_err().fatal, "{frame}");
    }
}

#[test]
fn plugin_business_error_does_not_mark_the_transport_fatal() {
    let (_, error) = parse_response(&json!({
        "jsonrpc":"2.0",
        "id":9,
        "error":{"code":-32001,"message":"provider rejected request"}
    }))
    .unwrap();
    assert!(!error.unwrap_err().fatal);
}

#[test]
fn rpc_wait_distinguishes_deadline_disconnect_and_response() {
    let (sender, receiver) = mpsc::channel();
    let timeout = wait_for_response(receiver, Duration::from_millis(1)).unwrap_err();
    assert!(timeout.fatal);
    assert_eq!(timeout.message, "plugin request deadline exceeded");
    drop(sender);

    let (sender, receiver) = mpsc::channel();
    drop(sender);
    let disconnected = wait_for_response(receiver, Duration::from_secs(1)).unwrap_err();
    assert!(disconnected.fatal);
    assert_eq!(disconnected.message, "plugin process disconnected");

    let (sender, receiver) = mpsc::channel();
    sender.send(Ok(Value::Bool(true))).unwrap();
    assert_eq!(
        wait_for_response(receiver, Duration::from_secs(1)).unwrap(),
        Value::Bool(true)
    );
}
