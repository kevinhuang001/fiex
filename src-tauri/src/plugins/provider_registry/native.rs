//! Typed native Core provider registrations share storage, ID validation,
//! and duplicate checking with the SDK's provider-instance registry.
//! Core retains static IDs solely to construct native EventRouter candidates.
use crate::{
    infrastructure::provider_storage::{native::OwnerBoundStorage, CoreOwnerKey},
    plugins::events::Resolver,
};
use fiex_provider_contracts::{
    MatchResource, ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule,
    ProviderMatcher,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

pub(crate) struct NativeProviderRegistry<T: ?Sized + Send + Sync> {
    entries: ProviderInstances<T>,
    ids: Vec<&'static str>,
    owners: HashMap<&'static str, CoreOwnerKey>,
    events: HashMap<&'static str, BTreeSet<&'static str>>,
}

impl<T: ?Sized + Send + Sync> NativeProviderRegistry<T> {
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            entries: ProviderInstances::new(kind),
            ids: Vec::new(),
            owners: HashMap::new(),
            events: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: &'static str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.register_inner(
            id,
            CoreOwnerKey::new("core"),
            None,
            priority,
            rule,
            implementation,
        )
    }

    /// Register a native capability and bind its private state namespace in
    /// the same trusted operation. Wire descriptors never carry this owner.
    pub fn register_owned(
        &mut self,
        id: &'static str,
        owner: CoreOwnerKey,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.register_inner(id, owner, None, priority, rule, implementation)
    }

    /// Hook subscriptions are a registry property because their event name is
    /// not part of the resource MatchRule or the serializable descriptor.
    pub fn register_for_event(
        &mut self,
        id: &'static str,
        event: &'static str,
        owner: CoreOwnerKey,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        if event.is_empty() {
            return Err("native provider event is empty".into());
        }
        self.register_inner(id, owner, Some(event), priority, rule, implementation)
    }

    fn register_inner(
        &mut self,
        id: &'static str,
        owner: CoreOwnerKey,
        event: Option<&'static str>,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.entries.register(
            ProviderDescriptor {
                id: id.into(),
                kind: self.entries.kind(),
                priority,
                r#match: rule,
            },
            implementation,
        )?;
        self.ids.push(id);
        self.owners.insert(id, owner);
        if let Some(event) = event {
            self.events.entry(id).or_default().insert(event);
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.get(id)
    }

    pub fn get_arc(&self, id: &str) -> Option<Arc<T>> {
        self.entries.get_arc(id)
    }

    pub fn owner(&self, id: &str) -> Option<CoreOwnerKey> {
        self.owners.get(id).copied()
    }

    pub fn storage_for(
        &self,
        id: &str,
        scope: &str,
    ) -> Result<crate::infrastructure::provider_storage::native::NativeScopedStorage, String> {
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

    /// Select by an implementation-owned resource match, not a URI switch
    /// in the generic broker. A rejected or unknown resource has no owner.
    #[cfg(test)]
    pub fn descriptor(&self, id: &str) -> Option<&ProviderDescriptor> {
        self.entries.descriptor(id)
    }

    #[cfg(test)]
    pub fn candidates(&self, mut eligible: impl FnMut(&T) -> bool) -> Vec<Resolver> {
        let mut providers = self
            .ids
            .iter()
            .filter_map(|id| {
                let implementation = self.entries.get(id)?;
                let descriptor = self.entries.descriptor(id)?;
                eligible(implementation).then(|| Resolver::builtin(id, descriptor.priority))
            })
            .collect::<Vec<_>>();
        providers.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        providers
    }

    /// Select native providers with the same declarative rule evaluator used
    /// for plugin manifests. `directory` is a trusted Core snapshot used only
    /// when there is no selected target (for example, blank-canvas menus).
    pub fn matched_candidates(
        &self,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<Resolver> {
        self.matched_candidates_for(None, targets, directory, selection_count)
    }

    pub fn event_candidates(
        &self,
        event: &str,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<Resolver> {
        self.matched_candidates_for(Some(event), targets, directory, selection_count)
    }

    fn matched_candidates_for(
        &self,
        event: Option<&str>,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<Resolver> {
        let mut providers = self
            .ids
            .iter()
            .filter_map(|id| {
                let subscribed = self.events.get(id);
                let event_matches = match event {
                    Some(event) => subscribed.is_some_and(|events| events.contains(event)),
                    None => subscribed.is_none(),
                };
                if !event_matches {
                    return None;
                }
                let descriptor = self.entries.descriptor(id)?;
                ProviderMatcher::matches(&descriptor.r#match, targets, directory, selection_count)
                    .then(|| Resolver::builtin(id, descriptor.priority))
            })
            .collect::<Vec<_>>();
        providers.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        providers
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn descriptors(&self) -> impl Iterator<Item = &ProviderDescriptor> {
        self.entries.descriptors()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::provider_storage::CoreOwnerKey;

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
        assert!(entries.storage_for("missing", "scope").is_err());
    }

    #[test]
    fn registered_providers_can_share_one_owner_and_subscribe_to_an_event() {
        let owner = CoreOwnerKey::new("recent");
        let mut entries: NativeProviderRegistry<dyn Handler> =
            NativeProviderRegistry::new(ProviderKind::Hook);
        entries
            .register_for_event(
                "core:recent.hook",
                "afterOpen",
                owner,
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
