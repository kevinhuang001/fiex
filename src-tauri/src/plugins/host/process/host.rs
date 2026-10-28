//! Per-plugin child pool and operation-scoped invocation entry points.
use super::super::catalog::{discover, executable};
use super::super::manifest::declared;
use super::rpc::Running;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

struct Instance {
    running: Arc<Running>,
    kill: crate::plugins::security::sandbox::KillHandle,
}

#[derive(Default)]
pub struct PluginHost {
    children: Mutex<HashMap<String, Arc<Instance>>>,
    /// One startup gate per plugin. Several Git/task RPCs can arrive before
    /// the first process is inserted; without this they all pass the empty
    /// check and spawn a short-lived duplicate process.
    start_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
mod invocation;
mod lifecycle;
mod startup;
pub fn global() -> &'static PluginHost {
    static HOST: OnceLock<PluginHost> = OnceLock::new();
    HOST.get_or_init(PluginHost::default)
}
