use super::{actor::Operation, RpcFailure, Running};
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    io::{BufReader, Read, Write},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

/// Identity and budgets captured when a resident plugin process starts.
pub(in crate::plugin_host::process) struct RunningInit {
    pub(in crate::plugin_host::process) plugin_id: String,
    pub(in crate::plugin_host::process) base_owner: String,
    pub(in crate::plugin_host::process) declared: BTreeSet<String>,
    pub(in crate::plugin_host::process) commands: BTreeSet<String>,
    pub(in crate::plugin_host::process) request_timeout: Duration,
}

impl Running {
    pub(in crate::plugin_host::process) fn start(
        init: RunningInit,
        child: crate::plugin_sandbox::SandboxChild,
        stdin: Box<dyn Write + Send>,
        stdout: BufReader<Box<dyn Read + Send>>,
    ) -> Arc<Self> {
        let RunningInit {
            plugin_id,
            base_owner,
            declared,
            commands,
            request_timeout,
        } = init;
        let running = Arc::new(Self {
            plugin_id,
            base_owner,
            declared,
            commands,
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            operations: Mutex::new(HashMap::new()),
            tasks: Mutex::new(HashMap::new()),
            history: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(0),
            initialized: AtomicBool::new(false),
            init_lock: Mutex::new(()),
            closed: AtomicBool::new(false),
            request_timeout,
        });
        let reader_running = Arc::clone(&running);
        thread::Builder::new()
            .name("fiex-plugin-reader".into())
            .spawn(move || reader_running.read_loop(stdout))
            .expect("plugin reader thread");
        running
    }

    fn read_loop(self: Arc<Self>, mut stdout: BufReader<Box<dyn Read + Send>>) {
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
                    let result = super::dispatch::host_method(&running, &method, &params);
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
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(sender) = pending.remove(&id) {
                let _ = sender.send(result);
            }
        }
    }

    pub(super) fn send_frame(&self, message: &Value) -> Result<(), String> {
        let mut writer = self
            .stdin
            .lock()
            .map_err(|_| "plugin input lock poisoned")?;
        plugin_sdk::write_frame(&mut *writer, message).map_err(|error| error.to_string())
    }

    pub(super) fn fail(&self, message: &str) {
        self.closed.store(true, Ordering::Release);
        crate::plugin_permission_prompt::cancel_plugin(&self.plugin_id);
        let diagnostic = self.exit_diagnostic();
        let message = format!("{message} ({diagnostic})");
        if let Ok(mut pending) = self.pending.lock() {
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(RpcFailure::fatal(message.clone())));
            }
        }
    }
}

impl Running {
    fn ensure_initialized(&self) -> Result<(), RpcFailure> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self
            .init_lock
            .lock()
            .map_err(|_| RpcFailure::fatal("plugin initialization lock poisoned"))?;
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let result = self
            .rpc(
                "plugin.initialize",
                json!({"protocolVersion": plugin_sdk::PROTOCOL_VERSION}),
                None,
            )
            .map_err(|failure| RpcFailure::fatal(failure.message))?;
        if result.get("pluginId").and_then(Value::as_str) != Some(self.plugin_id.as_str()) {
            return Err(RpcFailure::fatal("plugin identity mismatch"));
        }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub(in crate::plugin_host::process) fn rpc(
        &self,
        method: &str,
        params: Value,
        scope: Option<crate::plugin_fs_access::Scope>,
    ) -> Result<Value, RpcFailure> {
        let locale = crate::i18n::locale();
        if self.closed.load(Ordering::Acquire) {
            return Err(RpcFailure::fatal("plugin process is not running"));
        }
        if method != "plugin.initialize" {
            self.ensure_initialized()?;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let owner = format!("{}:op:{id}", self.base_owner);
        self.operations
            .lock()
            .map_err(|_| RpcFailure::fatal("operation registry unavailable"))?
            .insert(
                id,
                Operation {
                    owner: owner.clone(),
                    scope,
                },
            );
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| RpcFailure::fatal("response registry unavailable"))?
            .insert(id, sender);
        let sent = self.send_frame(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
            "locale": locale,
        }));
        if let Err(error) = sent {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            self.finish_operation(id);
            return Err(RpcFailure::fatal(error));
        }
        let result = match receiver.recv_timeout(if method == "fs.writeFile" {
            Duration::from_secs(86_520)
        } else {
            self.request_timeout
        }) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(RpcFailure::fatal("plugin request deadline exceeded"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(RpcFailure::fatal("plugin process disconnected"))
            }
        };
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
        self.finish_operation(id);
        result
    }

    fn finish_operation(&self, id: u64) {
        let operation = self
            .operations
            .lock()
            .ok()
            .and_then(|mut operations| operations.remove(&id));
        if let Some(operation) = operation {
            crate::plugin_fs_access::cleanup(&operation.owner);
            crate::plugin_ipc::cleanup(&operation.owner);
            crate::plugin_net::cleanup(&operation.owner);
        }
    }
}
