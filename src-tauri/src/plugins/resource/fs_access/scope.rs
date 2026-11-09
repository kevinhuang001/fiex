/// A save dialog may select a file that does not exist yet. Canonicalize its
/// existing parent so a symlinked parent cannot change the selected target.
use super::*;
use fiex_provider_contracts::FsOperation;
use fiex_scope_policy::{method_denial, MethodDenial};

#[derive(Clone)]
struct MappedResourceBinding {
    snapshot: crate::plugins::resource::resource_context::ResourceSnapshot,
    path: String,
    operation: FsOperation,
    storage_scope: Scope,
}

pub fn canonical_selection(path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("selection must not contain parent traversal".into());
    }
    let mut ancestor = path;
    let mut missing = Vec::new();
    loop {
        match dunce::canonicalize(ancestor) {
            Ok(base) => {
                return Ok(missing
                    .into_iter()
                    .rev()
                    .fold(base, |base: PathBuf, name| base.join(name)));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or("selection has no name")?
                        .to_os_string(),
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
    user_action: Option<crate::core::fs::mapped_access::UserActionContext>,
    mapped_resources: std::sync::Arc<Mutex<HashMap<String, MappedResourceBinding>>>,
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
            user_action: None,
            mapped_resources: Default::default(),
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
    pub(crate) fn set_user_action_context(
        &mut self,
        user_action: crate::core::fs::mapped_access::UserActionContext,
    ) {
        self.user_action = Some(user_action);
    }
    pub(crate) fn user_action_context(
        &self,
    ) -> Option<crate::core::fs::mapped_access::UserActionContext> {
        self.user_action
    }
    pub(crate) fn without_user_action(mut self) -> Self {
        self.user_action = None;
        self.mapped_resources = Default::default();
        self
    }
    pub(crate) fn bind_mapped_resource(
        &self,
        snapshot: crate::plugins::resource::resource_context::ResourceSnapshot,
        operation: FsOperation,
    ) -> Result<(), String> {
        let reference = serde_json::to_value(&snapshot.reference).map_err(|e| e.to_string())?;
        let access = crate::core::fs::provider_registry::resource_access(&snapshot.display)
            .map_err(String::from)?;
        let mut storage_scope =
            if access == crate::core::fs::provider_registry::ResourceAccess::HostPath {
                let path = Path::new(&snapshot.display);
                let parent = path.parent().unwrap_or(path);
                Scope::selected(parent, &[path.to_path_buf()])?
            } else {
                Scope::selected(&std::env::temp_dir(), &[])?
            };
        if let Some(user_action) = self.user_action_context() {
            storage_scope.set_user_action_context(user_action);
        }
        let resource = json!({"ref": snapshot.reference, "metadata": snapshot.metadata()});
        if access == crate::core::fs::provider_registry::ResourceAccess::HostPath {
            storage_scope.bind_resource(&resource, Some(Path::new(&snapshot.display)))?;
        } else {
            storage_scope.bind_remote(&resource, &snapshot)?;
        }
        let path = storage_scope
            .resolve_reference(&reference)
            .unwrap_or_else(|_| snapshot.display.clone());
        self.mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?
            .insert(
                Self::ref_key(&reference)?,
                MappedResourceBinding {
                    snapshot,
                    path,
                    operation,
                    storage_scope,
                },
            );
        Ok(())
    }
    pub(crate) fn mapped_storage_scope(
        &self,
        reference: &Value,
        path: &str,
    ) -> Result<Option<(Scope, Option<FsOperation>)>, String> {
        let key = Self::ref_key(reference)?;
        let mapped = self
            .mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?;
        if let Some(binding) = mapped.get(&key) {
            return Ok(Some((
                binding.storage_scope.clone(),
                Some(binding.operation),
            )));
        }
        Ok(mapped.values().find_map(|binding| {
            let delegated_children = matches!(
                binding.operation,
                FsOperation::List | FsOperation::Search | FsOperation::Write | FsOperation::Mkdir
            );
            (delegated_children
                && binding.snapshot.is_dir
                && Path::new(path).starts_with(&binding.path))
            .then(|| (binding.storage_scope.clone(), None))
        }))
    }
    fn mapped_path_authorized(&self, path: &str) -> Result<bool, String> {
        Ok(self
            .mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?
            .values()
            .any(|binding| {
                path == binding.path
                    || (binding.snapshot.is_dir
                        && matches!(
                            binding.operation,
                            FsOperation::List
                                | FsOperation::Search
                                | FsOperation::Write
                                | FsOperation::Mkdir
                        )
                        && Path::new(path).starts_with(&binding.path))
            }))
    }
    pub fn authorize_method(&self, method: &str) -> Result<(), String> {
        match method_denial(method, self.metadata_only, self.read_only) {
            Some(MethodDenial::UnsupportedFilesystemNamespace) => {
                Err("unsupported filesystem method namespace".into())
            }
            Some(MethodDenial::ReadOnly) => {
                Err("PermissionDenied: view does not support writes".into())
            }
            Some(MethodDenial::MetadataOnly) => Err("discovery scope permits metadata only".into()),
            None => Ok(()),
        }
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
                .mapped_resources
                .lock()
                .map_err(|_| "mapped resource registry unavailable")?
                .contains_key(&key)
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
            if let Some(snapshot) = self
                .mapped_resources
                .lock()
                .ok()
                .and_then(|mapped| mapped.get(&key).map(|binding| binding.snapshot.clone()))
            {
                return Ok(Some(snapshot));
            }
            return Ok(self.remote_resources.get(&key).cloned().or_else(|| {
                self.issued
                    .lock()
                    .ok()?
                    .get(&key)
                    .filter(|snapshot| {
                        crate::core::fs::provider_registry::resource_access(&snapshot.display)
                            .is_ok_and(|access| {
                                access
                                    == crate::core::fs::provider_registry::ResourceAccess::Provider
                            })
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
        if let Some((key, _)) = self
            .mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?
            .iter()
            .find(|(_, binding)| binding.path == path || binding.snapshot.display == path)
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
                self.mapped_resources
                    .lock()
                    .ok()?
                    .get(&key)
                    .map(|binding| binding.path.clone())
            })
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
        crate::core::fs::provider_registry::validate_child(self, &parent_path, display)?;
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
        let snapshot = crate::plugins::resource::resource_context::from_metadata(display, meta);
        let reference =
            serde_json::to_value(&snapshot.reference).map_err(|error| error.to_string())?;
        self.issued
            .lock()
            .map_err(|_| "reference registry unavailable")?
            .insert(Self::ref_key(&reference)?, snapshot);
        Ok(json!({"ref":reference,"metadata":metadata}))
    }
    pub fn authorize(&self, path: &str) -> Result<(), String> {
        if self.mapped_path_authorized(path)? {
            return Ok(());
        }
        relative(self, path).map(|_| ())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
