//! Local and USB storage execute directly through the shared VFS provider.
mod streams;
mod usb;
use super::{shared::*, storage};
use crate::{
    core::{files::Entry, fs::ResourceBroker},
    plugins::provider_registry::FsOperation,
    plugins::resource::fs_access,
    usbfs::{self, Route},
};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsRead, FsRequest, FsWrite, PreflightDecision, ProviderError,
    ProviderInvocation, SearchCapabilities, TransferPreflight, VfsProvider,
};
use std::path::Path;

pub(super) struct StorageProvider {
    id: &'static str,
}

impl StorageProvider {
    pub(super) const fn new(id: &'static str) -> Self {
        Self { id }
    }

    fn resource(&self, ctx: &dyn ProviderInvocation) -> Result<&ResourceBroker, ProviderError> {
        let resource = broker(ctx)?;
        resource
            .ensure_current_binding()
            .map_err(provider_error)?;
        if super::shared_provider_id(&resource.path) != Some(self.id) {
            return Err(ProviderError::invalid_params("storage provider does not own resource"));
        }
        Ok(resource)
    }

    fn entry(&self, ctx: &dyn ProviderInvocation) -> Result<Entry, ProviderError> {
        let resource = self.resource(ctx)?;
        if let Some(route) = usbfs::route(&resource.path) {
            return usbfs::entry(&route)
                .map(|entry| storage::decorated_entry(&resource.path, entry))
                .map_err(provider_error);
        }
        resource.scope.authorize(&resource.path).map_err(provider_error)?;
        let value = fs_access::stat(&resource.scope, &resource.path).map_err(provider_error)?;
        let entry = storage::entry(value).map_err(provider_error)?;
        Ok(storage::decorated_entry(&resource.path, entry))
    }

    fn validate_node(&self, request: &FsRequest<'_>) -> Result<ResourceBroker, ProviderError> {
        let resource = self.resource(request.ctx())?.clone_binding();
        if request.node_id() != Some(resource.path.as_str()) {
            return Err(ProviderError::invalid_params("nodeId does not match scoped resource"));
        }
        Ok(resource)
    }

    fn validate_child(&self, request: &FsRequest<'_>) -> Result<ResourceBroker, ProviderError> {
        let resource = self.resource(request.ctx())?.clone_binding();
        Self::validate_child_path(&resource, request)?;
        Ok(resource)
    }

    fn validate_child_path(
        resource: &ResourceBroker,
        request: &FsRequest<'_>,
    ) -> Result<(), ProviderError> {
        let parent = crate::core::fs::parent_path(&resource.path).map_err(provider_error)?;
        let name = Path::new(&resource.path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .ok_or_else(|| ProviderError::invalid_params("scoped resource has no name"))?;
        if request.parent_id() != Some(parent.as_str()) || request.name() != Some(name.as_str()) {
            return Err(ProviderError::invalid_params("child identity does not match scoped resource"));
        }
        Ok(())
    }

    fn authorize_storage(
        &self,
        resource: &ResourceBroker,
        operation: FsOperation,
        create: bool,
    ) -> Result<(), ProviderError> {
        resource.ensure_current_binding().map_err(provider_error)?;
        if let Some(route) = usbfs::route(&resource.path) {
            return usb::authorize(&route, operation, create);
        }
        resource.scope.authorize(&resource.path).map_err(provider_error)?;
        if crate::core::volumes::application_policy::protected(&resource.path)
            && !matches!(
                operation,
                FsOperation::Read | FsOperation::List | FsOperation::Stat | FsOperation::Search
            )
        {
            return Err(ProviderError::invalid_params(
                "PermissionDenied: installed applications are managed by the operating system",
            ));
        }
        if create || operation == FsOperation::Mkdir {
            Path::new(&resource.path)
                .parent()
                .ok_or_else(|| ProviderError::invalid_params("invalid parent"))?;
            return Ok(());
        }
        let entry = self.entry_for_resource(resource)?;
        let allowed = storage::decorated_entry(&resource.path, entry)
            .allowed_ops
            .iter()
            .any(|name| name == operation.as_str());
        if allowed {
            Ok(())
        } else {
            Err(ProviderError::invalid_params(format!("Unsupported: {}", operation.as_str())))
        }
    }

    fn entry_for_resource(&self, resource: &ResourceBroker) -> Result<Entry, ProviderError> {
        if let Some(route) = usbfs::route(&resource.path) {
            usbfs::entry(&route).map_err(provider_error)
        } else {
            resource.scope.authorize(&resource.path).map_err(provider_error)?;
            let value = fs_access::stat(&resource.scope, &resource.path).map_err(provider_error)?;
            storage::entry(value).map_err(provider_error)
        }
    }

    fn listing(&self, resource: &ResourceBroker) -> Result<Vec<Entry>, ProviderError> {
        if let Some(route) = usbfs::route(&resource.path) {
            return usbfs::listing(&route).map_err(provider_error);
        }
        let value = fs_access::list(&resource.scope, &resource.path).map_err(provider_error)?;
        value
            .as_array()
            .ok_or_else(|| ProviderError::internal("invalid storage listing"))?
            .iter()
            .cloned()
            .map(|value| storage::entry(value).map_err(provider_error))
            .collect()
    }


}

impl VfsProvider for StorageProvider {
    fn supports_preflight(&self) -> bool {
        true
    }

    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Read,
            FsOperation::Write,
            FsOperation::Mkdir,
            FsOperation::Rename,
            FsOperation::Remove,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        let entry = self.entry(ctx)?;
        Ok(entry
            .allowed_ops
            .iter()
            .filter_map(|operation| FsOperation::parse(operation))
            .collect())
    }

