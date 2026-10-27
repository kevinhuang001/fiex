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
    /// One startup gate per plugin. Several Git/task RPCs can arrive before
    /// the first process is inserted; without this they all pass the empty
    /// check and spawn a short-lived duplicate process.
    start_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
impl PluginHost {
    fn start_gate(&self, id: &str) -> Result<Arc<Mutex<()>>, String> {
        let mut gates = self
            .start_locks
            .lock()
            .map_err(|_| "plugin startup lock poisoned")?;
        Ok(gates
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

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
    fn spawn_instance(
        plugin_id: &str,
        manifest: &crate::plugin_host::PluginManifest,
        directory: &std::path::Path,
        declared: std::collections::BTreeSet<String>,
    ) -> Result<Arc<Instance>, String> {
        let state = crate::plugin_permissions::status(plugin_id, &declared)?;
        if !state.approved || !state.enabled {
            return Err("plugin is disabled or installation consent is pending".into());
        }
        crate::plugin_signing::verify_installed(manifest, directory)?;
        let path = executable(manifest, directory)?;
        let policy = crate::plugin_sandbox::Policy::new(&path, directory, &state, &declared);
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
            super::rpc::transport::RunningInit {
                plugin_id: plugin_id.to_string(),
                base_owner: format!(
                    "{plugin_id}:{}:{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                ),
                declared,
                commands,
                request_timeout: std::time::Duration::from_millis(
                    crate::plugin_host::request_timeout_ms(&manifest.extensions),
                ),
            },
            sandbox,
            stdin,
            stdout,
        );
        Ok(Arc::new(Instance { running, kill }))
    }
    /// Start the resident backend without waiting for the first request.
    /// Declarative plugins (themes) and packages without a native entrypoint
    /// never reach this path; callers skip them.
    pub fn start(&self, plugin_id: &str) -> Result<(), String> {
        if self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?
            .contains_key(plugin_id)
        {
            return Ok(());
        }
        let gate = self.start_gate(plugin_id)?;
        let _starting = gate.lock().map_err(|_| "plugin startup lock poisoned")?;
        if self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?
            .contains_key(plugin_id)
        {
            return Ok(());
        }
        let (manifest, directory) = discover()
            .into_iter()
            .find(|(manifest, _)| manifest.id == plugin_id)
            .ok_or("plugin not installed")?;
        let declared = declared(&manifest)?;
        // Process creation and signature verification can be slow. Do them
        // outside the host lock so a slow plugin never blocks other providers.
        let instance = Self::spawn_instance(plugin_id, &manifest, &directory, declared)?;
        self.children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?
            .insert(plugin_id.to_string(), instance);
        Ok(())
    }
    /// Start every approved, enabled native plugin after app startup. The
    /// processes are started on a background thread; a broken package is
    /// recorded and skipped instead of blocking the window.
    pub fn start_enabled(&self) {
        for (manifest, directory) in discover() {
            if crate::plugin_themes::is_theme(&manifest)
                || executable(&manifest, &directory).is_err()
            {
                continue;
            }
            let Ok(declared) = declared(&manifest) else {
                continue;
            };
            let Ok(state) = crate::plugin_permissions::status(&manifest.id, &declared) else {
                continue;
            };
            if !state.approved || !state.enabled {
                continue;
            }
            if let Err(error) = self.start(&manifest.id) {
                eprintln!("plugin {} failed to auto-start: {error}", manifest.id);
            }
        }
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
        self.invoke_selected_with_bindings(plugin_id, method, params, directory, paths, &[])
    }
    pub fn invoke_selected_with_bindings(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        directory: &str,
        paths: &[PathBuf],
        bindings: &[crate::plugin_resource_context::ResourceBinding],
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
        let mut root = root;
        if let Some(scope) = &mut root {
            crate::plugin_resource_context::bind_selected(scope, bindings)?;
        }
        self.invoke_scoped(plugin_id, method, params, root)
    }
    /// Unified file-system entry point. Connections and views share `fs.*`;
    /// the caller decides whether an Operation Scope is attached.
    pub fn invoke_fs(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        scope: Option<crate::plugin_fs_access::Scope>,
    ) -> Result<Value, String> {
        if !method.starts_with("fs.") {
            return Err("invalid file-system method".into());
        }
        self.invoke_scoped(plugin_id, method, params, scope)
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
        if method.starts_with("connection.")
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
                .get("connections")
                .is_none_or(|v| !v.is_object())
            && manifest
                .extensions
                .get("vfs")
                .is_none_or(|v| !v.is_object())
        {
            return Err("PermissionDenied: file-system provider declaration required".into());
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
        if let Some(scope) = &mut task_root {
            let policy =
                crate::plugin_permission_policy::PermissionPolicy::new(plugin_id, &declared);
            crate::plugin_request_resources::project(
                plugin_id,
                &mut params,
                scope,
                policy.allowed(crate::plugin_permission_policy::Capability::Metadata),
            )?;
        }
        let existing = self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?
            .get(plugin_id)
            .cloned();
        let instance = if let Some(instance) = existing {
            instance
        } else {
            let gate = self.start_gate(plugin_id)?;
            let _starting = gate.lock().map_err(|_| "plugin startup lock poisoned")?;
            let existing = self
                .children
                .lock()
                .map_err(|_| "plugin host lock poisoned")?
                .get(plugin_id)
                .cloned();
            if let Some(instance) = existing {
                instance
            } else {
                // Never hold the host lock across process creation.
                let created = Self::spawn_instance(plugin_id, &manifest, &dir, declared)?;
                self.children
                    .lock()
                    .map_err(|_| "plugin host lock poisoned")?
                    .insert(plugin_id.to_string(), created.clone());
                created
            }
        };
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
