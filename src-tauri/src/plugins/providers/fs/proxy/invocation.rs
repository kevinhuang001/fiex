use super::Mount;
use crate::plugins::native_invocation::NativeProviderInvocation;
use fiex_provider_contracts::{ProviderError, ProviderInvocation, ProviderRequestContext};
use serde_json::Value;
use std::any::Any;

pub(super) struct MountInvocation {
    base: NativeProviderInvocation,
    mount: Mount,
}

impl MountInvocation {
    pub fn new(mount: Mount, method: &str, params: Value) -> Self {
        Self {
            base: NativeProviderInvocation::new(method, params, &[], None),
            mount,
        }
    }

    pub(super) fn mount(ctx: &dyn ProviderInvocation) -> Result<&Mount, ProviderError> {
        ctx.local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .map(|invocation| &invocation.mount)
            .ok_or_else(|| ProviderError::internal("missing Core mount context"))
    }
}

impl ProviderInvocation for MountInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(self)
    }
}
