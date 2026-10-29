use super::*;

impl PluginHost {
    pub(super) fn start_gate(&self, id: &str) -> Result<Arc<Mutex<()>>, String> {
        let mut gates = self
            .start_locks
            .lock()
            .map_err(|_| "plugin startup lock poisoned")?;
        Ok(gates
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

    pub(super) fn spawn_instance(
        plugin_id: &str,
        manifest: &crate::plugins::host::PluginManifest,
        directory: &std::path::Path,
        declared: std::collections::BTreeSet<String>,
    ) -> Result<Arc<Instance>, String> {
        let state = crate::plugins::security::permissions::status(plugin_id, &declared)?;
        if !state.approved || !state.enabled {
            return Err("plugin is disabled or installation consent is pending".into());
        }
        crate::plugins::signing::verify_installed(manifest, directory)?;
        let path = executable(manifest, directory)?;
        let policy =
            crate::plugins::security::sandbox::Policy::new(&path, directory, &state, &declared);
        let mut sandbox = crate::plugins::security::sandbox::spawn(&policy)?;
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
            super::super::rpc::transport::RunningInit {
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
                    crate::plugins::host::request_timeout_ms(&manifest.extensions),
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
        crate::infrastructure::logging::plugin(
            plugin_id, "info", "host", Some("plugin.start"),
            "Plugin process started", serde_json::Value::Null,
        );
        // Handshake eagerly so the capability registry is populated before the
        // first listing or picker asks which operations this provider supports.
        let _ = instance.running.ensure_initialized();
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
            if crate::plugins::themes::is_theme(&manifest)
                || executable(&manifest, &directory).is_err()
            {
                continue;
            }
            let Ok(declared) = declared(&manifest) else {
                continue;
            };
            let Ok(state) = crate::plugins::security::permissions::status(&manifest.id, &declared)
            else {
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
}