    fn authorize(
        &self,
        ctx: &dyn ProviderInvocation,
        operation: FsOperation,
        create: bool,
    ) -> Result<(), ProviderError> {
        let resource = self.resource(ctx)?;
        self.authorize_storage(resource, operation, create)
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: self
                .entry(ctx)
                .is_ok_and(|entry| entry.allowed_ops.iter().any(|op| op == "search")),
            content: false,
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let resource = self.validate_node(request)?;
        self.authorize_storage(&resource, FsOperation::Stat, false)?;
        self.entry(request.ctx()).map(shared_entry)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let resource = self.validate_node(request)?;
        self.authorize_storage(&resource, FsOperation::List, false)?;
        let ignore_patterns = request.ignores();
        let ignores = crate::core::ignores::IgnoreSet::new(&ignore_patterns);
        let mut entries = self
            .listing(&resource)?
            .into_iter()
            .filter(|entry| {
                (request.hidden() || !entry.name.starts_with('.'))
                    && !ignores.is_ignored(Path::new(&entry.name))
            })
            .map(|entry| {
                let path = entry.path.clone();
                storage::decorated_entry(&path, entry)
            })
            .collect::<Vec<_>>();
        entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
        Ok(list_result(entries))
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        let resource = self.validate_node(request)?;
        self.authorize_storage(&resource, FsOperation::Read, false)?;
        streams::read(resource)
    }

    fn write(&self, request: &FsRequest<'_>) -> Result<FsWrite, ProviderError> {
        let create = request.node_id().is_none();
        let resource = if create {
            self.validate_child(request)?
        } else {
            self.validate_node(request)?
        };
        self.authorize_storage(&resource, FsOperation::Write, create)?;
        streams::write(resource, create)
    }

    fn mkdir(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let resource = self.validate_child(request)?;
        self.authorize_storage(&resource, FsOperation::Mkdir, true)?;
        if let Some(route) = usbfs::route(&resource.path) {
            let (parent, name) = route.rel.rsplit_once('/').unwrap_or(("", &route.rel));
            if name.is_empty() {
                return Err(ProviderError::invalid_params("cannot create USB root"));
            }
            usbfs::create(
                &Route {
                    rel: parent.into(),
                    ..route.clone()
                },
                name,
                true,
            )
            .map_err(provider_error)?;
            return usbfs::entry(&route)
                .map(|entry| shared_entry(storage::decorated_entry(&resource.path, entry)))
                .map_err(provider_error);
        }
        let value = fs_access::mkdir(&resource.scope, &resource.path).map_err(provider_error)?;
        let entry = storage::entry(value).map_err(provider_error)?;
        Ok(shared_entry(storage::decorated_entry(&resource.path, entry)))
    }

