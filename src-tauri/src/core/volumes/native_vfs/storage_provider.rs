//! Local and USB storage execute directly through the shared VFS provider.
mod streams;
mod usb;
use super::{shared::*, storage};
use crate::{
    core::{files::Entry, fs::ResourceBroker},
    plugins::provider_registry::FsOperation,
    plugins::resource::fs_access,
    usbfs,
};
use fiex_provider_contracts::{FsRequest, ProviderError, ProviderInvocation};
use std::path::Path;

pub(super) struct StorageProvider {
    id: &'static str,
}

impl StorageProvider {
    pub(super) const fn new(id: &'static str) -> Self {
        Self { id }
    }

    fn resource<'a>(
        &self,
        ctx: &'a dyn ProviderInvocation,
    ) -> Result<&'a ResourceBroker, ProviderError> {
        let resource = broker(ctx)?;
        super::ensure_current_binding(&resource.path, &resource.reference)
            .map_err(provider_error)?;
        if super::shared_provider_id(&resource.path) != Some(self.id) {
            return Err(ProviderError::invalid_params(
                "storage provider does not own resource",
            ));
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
        resource
            .storage_scope
            .authorize(&resource.path)
            .map_err(provider_error)?;
        let value =
            fs_access::stat(&resource.storage_scope, &resource.path).map_err(provider_error)?;
        let entry = storage::entry(value).map_err(provider_error)?;
        Ok(storage::decorated_entry(&resource.path, entry))
    }

    fn validate_node(&self, request: &FsRequest<'_>) -> Result<ResourceBroker, ProviderError> {
        let resource = self.resource(request.ctx())?.clone_binding();
        if request.node_id() != Some(resource.path.as_str()) {
            return Err(ProviderError::invalid_params(
                "nodeId does not match scoped resource",
            ));
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
            return Err(ProviderError::invalid_params(
                "child identity does not match scoped resource",
            ));
        }
        Ok(())
    }

    fn authorize_storage(
        &self,
        resource: &ResourceBroker,
        operation: FsOperation,
        create: bool,
    ) -> Result<(), ProviderError> {
        if let Some(route) = usbfs::route(&resource.path) {
            return usb::authorize(&route, operation, create);
        }
        resource
            .storage_scope
            .authorize(&resource.path)
            .map_err(provider_error)?;
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
            Err(ProviderError::invalid_params(format!(
                "Unsupported: {}",
                operation.as_str()
            )))
        }
    }

    fn entry_for_resource(&self, resource: &ResourceBroker) -> Result<Entry, ProviderError> {
        if let Some(route) = usbfs::route(&resource.path) {
            usbfs::entry(&route).map_err(provider_error)
        } else {
            resource
                .storage_scope
                .authorize(&resource.path)
                .map_err(provider_error)?;
            let value =
                fs_access::stat(&resource.storage_scope, &resource.path).map_err(provider_error)?;
            storage::entry(value).map_err(provider_error)
        }
    }

    fn listing(&self, resource: &ResourceBroker) -> Result<Vec<Entry>, ProviderError> {
        if let Some(route) = usbfs::route(&resource.path) {
            return usbfs::listing(&route).map_err(provider_error);
        }
        let value =
            fs_access::list(&resource.storage_scope, &resource.path).map_err(provider_error)?;
        value
            .as_array()
            .ok_or_else(|| ProviderError::internal("invalid storage listing"))?
            .iter()
            .cloned()
            .map(|value| storage::entry(value).map_err(provider_error))
            .collect()
    }
}

mod provider;
