use super::*;

impl PluginHost {
    pub fn invoke(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
        self.invoke_scoped(plugin_id, method, params, None)
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
        bindings: &[crate::plugins::resource::resource_context::ResourceBinding],
    ) -> Result<Value, String> {
        // A VFS selection stays virtual. Bind its Core identity in this
        // invocation instead of substituting a provider's cache path.
        let local_paths = paths
            .iter()
            .filter(|path| {
                let path = path.to_string_lossy();
                !crate::plugins::mounts::handles(&path)
                    && crate::usbfs::route(&path).is_none()
                    && !crate::core::volumes::virtual_resolvers::opaque(&path)
            })
            .cloned()
            .collect::<Vec<_>>();
        for path in paths.iter().filter(|path| {
            let path = path.to_string_lossy();
            crate::plugins::mounts::handles(&path) || crate::usbfs::route(&path).is_some()
        }) {
            let path = path.to_string_lossy();
            if !bindings.iter().any(|binding| binding.path == path) {
                return Err("ScopeDenied: selected VFS resource is not bound".into());
            }
        }
        let root = if local_paths.is_empty() && !paths.is_empty() {
            Some(crate::plugins::resource::fs_access::Scope::selected(
                &std::env::temp_dir(),
                &[],
            )?)
        } else if paths.is_empty() {
            if directory.contains("://") {
                None
            } else {
                Some(crate::plugins::resource::fs_access::Scope::selected(
                    &PathBuf::from(directory),
                    paths,
                )?)
            }
        } else {
            let canonical: Vec<PathBuf> = local_paths
                .iter()
                .map(|path| crate::plugins::resource::fs_access::canonical_selection(path))
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
            Some(crate::plugins::resource::fs_access::Scope::selected(
                &common, &canonical,
            )?)
        };
        let mut root = root;
        if let Some(scope) = &mut root {
            crate::plugins::resource::resource_context::bind_selected(scope, bindings)?;
        }
        self.invoke_scoped(plugin_id, method, params, root)
    }

    /// Unified file-system entry point. Connections and views share `vfs.*`;
    /// the caller decides whether an Operation Scope is attached.
    pub fn invoke_fs(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        scope: Option<crate::plugins::resource::fs_access::Scope>,
    ) -> Result<Value, String> {
        if !method.starts_with("vfs.") {
            return Err("invalid file-system method".into());
        }
        self.invoke_scoped(plugin_id, method, params, scope)
    }

    pub(crate) fn invoke_scoped(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        task_root: Option<crate::plugins::resource::fs_access::Scope>,
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
        if method.starts_with("vfs.")
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
        let state = crate::plugins::security::permissions::status(plugin_id, &declared)?;
        if !state.approved || !state.enabled {
            return Err("plugin is disabled or installation consent is pending".into());
        }
        let mut task_root = task_root;
        let mut params =
            crate::plugins::events::prepare(plugin_id, method, params, &mut task_root, &declared)?;
        if let Some(object) = params.as_object_mut() {
            object.insert("networkDomains".into(), json!(state.network_domains));
        }
        if let Some(scope) = &mut task_root {
            let policy =
                crate::plugins::security::policy::PermissionPolicy::new(plugin_id, &declared);
            crate::plugins::resource::request_resources::project(
                plugin_id,
                &mut params,
                scope,
                policy.allowed(crate::plugins::security::policy::Capability::Metadata),
            )?;
        }
        let mut existing = self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?
            .get(plugin_id)
            .cloned();
        // A crashed/exited plugin leaves a closed handle in the pool. Drop it
        // before the request so the next call starts a fresh process instead
        // of failing once with "plugin process is not running".
        if existing
            .as_ref()
            .is_some_and(|instance| instance.running.is_closed())
        {
            if let Ok(mut guard) = self.children.lock() {
                if guard
                    .get(plugin_id)
                    .is_some_and(|current| Arc::ptr_eq(current, existing.as_ref().unwrap()))
                {
                    guard.remove(plugin_id);
                }
            }
            existing = None;
        }
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
        // The authenticated plugin.initialize handshake has completed by
        // this point. Rebind the selected filesystem implementation using the
        // real Core mount owner and its per-instance capability ceiling.
        if method.starts_with("vfs.") {
            crate::plugins::provider_registry::bind_fs_instance(plugin_id, &mut params)?;
        }
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
