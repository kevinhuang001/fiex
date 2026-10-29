use super::super::{
    tasks::{PluginTaskInfo, MAX_TASKS},
    Running, TaskEntry,
};
use serde_json::Value;

use super::run;

/// Task-scoped methods are authenticated by the Running process identity and
/// do not need the operation target resolver.
pub(super) fn host_method(running: &Running, method: &str, p: &Value) -> Result<Value, String> {
    let arg = |key: &str| {
        p.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("missing {key}"))
    };
    match method {
        "host.task.start" => {
            let operation = running.operation_actor(p)?;
            let task_id = arg("taskId")?;
            if task_id.is_empty()
                || task_id.len() > 128
                || !task_id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            {
                return Err("invalid task id".into());
            }
            let label = p
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.is_empty())
                .unwrap_or(crate::i18n::t("插件后台任务"));
            let mut tasks = running
                .tasks
                .lock()
                .map_err(|_| "task registry unavailable")?;
            if tasks.len() >= MAX_TASKS {
                return Err("too many background tasks".into());
            }
            if tasks.contains_key(task_id) {
                return Err("task id already registered".into());
            }
            let info = PluginTaskInfo {
                task_id: task_id.to_string(),
                plugin_id: running.plugin_id.clone(),
                label: label.to_string(),
                status: "running".into(),
                done: 0,
                total: None,
                result: Value::Null,
                error: None,
                internal: p.get("internal").and_then(Value::as_bool).unwrap_or(false),
            };
            tasks.insert(
                task_id.to_string(),
                TaskEntry {
                    owner: format!("{}:task:{task_id}", running.base_owner),
                    scope: operation.scope,
                    info: info.clone(),
                },
            );
            drop(tasks);
            running.emit_task(&info);
            crate::plugins::errors::report_log_limited(running.plugin_id(), "info", &format!("Background task started: {label}"));
            Ok(Value::Null)
        }
        "host.task.finish" => {
            let authorization = p.get("authorization").ok_or("missing task authorization")?;
            let task_id = authorization
                .get("taskId")
                .and_then(Value::as_str)
                .ok_or("missing task authorization")?;
            let finished = running
                .tasks
                .lock()
                .map_err(|_| "task registry unavailable")?
                .remove(task_id)
                .map(|mut entry| {
                    entry.info.status = match p.get("status").and_then(Value::as_str) {
                        Some("completed") => "completed",
                        Some("cancelled") => "cancelled",
                        _ => "failed",
                    }
                    .to_string();
                    entry.info.result = p.get("result").cloned().unwrap_or(Value::Null);
                    entry.info.error = p.get("error").and_then(Value::as_str).map(str::to_owned);
                    entry
                });
            let Some(entry) = finished else {
                return Ok(Value::Null);
            };
            crate::plugins::resource::lifecycle::release(&entry.owner);
            running.emit_task(&entry.info);
            crate::plugins::errors::report_log_limited(running.plugin_id(), if entry.info.status == "failed" { "error" } else { "info" }, &format!("Background task {}: {}", entry.info.status, entry.info.label));
            running.remember_task(entry.info);
            Ok(Value::Null)
        }
        "host.task.progress" => {
            let authorization = p.get("authorization").ok_or("missing task authorization")?;
            let task_id = authorization
                .get("taskId")
                .and_then(Value::as_str)
                .ok_or("missing task authorization")?;
            let info = {
                let mut tasks = running
                    .tasks
                    .lock()
                    .map_err(|_| "task registry unavailable")?;
                let Some(entry) = tasks.get_mut(task_id) else {
                    return Err("task authorization expired".into());
                };
                if !matches!(entry.info.status.as_str(), "running" | "cancelling") {
                    return Ok(Value::Null);
                }
                entry.info.done = p.get("done").and_then(Value::as_u64).unwrap_or(0);
                entry.info.total = p.get("total").and_then(Value::as_u64);
                entry.info.clone()
            };
            running.emit_task(&info);
            Ok(Value::Null)
        }
        "host.task.cancel" => {
            let authorization = p.get("authorization").ok_or("missing task authorization")?;
            let task_id = authorization
                .get("taskId")
                .and_then(Value::as_str)
                .ok_or("missing task authorization")?;
            if !running
                .tasks
                .lock()
                .map_err(|_| "task registry unavailable")?
                .contains_key(task_id)
            {
                return Err("task authorization expired".into());
            }
            Ok(Value::Null)
        }
        _ => run(running, method, p),
    }
}
