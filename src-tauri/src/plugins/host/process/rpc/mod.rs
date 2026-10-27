//! Plugin process JSON-RPC state shared by transport, tasks, permission guards
//! and host-method dispatch. The actual behavior lives in child modules; this
//! file only owns the state types so every module stays focused.
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    io::Write,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        mpsc, Mutex,
    },
    time::Duration,
};

mod actor;
mod dispatch;
mod permissions;
mod tasks;
pub(super) mod transport;

pub use tasks::PluginTaskInfo;

/// A plugin call failure. Business errors (a valid JSON-RPC error reply) must
/// not terminate the resident process; transport and protocol failures must.
#[derive(Debug)]
pub(super) struct RpcFailure {
    pub(super) message: String,
    pub(super) fatal: bool,
}

impl RpcFailure {
    pub(super) fn business(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: false,
        }
    }

    pub(super) fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: true,
        }
    }
}

#[derive(Clone)]
pub(super) struct TaskEntry {
    pub(super) owner: String,
    pub(super) scope: Option<crate::plugins::resource::fs_access::Scope>,
    pub(super) info: PluginTaskInfo,
}

pub(super) struct Running {
    plugin_id: String,
    base_owner: String,
    declared: BTreeSet<String>,
    /// Executable names approved by this package's manifest. A runtime grant
    /// can never widen this list.
    commands: BTreeSet<String>,
    child: Mutex<crate::plugins::security::sandbox::SandboxChild>,
    stdin: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, RpcFailure>>>>,
    operations: Mutex<HashMap<u64, actor::Operation>>,
    tasks: Mutex<HashMap<String, TaskEntry>>,
    history: Mutex<VecDeque<PluginTaskInfo>>,
    next_id: AtomicU64,
    initialized: AtomicBool,
    init_lock: Mutex<()>,
    closed: AtomicBool,
    /// Per-plugin budget from `extensions.ui.timeoutMs`; slow providers such
    /// as archive listing can exceed the default without a global timeout.
    request_timeout: Duration,
}

impl Running {
    pub(super) fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
}
