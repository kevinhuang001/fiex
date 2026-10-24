//! Plugin background-task events consumed by the desktop status centre.
//!
//! Core owns the task record and authorization lifetime. The UI only mirrors
//! the events; cancellation goes back through `plugin_cancel_task`.
use serde::Serialize;
use serde_json::Value;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTaskEvent {
    pub task_id: String,
    pub plugin_id: String,
    pub label: String,
    pub status: String,
    pub done: u64,
    pub total: Option<u64>,
    pub result: Value,
    pub error: Option<String>,
}

type Sink = Box<dyn Fn(PluginTaskEvent) + Send + Sync>;
static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();

pub fn set_sink(sink: Sink) {
    let slot = SINK.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(sink);
    }
}

pub fn emit(event: PluginTaskEvent) {
    let Some(slot) = SINK.get() else {
        return;
    };
    if let Ok(slot) = slot.lock() {
        if let Some(sink) = slot.as_ref() {
            sink(event);
        }
    }
}
