//! Module-scoped registration API for typed providers.
use super::{
    TypedProviderRegistry,
    types::{ModuleDescriptor, RegistrationRequest},
};
use fiex_provider_contracts::ProviderMatchRule;
use std::sync::Arc;

pub struct ModuleRegistrar<'a, T: ?Sized + Send + Sync + 'static> {
    pub(super) registry: &'a mut TypedProviderRegistry<T>,
    pub(super) descriptor: ModuleDescriptor,
}

impl<T: ?Sized + Send + Sync + 'static> ModuleRegistrar<'_, T> {
    pub fn register(
        &mut self,
        id: &str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.register_for_event(id, None, priority, rule, implementation)
    }

    pub fn register_for_event(
        &mut self,
        id: &str,
        event: Option<&str>,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.registry.runtime.register(RegistrationRequest {
            kind: self.registry.kind,
            module: self.descriptor.clone(),
            id: id.into(),
            uri_route: None,
            event: event.map(Into::into),
            priority,
            rule,
            implementation,
        })
    }

    /// Declare a URI scheme for one of this module's VFS providers.
    pub fn register_vfs_uri(
        &mut self,
        provider_id: &'static str,
        scheme: &'static str,
    ) -> Result<(), String> {
        self.registry.runtime.register_uri_alias(
            self.registry.kind,
            &self.descriptor.id,
            provider_id,
            scheme,
        )
    }
}