    fn rename(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let resource = self.validate_node(request)?;
        let destination = destination_broker(request.ctx())?.clone_binding();
        Self::validate_child_path(&destination, request)?;
        if super::shared_provider_id(&resource.path) != super::shared_provider_id(&destination.path) {
            return Err(ProviderError::invalid_params(
                "Unsupported: cross-provider rename requires TransferEngine",
            ));
        }
        self.authorize_storage(&resource, FsOperation::Rename, false)?;
        self.authorize_storage(&destination, FsOperation::Write, true)?;
        if let Some(route) = usbfs::route(&resource.path) {
            let other = usbfs::route(&destination.path)
                .ok_or_else(|| ProviderError::invalid_params("invalid USB destination"))?;
            let (parent, name) = other.rel.rsplit_once('/').unwrap_or(("", &other.rel));
            let source_parent = route.rel.rsplit_once('/').map(|(path, _)| path).unwrap_or("");
            if other.root != route.root || parent != source_parent {
                return Err(ProviderError::invalid_params(
                    "Unsupported: cross-directory USB rename; use TransferEngine",
                ));
            }
            if usbfs::entry(&other).is_ok() {
                return Err(ProviderError::invalid_params("Conflict: destination already exists"));
            }
            usbfs::rename(&route, name).map_err(provider_error)?;
            let entry = usbfs::entry(&other).map_err(provider_error)?;
            return Ok(shared_entry(storage::decorated_entry(&destination.path, entry)));
        }
        fs_access::rename(&resource.scope, &resource.path, &destination.path)
            .map_err(provider_error)?;
        let value = fs_access::stat(&destination.scope, &destination.path).map_err(provider_error)?;
        let entry = storage::entry(value).map_err(provider_error)?;
        Ok(shared_entry(storage::decorated_entry(&destination.path, entry)))
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let resource = self.validate_node(request)?;
        self.authorize_storage(&resource, FsOperation::Remove, false)?;
        if let Some(route) = usbfs::route(&resource.path) {
            return usb::remove_tree(&route, 0).map_err(provider_error);
        }
        fs_access::remove(&resource.scope, &resource.path).map_err(provider_error)
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        self.search_cancellable(request, &|| false)
    }

    fn search_cancellable(
        &self,
        request: &FsRequest<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FsListing, ProviderError> {
        let resource = self.validate_node(request)?;
        self.authorize_storage(&resource, FsOperation::Search, false)?;
        let query = request
            .query()
            .filter(|query| !query.is_empty())
            .ok_or_else(|| ProviderError::invalid_params("search query is required"))?;
        let (entries, truncated, _) = if let Some(route) = usbfs::route(&resource.path) {
            let (entries, truncated) = usbfs::search(&route, query, request.limit(), cancelled)
                .map_err(provider_error)?;
            (entries, truncated, 0)
        } else {
            crate::core::search::search_directory(
                Path::new(&resource.path),
                query,
                request.hidden(),
                &request.ignores(),
                request.limit(),
                cancelled,
            )
        };
        let ignore_patterns = request.ignores();
        let filter = crate::core::ignores::IgnoreSet::new(&ignore_patterns);
        let entries = entries
            .into_iter()
            .filter(|entry| {
                (request.hidden() || !entry.name.starts_with('.'))
                    && !filter.is_ignored(Path::new(&entry.name))
            })
            .map(|entry| {
                let path = entry.path.clone();
                storage::decorated_entry(&path, entry)
            })
            .collect();
        Ok(FsListing {
            entries: entries.into_iter().map(shared_entry).collect(),
            truncated,
            ..Default::default()
        })
    }

    fn preflight(
        &self,
        ctx: &dyn ProviderInvocation,
        _request: &TransferPreflight,
    ) -> Result<PreflightDecision, ProviderError> {
        self.resource(ctx)?;
        Ok(PreflightDecision {
            allowed: true,
            reason: None,
        })
    }
}
