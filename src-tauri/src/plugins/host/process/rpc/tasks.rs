use super::{actor::Operation, Running, TaskEntry};
use serde_json::{json, Value};

pub(super) const MAX_TASKS: usize = 32;
const MAX_TASK_HISTORY: usize = 64;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTaskInfo {
    pub task_id: String,
    pub plugin_id: String,
    pub label: String,
    pub status: String,
    pub done: u64,
    pub total: Option<u64>,
    pub result: Value,
    pub error: Option<String>,
    pub internal: bool,
}

impl Running {
    pub(in crate::plugin_host::process) fn stop(&self) {
        self.fail("plugin process stopped");
        let operations: Vec<Operation> = self
            .operations
            .lock()
            .map(|mut operations| operations.drain().map(|(_, value)| value).collect())
            .unwrap_or_default();
        for operation in operations {
            crate::plugin_fs_access::cleanup(&operation.owner);
            crate::plugin_ipc::cleanup(&operation.owner);
            crate::plugin_net::cleanup(&operation.owner);
        }
        let tasks: Vec<TaskEntry> = self
            .tasks
            .lock()
            .map(|mut tasks| tasks.drain().map(|(_, value)| value).collect())
            .unwrap_or_default();
        for task in tasks {
            let mut info = task.info;
            info.status = "cancelled".into();
            info.error = Some("插件进程已停止".into());
            self.emit_task(&info);
            crate::plugin_fs_access::cleanup(&task.owner);
            crate::plugin_ipc::cleanup(&task.owner);
            crate::plugin_net::cleanup(&task.owner);
        }
    }

    pub(super) fn emit_task(&self, info: &PluginTaskInfo) {
        crate::plugin_task_events::emit(crate::plugin_task_events::PluginTaskEvent {
            task_id: info.task_id.clone(),
            plugin_id: info.plugin_id.clone(),
            label: info.label.clone(),
            status: info.status.clone(),
            done: info.done,
            total: info.total,
            result: info.result.clone(),
            error: info.error.clone(),
            internal: info.internal,
        });
    }

    pub(in crate::plugin_host::process) fn task(&self, id: &str) -> Option<PluginTaskInfo> {
        if let Some(info) = self
            .tasks
            .lock()
            .ok()
            .and_then(|tasks| tasks.get(id).map(|task| task.info.clone()))
        {
            return Some(info);
        }
        self.history
            .lock()
            .ok()
            .and_then(|history| history.iter().find(|info| info.task_id == id).cloned())
    }

    pub(in crate::plugin_host::process) fn cancel_task(&self, id: &str) -> Result<(), String> {
        let info = {
            let mut tasks = self.tasks.lock().map_err(|_| "task registry unavailable")?;
            let entry = tasks.get_mut(id).ok_or("background task not found")?;
            if !matches!(entry.info.status.as_str(), "running" | "cancelling") {
                return Err("background task has already finished".into());
            }
            entry.info.status = "cancelling".into();
            entry.info.clone()
        };
        self.emit_task(&info);
        if let Err(error) = self.send_frame(&json!({
            "jsonrpc": "2.0",
            "method": "plugin.cancel",
            "params": { "taskId": info.task_id },
        })) {
            if let Some(mut entry) = self
                .tasks
                .lock()
                .ok()
                .and_then(|mut tasks| tasks.remove(id))
            {
                entry.info.status = "failed".into();
                entry.info.error = Some("插件进程已断开".into());
                crate::plugin_fs_access::cleanup(&entry.owner);
                crate::plugin_ipc::cleanup(&entry.owner);
                crate::plugin_net::cleanup(&entry.owner);
                self.emit_task(&entry.info);
                self.remember_task(entry.info);
            }
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn remember_task(&self, info: PluginTaskInfo) {
        if let Ok(mut history) = self.history.lock() {
            history.push_back(info);
            while history.len() > MAX_TASK_HISTORY {
                history.pop_front();
            }
        }
    }

    pub(super) fn exit_diagnostic(&self) -> String {
        self.child
            .lock()
            .map(|mut child| child.exit_diagnostic())
            .unwrap_or_else(|_| "plugin process state unavailable".into())
    }
}
