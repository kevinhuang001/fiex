use super::*;
use fiex_provider_contracts::FsOperation;

#[derive(Clone)]
pub(super) struct MappedResourceBinding {
    pub(super) snapshot: crate::plugins::resource::resource_context::ResourceSnapshot,
    pub(super) path: String,
    pub(super) operation: FsOperation,
    pub(super) storage_scope: Scope,
}

impl Scope {
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
    pub(super) fn mapped_path_authorized(&self, path: &str) -> Result<bool, String> {
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
}
