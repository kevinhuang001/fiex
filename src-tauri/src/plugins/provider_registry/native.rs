//! Application adapters over the shared Runtime registry.
//!
//! Runtime owns descriptors, implementation instances, module ownership and
//! candidate matching. This layer only adapts candidates to the EventRouter
//! and binds Core's request-scoped invocation services.
use crate::plugins::events::Resolver;
#[cfg(test)]
use fiex_provider_contracts::ProviderDescriptor;
use fiex_provider_contracts::{MatchResource, ProviderKind, ProviderMatchRule};
use fiex_runtime::{ModuleDescriptor, ProviderCandidate, TypedProviderRegistry};
use fiex_storage::{native::OwnerBoundStorage, CoreOwnerKey};
use std::sync::Arc;

pub(crate) struct NativeProviderRegistry<T: ?Sized + Send + Sync + 'static> {
    entries: TypedProviderRegistry<T>,
}

impl<T: ?Sized + Send + Sync + 'static> NativeProviderRegistry<T> {
    #[cfg(test)]
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            entries: TypedProviderRegistry::new(kind),
        }
    }

    pub fn shared(kind: ProviderKind) -> Self {
        Self {
            entries: TypedProviderRegistry::shared(kind),
        }
    }

    pub fn module<'a>(
        &'a mut self,
        descriptor: ModuleDescriptor,
    ) -> fiex_runtime::ModuleRegistrar<'a, T> {
        self.entries.module(descriptor)
    }

    pub fn register(
        &mut self,
        id: &'static str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.entries.register(id, priority, rule, implementation)
    }

    pub fn get(&self, id: &str) -> Option<Arc<T>> {
        self.entries.get(id)
    }

    pub fn get_arc(&self, id: &str) -> Option<Arc<T>> {
        self.entries.get_arc(id)
    }

    pub fn owner(&self, id: &str) -> Option<CoreOwnerKey> {
        self.entries.owner(id)
    }

    pub fn management_storage_for(
        &self,
        id: &str,
        scope: &str,
    ) -> Result<fiex_storage::native::NativeScopedStorage, String> {
        let owner = self
            .owner(id)
            .ok_or_else(|| format!("native provider is not registered: {id}"))?;
        OwnerBoundStorage::from_registry(owner).scoped(scope)
    }

    pub fn invocation(
        &self,
        id: &str,
        method: impl Into<String>,
        params: serde_json::Value,
        targets: &[crate::plugins::resource::resource_context::ResourceSnapshot],
        directory: Option<&crate::plugins::resource::resource_context::ResourceSnapshot>,
    ) -> Result<crate::plugins::native_invocation::NativeProviderInvocation, String> {
        let owner = self
            .owner(id)
            .ok_or_else(|| format!("native provider is not registered: {id}"))?;
        Ok(
            crate::plugins::native_invocation::NativeProviderInvocation::new(
                method, params, targets, directory,
            )
            .with_registry_binding(id, owner),
        )
    }

    #[cfg(test)]
    pub fn descriptor(&self, id: &str) -> Option<ProviderDescriptor> {
        self.entries.descriptor(id)
    }

    #[cfg(test)]
    pub fn candidates(&self, eligible: impl FnMut(&T) -> bool) -> Vec<Resolver> {
        self.to_resolvers(self.entries.candidates(eligible))
    }

    pub fn matched_candidates(
        &self,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<Resolver> {
        self.to_resolvers(self.entries.matched_candidates(
            None,
            targets,
            directory,
            selection_count,
        ))
    }

    pub fn event_candidates(
        &self,
        event: &str,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<Resolver> {
        self.to_resolvers(
            self.entries
                .event_candidates(event, targets, directory, selection_count),
        )
    }

    fn to_resolvers(&self, candidates: Vec<ProviderCandidate>) -> Vec<Resolver> {
        candidates
            .into_iter()
            .map(|candidate| Resolver::builtin(candidate.id, candidate.priority))
            .collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.entries.descriptors()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fiex_storage::CoreOwnerKey;

    trait Handler: Send + Sync {
        fn call(&self) -> u32;
    }

    struct First;
    impl Handler for First {
        fn call(&self) -> u32 {
            7
        }
    }

    #[test]
    fn registry_validates_and_retains_typed_handlers() {
        let mut entries: NativeProviderRegistry<dyn Handler> =
            NativeProviderRegistry::new(ProviderKind::Info);
        entries
            .register(
                "core.example",
                100,
                ProviderMatchRule::default(),
                Arc::new(First),
            )
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.get("core.example").unwrap().call(), 7);
        assert!(entries
            .register(
                "core.example",
                90,
                ProviderMatchRule::default(),
                Arc::new(First)
            )
            .is_err());
        assert!(entries
            .register(
                "invalid/id",
                10,
                ProviderMatchRule::default(),
                Arc::new(First)
            )
            .is_err());
        let selected = entries.candidates(|handler| handler.call() == 7);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "core.example");
        assert_eq!(
            entries.owner("core.example"),
            Some(CoreOwnerKey::new("core"))
        );
        assert!(entries.management_storage_for("missing", "scope").is_err());
    }

    #[test]
    fn registered_providers_can_share_one_owner_and_subscribe_to_an_event() {
        let owner = CoreOwnerKey::new("recent");
        let mut entries: NativeProviderRegistry<dyn Handler> =
            NativeProviderRegistry::new(ProviderKind::Hook);
        entries
            .module(ModuleDescriptor::new("core:recent", owner, "1"))
            .register_for_event(
                "core:recent.hook",
                Some("afterOpen"),
                100,
                ProviderMatchRule::default(),
                Arc::new(First),
            )
            .unwrap();
        assert_eq!(entries.owner("core:recent.hook"), Some(owner));
        assert_eq!(
            entries.event_candidates("beforeOpen", &[], None, 1).len(),
            0
        );
        assert_eq!(entries.event_candidates("afterOpen", &[], None, 1).len(), 1);
    }
}
