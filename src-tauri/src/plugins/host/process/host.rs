//! Per-plugin child pool and operation-scoped invocation entry points.
use super::super::catalog::{discover, executable};
use super::super::manifest::{activation_requirements, declared};
use super::rpc::Running;
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

struct Instance {
    running: Mutex<Running>,
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
                instance.kill.terminate();
            }
        }
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
    fn invoke_scoped(
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
        let required = activation_requirements(&manifest)?;
        let state = crate::plugin_permissions::status(plugin_id, &required.permissions)?;
        if !state.enabled || !crate::plugin_permissions::activation_complete(&state, &required) {
            return Err("plugin activation requires complete permission consent".into());
        }
        let mut params = params;
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
            let declared = declared(&manifest)?;
            let state = crate::plugin_permissions::status(plugin_id, &declared)?;
            if !state.enabled
                || !crate::plugin_permissions::activation_complete(
                    &state,
                    &activation_requirements(&manifest)?,
                )
            {
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
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            thread::spawn(move || {
                let mut stdout = stdout;
                loop {
                    let frame = plugin_sdk::read_frame(&mut stdout).map_err(|e| e.to_string());
                    let finished = !matches!(frame, Ok(Some(_)));
                    if sender.send(frame).is_err() || finished {
                        break;
                    }
                }
            });
            let running = Running {
                plugin_id: plugin_id.to_string(),
                resource_owner: format!(
                    "{plugin_id}:{}:{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                ),
                declared,
                child: sandbox,
                stdin,
                stdout: receiver,
                next_id: 0,
                task_root: None,
            };
            let instance = Arc::new(Instance {
                running: Mutex::new(running),
                kill,
            });
            guard.insert(plugin_id.to_string(), instance.clone());
            instance
        };
        drop(guard);
        let response = (|| {
            let mut running = instance
                .running
                .lock()
                .map_err(|_| "plugin instance lock poisoned")?;
            let result = (|| {
                if running.next_id == 0 {
                    let result = running.rpc(
                        "plugin.initialize",
                        json!({"protocolVersion":plugin_sdk::PROTOCOL_VERSION}),
                        None,
                    )?;
                    if result.get("pluginId").and_then(Value::as_str) != Some(plugin_id) {
                        return Err("plugin identity mismatch".into());
                    }
                }
                running.rpc(method, params, task_root)
            })();
            // Capabilities and pending writes belong to this operation, never
            // to the lifetime of the pooled plugin process.
            running.task_root = None;
            crate::plugin_fs_access::cleanup(&running.resource_owner);
            crate::plugin_ipc::cleanup(&running.resource_owner);
            result
        })();
        if response.is_err() {
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
        response
    }
}
pub fn global() -> &'static PluginHost {
    static HOST: OnceLock<PluginHost> = OnceLock::new();
    HOST.get_or_init(PluginHost::default)
}
