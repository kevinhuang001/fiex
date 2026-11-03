//! Process-local invocation context for Core's implementations of the shared
//! provider traits. The serializable request contains only trusted, projected
//! resource metadata; native services remain behind this process-local handle.
use crate::core::fs::mapped_access::UserActionContext;
use crate::infrastructure::provider_storage::{
    native::{NativeScopedStorage, OwnerBoundStorage},
    CoreOwnerKey,
};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderError, ProviderInvocation, ProviderRequestContext};
use serde_json::Value;
use std::{any::Any, sync::Arc};

pub(crate) struct NativeProviderInvocation {
    request: ProviderRequestContext,
    targets: Vec<ResourceSnapshot>,
    directory: Option<ResourceSnapshot>,
    local_data: Option<Arc<dyn Any + Send + Sync>>,
    registration: Option<NativeProviderServices>,
    user_action: Option<UserActionContext>,
}

struct NativeProviderServices {
    provider_id: String,
    storage: OwnerBoundStorage,
}

impl NativeProviderInvocation {
    pub(crate) fn new(
        method: impl Into<String>,
        params: Value,
        targets: &[ResourceSnapshot],
        directory: Option<&ResourceSnapshot>,
    ) -> Self {
        let request = ProviderRequestContext::from_params(method, &params, crate::i18n::locale());
        Self::from_request_context(request, targets, directory)
    }

    /// Build an invocation from an already-decoded standard event. This keeps
    /// native dispatch typed; only `new` is used when crossing a JSON boundary.
    pub(crate) fn from_request_context(
        mut request: ProviderRequestContext,
        targets: &[ResourceSnapshot],
        directory: Option<&ResourceSnapshot>,
    ) -> Self {
        request.targets = targets
            .iter()
            .map(|resource| resource.project("core", &request.session_id, true, true))
            .collect();
        request.directory =
            directory.map(|resource| resource.project("core", &request.session_id, true, true));
        request.selection_count = targets.len();
        Self {
            request,
            targets: targets.to_vec(),
            directory: directory.cloned(),
            local_data: None,
            registration: None,
            user_action: None,
        }
    }

    pub(crate) fn context_menu_resolve(
        invocation: &dyn ProviderInvocation,
    ) -> Result<Self, ProviderError> {
        let mut request = invocation.request().clone();
        request.method = "contextMenu.resolve".into();
        request.input = invocation
            .input()?
            .context_menu_resolve_for_execute()
            .map_err(ProviderError::invalid_params)?;
        request.input_error = None;
        let resources = Self::resources(invocation)?;
        let directory = Self::directory(invocation).ok();
        Ok(Self::from_request_context(request, resources, directory))
    }

    pub(crate) fn with_local_data<T: Any + Send + Sync>(mut self, data: Arc<T>) -> Self {
        self.local_data = Some(data);
        self
    }

    pub(crate) fn with_user_action_context(mut self, user_action: UserActionContext) -> Self {
        self.user_action = Some(user_action);
        self
    }

    pub(crate) fn with_registry_binding(mut self, provider_id: &str, owner: CoreOwnerKey) -> Self {
        self.registration = Some(NativeProviderServices {
            provider_id: provider_id.to_owned(),
            storage: OwnerBoundStorage::from_registry(owner),
        });
        self
    }

    /// Return the scoped storage handle only for the provider identity bound
    /// by its typed native registry. The binding is local process data and is
    /// never part of ProviderRequestContext serialization.
    pub(crate) fn storage(
        invocation: &dyn ProviderInvocation,
        provider_id: &str,
        scope: &str,
    ) -> Result<NativeScopedStorage, ProviderError> {
        let services = invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.registration.as_ref())
            .filter(|services| services.provider_id == provider_id)
            .ok_or_else(|| ProviderError::invalid_params("missing registered owner services"))?;
        services
            .storage
            .scoped(scope)
            .map_err(ProviderError::internal)
    }

    pub(crate) fn user_action(
        invocation: &dyn ProviderInvocation,
    ) -> Result<UserActionContext, ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.user_action)
            .ok_or_else(|| ProviderError::invalid_params("missing user action context"))
    }

    pub(crate) fn resource(
        invocation: &dyn ProviderInvocation,
        index: usize,
    ) -> Result<&ResourceSnapshot, ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.targets.get(index))
            .ok_or_else(|| ProviderError::invalid_params("missing native resource context"))
    }

    pub(crate) fn resources(
        invocation: &dyn ProviderInvocation,
    ) -> Result<&[ResourceSnapshot], ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .map(|native| native.targets.as_slice())
            .ok_or_else(|| ProviderError::invalid_params("missing native resource context"))
    }

    pub(crate) fn directory(
        invocation: &dyn ProviderInvocation,
    ) -> Result<&ResourceSnapshot, ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.directory.as_ref())
            .ok_or_else(|| ProviderError::invalid_params("missing native directory context"))
    }

    pub(crate) fn local_data<T: Any + Send + Sync>(
        invocation: &dyn ProviderInvocation,
    ) -> Result<&T, ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.local_data.as_ref())
            .and_then(|value| value.downcast_ref::<T>())
            .ok_or_else(|| ProviderError::invalid_params("missing native provider service"))
    }
}

impl ProviderInvocation for NativeProviderInvocation {
    fn request(&self) -> &ProviderRequestContext {
        &self.request
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(self)
    }
}
