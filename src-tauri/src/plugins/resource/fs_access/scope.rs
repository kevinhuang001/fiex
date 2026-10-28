/// A save dialog may select a file that does not exist yet. Canonicalize its
/// existing parent so a symlinked parent cannot change the selected target.
use super::*;

pub fn canonical_selection(path: &Path) -> Result<PathBuf, String> {
    if path.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("selection must not contain parent traversal".into());
    }
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match dunce::canonicalize(ancestor) {
            Ok(base) => {
                return Ok(missing.into_iter().rev().fold(base, |base: PathBuf, name| {
                    base.join(name)
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor.file_name().ok_or("selection has no name")?.to_os_string(),
                );
                ancestor = ancestor.parent().ok_or("selection has no parent")?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Captured once at invocation start, including an open directory capability.
#[derive(Clone)]
pub struct Scope {
    pub(super) path: PathBuf,
    pub(super) dir: std::sync::Arc<Dir>,
    pub(super) allowed: Option<Vec<(PathBuf, bool)>>,
    pub(super) references: std::sync::Arc<HashMap<String, String>>,
    pub(super) metadata_refs: std::sync::Arc<HashMap<String, Value>>,
    pub(super) metadata_only: bool,
    pub(super) read_only: bool,
    pub(super) issued: std::sync::Arc<
        Mutex<HashMap<String, crate::plugins::resource::resource_context::ResourceSnapshot>>,
    >,
    pub(super) remote_resources: std::sync::Arc<
        HashMap<String, crate::plugins::resource::resource_context::ResourceSnapshot>,
    >,
}
impl Scope {
    pub fn new(path: &Path) -> Result<Self, String> {
        let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            dir: std::sync::Arc::new(dir),
            allowed: None,
            references: Default::default(),
            metadata_refs: Default::default(),
            metadata_only: false,
            read_only: false,
            issued: Default::default(),
            remote_resources: Default::default(),
        })
    }
    /// Capture only paths chosen for this operation. A file grants that file;
    /// a directory grants its descendants. An empty selection grants no files.
    pub fn selected(path: &Path, paths: &[PathBuf]) -> Result<Self, String> {
        let mut scope = Self::new(path)?;
        let mut allowed = Vec::with_capacity(paths.len());
        for selected in paths {
            let canonical = canonical_selection(selected)?;
            let relative = canonical
                .strip_prefix(&scope.path)
                .map_err(|_| "selection outside task directory")?
                .to_path_buf();
            let relative = if relative.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                relative
            };
            let is_dir = scope.dir.metadata(&relative).is_ok_and(|m| m.is_dir());
            allowed.push((relative, is_dir));
        }
        scope.allowed = Some(allowed);
        Ok(scope)
    }
    pub fn restrict_to_metadata(&mut self) {
        self.metadata_only = true;
    }
    pub fn restrict_to_readonly(&mut self) {
        self.read_only = true;
    }
    pub fn authorize_method(&self, method: &str) -> Result<(), String> {
        if self.read_only
            && matches!(
                method,
                "fs.mkdir"
                    | "fs.seekWrite"
                    | "fs.beginWrite"
                    | "fs.writeChunk"
                    | "fs.writeBuffer"
                    | "fs.commit"
                    | "fs.remove"
                    | "fs.rename"
                    | "host.process.run"
                    | "host.vfs.openView"
            )
        {
            return Err("PermissionDenied: view does not support writes".into());
        }
        if self.metadata_only
            && ((method.starts_with("fs.") && !matches!(method, "fs.stat" | "fs.statBatch"))
                || matches!(method, "host.process.run" | "host.vfs.openView"))
        {
            return Err("discovery scope permits metadata only".into());
        }
        Ok(())
    }
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.allowed
            .as_ref()
            .map(|paths| paths.iter().map(|(p, _)| self.path.join(p)).collect())
            .unwrap_or_default()
    }
    fn ref_key(reference: &Value) -> Result<String, String> {
        let reference: crate::plugins::contracts::ResourceRef =
            serde_json::from_value(reference.clone()).map_err(|e| e.to_string())?;
        serde_json::to_string(&reference).map_err(|e| e.to_string())
    }
    pub fn resource_bound(&self, reference: &Value) -> Result<bool, String> {
        let key = Self::ref_key(reference)?;
        Ok(self.references.contains_key(&key)
            || self.metadata_refs.contains_key(&key)
            || self.remote_resources.contains_key(&key)
            || self
                .issued
                .lock()
                .map_err(|_| "reference registry unavailable")?
                .contains_key(&key))
    }
    pub fn bind_resource(&mut self, resource: &Value, path: Option<&Path>) -> Result<(), String> {
        let key = Self::ref_key(&resource["ref"])?;
        if let Some(path) = path {
            let path = canonical_selection(path)?.to_string_lossy().into_owned();
            self.authorize(&path)?;
            std::sync::Arc::make_mut(&mut self.references).insert(key.clone(), path.clone());
        }
        if let Some(metadata) = resource.get("metadata").filter(|v| v.is_object()) {
            let mut metadata = metadata.clone();
            metadata["isDir"] = json!(metadata["kind"] == "directory");
            metadata["path"] = resource["ref"]["nodeId"].clone();
            for (key, value) in [
                ("size", json!(0)),
                ("modified", json!(0)),
                ("extension", json!("")),
                ("mime", json!("")),
                ("readonly", json!(false)),
                ("symlink", json!(false)),
            ] {
                if metadata.get(key).is_none_or(Value::is_null) {
                    metadata[key] = value;
                }
            }
            std::sync::Arc::make_mut(&mut self.metadata_refs).insert(key, metadata);
        }
        Ok(())
    }
    pub fn bind_remote(
        &mut self,
        resource: &Value,
        snapshot: &crate::plugins::resource::resource_context::ResourceSnapshot,
    ) -> Result<(), String> {
        std::sync::Arc::make_mut(&mut self.remote_resources)
            .insert(Self::ref_key(&resource["ref"])?, snapshot.clone());
        Ok(())
    }
    pub fn remote_target(
        &self,
        p: &Value,
    ) -> Result<Option<crate::plugins::resource::resource_context::ResourceSnapshot>, String> {
        if let Some(reference) = p.get("ref") {
            let key = Self::ref_key(reference)?;
            return Ok(self.remote_resources.get(&key).cloned().or_else(|| {
                self.issued
                    .lock()
                    .ok()?
                    .get(&key)
                    .filter(|snapshot| {
                        crate::plugins::mounts::handles(&snapshot.display)
                            || crate::usbfs::route(&snapshot.display).is_some()
                            || crate::core::volumes::virtual_resolvers::resolver_for(
                                &snapshot.display,
                            )
                            .is_some()
                    })
                    .cloned()
            }));
        }
        Ok(None)
    }
    /// Resolve only identities already bound to this exact operation. A child
    /// cannot use an arbitrary parent path to acquire a different mount/root.
    pub fn reference_for_path(&self, path: &str) -> Result<Value, String> {
        if let Some((key, _)) = self.references.iter().find(|(_, p)| p.as_str() == path) {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        if let Some((key, _)) = self
            .remote_resources
            .iter()
            .find(|(_, r)| r.display == path)
        {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        let issued = self
            .issued
            .lock()
            .map_err(|_| "reference registry unavailable")?;
        if let Some((key, _)) = issued.iter().find(|(_, r)| r.display == path) {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        Err("ScopeDenied: parent directory is not bound".into())
    }
    pub fn metadata_reference(&self, reference: &Value) -> Result<Option<&Value>, String> {
        Ok(self.metadata_refs.get(&Self::ref_key(reference)?))
    }
    pub fn resolve_reference(&self, reference: &Value) -> Result<String, String> {
        let key = Self::ref_key(reference)?;
        self.references
            .get(&key)
            .cloned()
            .or_else(|| {
                self.issued
                    .lock()
                    .ok()?
                    .get(&key)
                    .map(|snapshot| snapshot.display.clone())
            })
            .ok_or_else(|| "resource not authorized in this operation".into())
    }
    /// Mint a child identity into the shared operation registry. Cloning a
    /// Scope into a task preserves issued refs but never widens its selection.
    pub fn issue_child(
        &self,
        parent: &Value,
        display: &str,
        metadata: Option<crate::plugins::contracts::ResourceMetadata>,
    ) -> Result<Value, String> {
        let parent_path = self.resolve_reference(parent).or_else(|_| {
            self.remote_target(&json!({"ref":parent}))?
                .map(|snapshot| snapshot.display)
                .ok_or_else(|| "parent ref is not authorized".to_string())
        })?;
        if crate::plugins::mounts::handles(&parent_path) {
            let root = crate::plugins::mounts::parse(&parent_path)?;
            let child = crate::plugins::mounts::parse(display)?;
            if root.mount_id != child.mount_id
                || !Path::new(&child.relative).starts_with(&root.relative)
            {
                return Err("child outside selected resource".into());
            }
        } else if parent_path == crate::core::volumes::virtual_resolvers::RECENT_URI {
            let _ = crate::core::recent::source_for(display)?;
        } else if parent_path == crate::core::volumes::applications::URI
            || crate::core::volumes::applications::is_user(&parent_path)
        {
            let parent_local = if parent_path == crate::core::volumes::applications::URI {
                crate::core::volumes::applications::stored_path(&parent_path)?
            } else {
                crate::core::volumes::applications::candidate_path(&parent_path)?
            };
            let child_local = crate::core::volumes::applications::candidate_path(display)?;
            if child_local.parent() != Some(parent_local.as_path()) {
                return Err("ScopeDenied: child outside selected application directory".into());
            }
        } else if let Some(root) = crate::usbfs::route(&parent_path) {
            let child = crate::usbfs::route(display).ok_or("invalid USB child")?;
            if root.root != child.root
                || !Path::new(&child.rel).starts_with(&root.rel)
                || Path::new(&child.rel)
                    .components()
                    .any(|p| !matches!(p, Component::Normal(_)))
            {
                return Err("child outside selected USB resource".into());
            }
        } else {
            self.authorize(display)?;
            if !Path::new(display).starts_with(&parent_path) {
                return Err("child outside selected resource".into());
            }
        }
        let meta =
            metadata
                .clone()
                .unwrap_or_else(|| crate::plugins::contracts::ResourceMetadata {
                    name: Path::new(display)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    kind: "file".into(),
                    ..Default::default()
                });
        let mut snapshot =
            crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
                display, meta,
            );
        if parent["mountId"] == "core:applications" {
            snapshot.local = crate::core::volumes::applications::candidate_path(display)?;
        }
        let reference =
            serde_json::to_value(&snapshot.reference).map_err(|error| error.to_string())?;
        self.issued
            .lock()
            .map_err(|_| "reference registry unavailable")?
            .insert(Self::ref_key(&reference)?, snapshot);
        Ok(json!({"ref":reference,"metadata":metadata}))
    }
    pub fn authorize(&self, path: &str) -> Result<(), String> {
        relative(self, path).map(|_| ())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
