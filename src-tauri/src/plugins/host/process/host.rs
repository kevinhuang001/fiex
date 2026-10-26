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
    kill: crate::plugin_sandbox::KillHandle,
}

#[derive(Default)]
pub struct PluginHost {
    children: Mutex<HashMap<String, Arc<Instance>>>,
}
impl PluginHost {
    pub fn stop(&self, id: &str) {
        if let Ok(mut processes) = self.children.lock() {
            if let Some(instance) = processes.remove(id) {
                instance.running.stop();
                instance.kill.terminate();
            }
        }
    }
    pub fn task(&self, task_id: &str) -> Option<crate::plugin_host::PluginTaskInfo> {
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
    pub fn invoke(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
        self.invoke_scoped(plugin_id, method, params, None)
    }
    pub fn invoke_at(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        directory: &str,
    ) -> Result<Value, String> {
        let root = if directory.contains("://") {
            None
        } else {
            Some(dunce::canonicalize(directory).map_err(|e| e.to_string())?)
        };
        self.invoke_scoped(
            plugin_id,
            method,
            params,
            root.as_deref()
                .map(crate::plugin_fs_access::Scope::new)
                .transpose()?,
        )
    }
    pub fn invoke_selected(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        directory: &str,
        paths: &[PathBuf],
    ) -> Result<Value, String> {
        if crate::mounts::handles(directory) {
            let (uri, source) = crate::mounts::resolve(directory)?;
            let crate::mounts::Source::View(view) = source else {
                return Err("connection views cannot grant local filesystem access".into());
            };
            if view.descriptor.plugin_id != plugin_id {
                return Err("PermissionDenied: foreign VFS owner".into());
            }
            for path in paths {
                let selected = path.to_string_lossy();
                if crate::mounts::handles(&selected) {
                    let selected = crate::mounts::parse(&selected)?;
                    if selected.mount_id != uri.mount_id {
                        return Err("PermissionDenied: foreign mount".into());
                    }
                } else {
                    view.scope.authorize(&selected)?;
                }
            }
            return self.invoke_scoped(plugin_id, method, params, Some(view.scope));
        }
        let root = if paths.is_empty() {
            if directory.contains("://") {
                None
            } else {
                Some(crate::plugin_fs_access::Scope::selected(
                    &PathBuf::from(directory),
                    paths,
                )?)
            }
        } else {
            let canonical: Vec<PathBuf> = paths
                .iter()
                .map(|path| crate::plugin_fs_access::canonical_selection(path))
                .collect::<Result<_, _>>()?;
            let current = dunce::canonicalize(directory)
                .ok()
                .filter(|path| path.is_dir());
            let mut common = current
                .filter(|root| canonical.iter().all(|path| path.starts_with(root)))
                .unwrap_or(
                    canonical[0]
                        .parent()
                        .ok_or("selection has no parent")?
                        .to_path_buf(),
                );
            for path in &canonical[1..] {
                while !path.starts_with(&common) {
                    common = common
                        .parent()
                        .ok_or("selection has no common root")?
                        .to_path_buf();
                }
            }
            Some(crate::plugin_fs_access::Scope::selected(
                &common, &canonical,
            )?)
        };
        self.invoke_scoped(plugin_id, method, params, root)
    }
    pub(crate) fn invoke_vfs(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        scope: crate::plugin_fs_access::Scope,
    ) -> Result<Value, String> {
        if !method.starts_with("vfs.") {
            return Err("invalid VFS method".into());
        }
        self.invoke_scoped(plugin_id, method, params, Some(scope))
    }
    pub(crate) fn invoke_scoped(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        task_root: Option<crate::plugin_fs_access::Scope>,
    ) -> Result<Value, String> {
        let (manifest, dir) = discover()
            .into_iter()
            .find(|(m, _)| m.id == plugin_id)
            .ok_or("plugin not installed")?;
        if method.starts_with("vfs.") {
            if manifest
                .extensions
                .get("vfs")
                .is_none_or(|v| !v.is_object())
            {
                return Err("PermissionDenied: VFS provider declaration required".into());
            }
            if task_root.is_none() {
                return Err("VFS requests require a Core-bound view scope".into());
            }
        }
        if (method.starts_with("fs.") || method.starts_with("connection."))
            && manifest
                .extensions
                .get("connections")
                .is_none_or(|v| !v.is_object())
        {
            return Err("PermissionDenied: connection factory declaration required".into());
        }
        if method.starts_with("fs.")
            && manifest
                .extensions
                .get("vfs")
                .is_none_or(|v| !v.is_object())
        {
            return Err("PermissionDenied: VFS provider declaration required".into());
        }
        let declared = declared(&manifest)?;
        let state = crate::plugin_permissions::status(plugin_id, &declared)?;
        if !state.approved || !state.enabled {
            return Err("plugin is disabled or installation consent is pending".into());
        }
        let mut task_root = task_root;
        let mut params =
            crate::plugin_events::prepare(plugin_id, method, params, &mut task_root, &declared)?;
        if let Some(object) = params.as_object_mut() {
            object.insert("networkDomains".into(), json!(state.network_domains));
        }
        let mut guard = self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?;
        let instance = if let Some(instance) = guard.get(plugin_id) {
            instance.clone()
        } else {
            crate::plugin_signing::verify_installed(&manifest, &dir)?;
            let state = crate::plugin_permissions::status(plugin_id, &declared)?;
            if !state.approved || !state.enabled {
                return Err("plugin is disabled or installation consent is pending".into());
            }
            let path = executable(&manifest, &dir)?;
            let policy = crate::plugin_sandbox::Policy::new(&path, &dir, &state, &declared);
            let mut sandbox = crate::plugin_sandbox::spawn(&policy)?;
            let stdin = std::mem::replace(
                &mut sandbox.stdin,
                Box::new(std::io::sink()) as Box<dyn Write + Send>,
            );
            let stdout = std::mem::replace(
                &mut sandbox.stdout,
                BufReader::new(Box::new(std::io::empty()) as Box<dyn Read + Send>),
            );
            let kill = sandbox.kill_handle();
            let commands = manifest
                .permissions
                .commands
                .allow
                .iter()
                .map(|command| command.to_ascii_lowercase())
                .collect();
            let running = Running::start(
                plugin_id.to_string(),
                format!(
                    "{plugin_id}:{}:{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                ),
                declared,
                commands,
                sandbox,
                stdin,
                stdout,
            );
            let instance = Arc::new(Instance { running, kill });
            guard.insert(plugin_id.to_string(), instance.clone());
            instance
        };
        drop(guard);
        match instance.running.rpc(method, params, task_root) {
            Ok(value) => Ok(value),
            Err(failure) => {
                if failure.fatal {
                    instance.running.stop();
                    instance.kill.terminate();
                    if let Ok(mut guard) = self.children.lock() {
                        if guard
                            .get(plugin_id)
                            .is_some_and(|current| Arc::ptr_eq(current, &instance))
                        {
                            guard.remove(plugin_id);
                        }
                    }
                }
                Err(failure.message)
            }
        }
    }
}
pub fn global() -> &'static PluginHost {
    static HOST: OnceLock<PluginHost> = OnceLock::new();
    HOST.get_or_init(PluginHost::default)
}
