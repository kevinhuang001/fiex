//! Process-local invocation context for Core's implementations of the shared
//! provider traits. The serializable request contains only trusted, projected
//! resource metadata; native services remain behind this process-local handle.
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderError, ProviderInvocation, ProviderRequestContext};
use serde_json::Value;
use std::any::Any;

pub(crate) struct NativeProviderInvocation {
    request: ProviderRequestContext,
    targets: Vec<ResourceSnapshot>,
    directory: Option<ResourceSnapshot>,
}

impl NativeProviderInvocation {
    pub(crate) fn new(
        method: impl Into<String>,
        params: Value,
        targets: &[ResourceSnapshot],
        directory: Option<&ResourceSnapshot>,
    ) -> Self {
        let session_id = params
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let request = ProviderRequestContext {
            method: method.into(),
            params,
            targets: targets
                .iter()
                .map(|resource| resource.project("core", &session_id, true, true))
                .collect(),
            directory: directory.map(|resource| resource.project("core", &session_id, true, true)),
            session_id,
            selection_count: targets.len(),
            locale: crate::i18n::locale().to_owned(),
        };
        Self {
            request,
            targets: targets.to_vec(),
            directory: directory.cloned(),
        }
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

    #[allow(dead_code)]
    pub(crate) fn directory(
        invocation: &dyn ProviderInvocation,
    ) -> Result<&ResourceSnapshot, ProviderError> {
        invocation
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .and_then(|native| native.directory.as_ref())
            .ok_or_else(|| ProviderError::invalid_params("missing native directory context"))
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
