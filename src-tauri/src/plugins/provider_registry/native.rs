//! Typed native Core provider registrations share storage, ID validation,
//! and duplicate checking with the SDK's provider-instance registry.
//! Core retains static IDs solely to construct native EventRouter candidates.
use crate::plugins::events::Resolver;
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule,
};
use std::sync::Arc;

pub(crate) struct NativeProviderRegistry<T: ?Sized + Send + Sync> {
    entries: ProviderInstances<T>,
    ids: Vec<&'static str>,
}

impl<T: ?Sized + Send + Sync> NativeProviderRegistry<T> {
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            entries: ProviderInstances::new(kind),
            ids: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        id: &'static str,
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
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.get(id)
    }

    /// Select by an implementation-owned resource match, not a URI switch
    /// in the generic broker. A rejected or unknown resource has no owner.
    pub fn find(&self, mut matches: impl FnMut(&T) -> bool) -> Option<&T> {
        self.entries
            .instances()
            .find_map(|(_, provider)| matches(provider).then_some(provider))
    }

    pub fn descriptor(&self, id: &str) -> Option<&ProviderDescriptor> {
        self.entries.descriptor(id)
    }

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
    }
}
