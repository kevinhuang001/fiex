//! Persistent typed Core provider instances, one registry per provider kind.
//! Registration validates the exact shared descriptor contract used by SDK
//! providers; invocation never selects an implementation from untrusted input.
use crate::plugins::events::Resolver;
use fiex_provider_contracts::{ProviderDescriptor, ProviderKind, ProviderMatchRule};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct NativeProviderRegistry<T: ?Sized> {
    kind: ProviderKind,
    entries: BTreeMap<&'static str, (ProviderDescriptor, Arc<T>)>,
}

impl<T: ?Sized + Send + Sync> NativeProviderRegistry<T> {
    pub fn new(kind: ProviderKind) -> Self {
        Self {
            kind,
            entries: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: &'static str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        let descriptor = ProviderDescriptor {
            id: id.into(),
            kind: self.kind,
            priority,
            r#match: rule,
        };
        descriptor.validate()?;
        match self.entries.entry(id) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(format!("duplicate native provider ID: {id}"))
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((descriptor, implementation));
                Ok(())
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.get(id).map(|(_, instance)| instance.as_ref())
    }

    pub fn candidates(&self, mut eligible: impl FnMut(&T) -> bool) -> Vec<Resolver> {
        self.entries
            .iter()
            .filter(|(_, (_, instance))| eligible(instance))
            .map(|(id, (descriptor, _))| Resolver::builtin(id, descriptor.priority))
            .collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn descriptors(&self) -> impl Iterator<Item = &ProviderDescriptor> {
        self.entries.values().map(|(descriptor, _)| descriptor)
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
