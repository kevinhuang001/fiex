//! The builtin search walker is registered like every other native
//! provider. Capability selection is owned by the implementation, not by
//! a special EventRouter or SearchEngineInfo branch.
use super::{SearchMode, BUILTIN_SEARCH};
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{LocationMatch, ProviderKind, ProviderMatchRule};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

trait NativeSearchProvider: Send + Sync {
    fn modes(&self) -> &'static [SearchMode];
}

struct Walker;
impl NativeSearchProvider for Walker {
    fn modes(&self) -> &'static [SearchMode] {
        &[SearchMode::Name, SearchMode::Content]
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn NativeSearchProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn NativeSearchProvider>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut providers: NativeProviderRegistry<dyn NativeSearchProvider> =
            NativeProviderRegistry::new(ProviderKind::Search);
        providers
            .register(
                BUILTIN_SEARCH,
                10,
                ProviderMatchRule {
                    storage: Some(LocationMatch {
                        types: Some(vec!["fs".into()]),
                        ..Default::default()
                    }),
                    kinds: vec!["directory".into()],
                    required_ops: vec![crate::plugins::provider_registry::FsOperation::List],
                    ..Default::default()
                },
                Arc::new(Walker),
            )
            .expect("invalid builtin SearchProvider");
        providers
    })
}

pub(super) fn candidates(targets: &[Value]) -> Vec<Resolver> {
    registry()
        .matched_candidates(targets, None, targets.len())
        .into_iter()
        .filter(|resolver| {
            registry()
                .get(&resolver.id)
                .is_some_and(|provider| !provider.modes().is_empty())
        })
        .collect()
}

pub(super) fn modes(id: &str) -> Vec<SearchMode> {
    registry()
        .get(id)
        .map(|provider| provider.modes().to_vec())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walker_is_registered_once_with_exact_search_modes() {
        assert_eq!(registry().len(), 1);
        let target = serde_json::json!({
            "kind":"directory",
            "view":{"type":"fs","providerId":"core:local-fs","mountId":"local"},
            "storage":{"type":"fs","providerId":"core:local-fs","mountId":"local"},
            "operations":["list"]
        });
        assert_eq!(candidates(&[target])[0].id, BUILTIN_SEARCH);
        assert_eq!(
            modes(BUILTIN_SEARCH),
            vec![SearchMode::Name, SearchMode::Content]
        );
        assert!(modes("builtin:unregistered").is_empty());
        for descriptor in registry().descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::Search);
            descriptor.validate().unwrap();
        }
    }
}
