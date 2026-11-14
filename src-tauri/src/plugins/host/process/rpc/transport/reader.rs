//! Routes plugin process frames to host methods and pending RPC callers.
use super::super::{RpcFailure, Running};
use crate::plugin_sdk;
use serde_json::{Value, json};
use std::{
    io::{BufReader, Read},
    sync::Arc,
    thread,
};

impl Running {
    pub(super) fn read_loop(self: Arc<Self>, mut stdout: BufReader<Box<dyn Read + Send>>) {
        loop {
            match plugin_sdk::read_frame(&mut stdout) {
                Ok(Some(message)) => self.route(message),
                Ok(None) => {
                    self.fail("plugin closed its control channel");
                    break;
                }
                Err(error) => {
                    self.fail(&format!("plugin protocol error: {error}"));
                    break;
                }
            }
        }
    }

    fn route(self: &Arc<Self>, message: Value) {
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            let Some(id) = message.get("id").cloned() else {
                // Notifications currently have no host-side work.
                return;
            };
            if !is_jsonrpc_2(&message) {
                let reply = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32600, "message": "invalid JSON-RPC version"}
                });
                let _ = self.send_frame(&reply);
                return;
            }
            let method = method.to_string();
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let running = Arc::clone(self);
            let reply_id = id.clone();
            let spawned = thread::Builder::new()
                .name("fiex-plugin-host-call".into())
                .spawn(move || {
                    let result = super::super::dispatch::host_method(&running, &method, &params);
                    let reply = match result {
                        Ok(value) => json!({"jsonrpc":"2.0","id":reply_id,"result":value}),
                        Err(message) => {
                            json!({"jsonrpc":"2.0","id":reply_id,"error":{"code":-32001,"message":message}})
                        }
                    };
                    let _ = running.send_frame(&reply);
                });
            if spawned.is_err() {
                let reply = json!({"jsonrpc":"2.0","id":id,
                    "error":{"code":-32001,"message":"host worker unavailable"}});
                let _ = self.send_frame(&reply);
            }
            return;
        }
        let Some((id, result)) = parse_response(&message) else {
            return;
        };
        if let Ok(mut pending) = self.pending.lock()
            && let Some(sender) = pending.remove(&id)
        {
            let _ = sender.send(result);
        }
    }
}

fn is_jsonrpc_2(message: &Value) -> bool {
    message.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
}

pub(super) fn parse_response(message: &Value) -> Option<(u64, Result<Value, RpcFailure>)> {
    let id = message.get("id")?.as_u64()?;
    let result = if !is_jsonrpc_2(message) {
        Err(RpcFailure::fatal("invalid plugin JSON-RPC version"))
    } else {
        match (message.get("result"), message.get("error")) {
            (Some(value), None) => Ok(value.clone()),
            (None, Some(error)) => {
                let valid_code = error
                    .get("code")
                    .is_some_and(|code| code.as_i64().is_some() || code.as_u64().is_some());
                match (valid_code, error.get("message").and_then(Value::as_str)) {
                    (true, Some(message)) => Err(RpcFailure::business(crate::i18n::text(message))),
                    _ => Err(RpcFailure::fatal("malformed plugin JSON-RPC error")),
                }
            }
            _ => Err(RpcFailure::fatal("invalid plugin response envelope")),
        }
    };
    Some((id, result))
}
