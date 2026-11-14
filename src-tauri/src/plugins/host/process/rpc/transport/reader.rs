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
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return;
        };
        let result = if let Some(error) = message.get("error") {
            Err(RpcFailure::business(
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .map(crate::i18n::text)
                    .unwrap_or_else(|| format!("plugin error: {error}")),
            ))
        } else {
            message
                .get("result")
                .cloned()
                .ok_or_else(|| RpcFailure::fatal("plugin response missing result"))
        };
        if let Ok(mut pending) = self.pending.lock()
            && let Some(sender) = pending.remove(&id)
        {
            let _ = sender.send(result);
        }
    }
}
