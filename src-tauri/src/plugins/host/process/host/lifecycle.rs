use super::*;

impl PluginHost {
    pub fn stop(&self, id: &str) {
        if let Ok(mut processes) = self.children.lock() {
            if let Some(instance) = processes.remove(id) {
                instance.running.stop();
                instance.kill.terminate();
            }
        }
    }

    /// Stop every resident plugin process. Called on application exit so a
    /// closed Fiex window never leaves orphan plugin children behind.
    pub fn shutdown(&self) {
        if let Ok(mut processes) = self.children.lock() {
            for (_, instance) in processes.drain() {
                instance.running.stop();
                instance.kill.terminate();
            }
        }
    }

    pub fn task(&self, task_id: &str) -> Option<crate::plugins::host::PluginTaskInfo> {
        self.children
            .lock()
            .ok()?
            .values()
            .find_map(|instance| instance.running.task(task_id))
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<(), String> {
        let instance = {
            let guard = self
                .children
                .lock()
                .map_err(|_| "plugin host lock poisoned")?;
            guard
                .values()
                .find(|instance| instance.running.task(task_id).is_some())
                .cloned()
        };
        instance
            .ok_or_else(|| "background task not found".to_string())?
            .running
            .cancel_task(task_id)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        discover()
            .into_iter()
            .find(|(m, _)| m.id == id)
            .is_some_and(|(m, d)| executable(&m, &d).is_ok())
    }
}
