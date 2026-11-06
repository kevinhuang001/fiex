//! The builtin walker implements the same shared SearchProvider trait as
//! plugin proxies. Core invokes it directly and keeps cancellation and result
//! streaming inside the process.
use super::SearchMode;
#[cfg(test)]
use super::BUILTIN_SEARCH;
use crate::plugins::{
    events::Resolver, native_invocation::NativeProviderInvocation,
    provider_registry::native::NativeProviderRegistry,
};
#[cfg(test)]
use fiex_provider_contracts::{MatchResource, ResourceCapabilities, ResourceMetadata};
use fiex_provider_contracts::{
    ProviderError, ProviderInvocation, ProviderKind, SearchProvider, SearchResults,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;

struct NativeSearchServices;

impl fiex_search::SearchModuleServices for NativeSearchServices {
    fn query(
        &self,
        ctx: &dyn ProviderInvocation,
        request: &fiex_provider_contracts::SearchQueryRequest,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(fiex_provider_contracts::SearchEntry) -> Result<(), ProviderError>,
    ) -> Result<SearchResults, ProviderError> {
        let root = NativeProviderInvocation::resource(ctx, 0)?;
        if !root.is_dir {
            return Err(ProviderError::invalid_params(
                "search target must be a directory",
            ));
        }
        let truncated = crate::core::search::run_native_search_stream(
            &root.local,
            request.query.trim(),
            request.mode,
            request.hidden,
            &request.ignores,
            request.bounded_limit(),
            cancelled,
            &mut |entry| emit(super::search_entry(entry)).map_err(|error| error.to_string()),
        )
        .map_err(ProviderError::internal)?;
        Ok(SearchResults {
            entries: Vec::new(),
            truncated,
            next_cursor: None,
        })
    }
}

pub(super) fn register(providers: &mut NativeProviderRegistry<dyn SearchProvider>) {
    let registrations =
        fiex_search::SearchModule::new(Arc::new(NativeSearchServices)).registrations();
    let mut module = providers.module(ModuleDescriptor::new(
        fiex_search::MODULE_ID,
        CoreOwnerKey::new("core"),
        env!("CARGO_PKG_VERSION"),
    ));
    for registration in registrations {
        module
            .register(
                registration.id,
                registration.priority,
                registration.rule,
                registration.provider,
            )
            .expect("invalid builtin SearchProvider");
    }
}

fn registry() -> NativeProviderRegistry<dyn SearchProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::Search)
}

pub(super) fn candidates(targets: &[fiex_provider_contracts::MatchResource]) -> Vec<Resolver> {
    registry()
        .matched_candidates(targets, None, targets.len())
        .into_iter()
        .filter(|resolver| {
            registry().get(&resolver.id).is_some_and(|provider| {
                provider.supports(SearchMode::Name) || provider.supports(SearchMode::Content)
            })
        })
        .collect()
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn SearchProvider>> {
    registry().get(id)
}

pub(super) fn provider_arc(id: &str) -> Option<Arc<dyn SearchProvider>> {
    registry().get_arc(id)
}

pub(super) fn invocation(
    id: &str,
    method: impl Into<String>,
    params: serde_json::Value,
    resources: &[crate::plugins::resource::resource_context::ResourceSnapshot],
) -> Result<NativeProviderInvocation, String> {
    registry().invocation(id, method, params, resources, None)
}

pub(super) fn modes(id: &str) -> Vec<SearchMode> {
    let Some(provider) = provider(id) else {
        return Vec::new();
    };
    [SearchMode::Name, SearchMode::Content]
        .into_iter()
        .filter(|mode| provider.supports(*mode))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walker_is_registered_once_with_exact_search_modes() {
        assert_eq!(registry().len(), 1);
        let (view, storage) = crate::plugins::match_rules::typed_locations_for("/tmp");
        let target = MatchResource {
            metadata: Some(ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            }),
            view,
            storage,
            capabilities: ResourceCapabilities {
                operations: vec![crate::plugins::provider_registry::FsOperation::List],
            },
        };
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
