//! The builtin walker implements the same shared SearchProvider trait as
//! plugin proxies. Core invokes it directly and keeps cancellation and result
//! streaming inside the process.
use super::{SearchMode, BUILTIN_SEARCH};
use crate::plugins::{
    events::Resolver, native_invocation::NativeProviderInvocation,
    provider_registry::native::NativeProviderRegistry,
};
use fiex_provider_contracts::{
    LocationMatch, MatchResource, ProviderError, ProviderInput, ProviderInvocation, ProviderKind,
    ProviderMatchRule, ResourceCapabilities, ResourceMetadata, SearchProvider, SearchResults,
};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

struct Walker;

impl SearchProvider for Walker {
    fn supports(&self, mode: SearchMode) -> bool {
        matches!(mode, SearchMode::Name | SearchMode::Content)
    }

    fn query(&self, ctx: &dyn ProviderInvocation) -> Result<SearchResults, ProviderError> {
        let mut entries = Vec::new();
        let page = self.query_stream(ctx, &|| false, &mut |entry| {
            entries.push(entry);
            Ok(())
        })?;
        Ok(SearchResults { entries, ..page })
    }

    fn query_stream(
        &self,
        ctx: &dyn ProviderInvocation,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(fiex_provider_contracts::SearchEntry) -> Result<(), ProviderError>,
    ) -> Result<SearchResults, ProviderError> {
        if ctx.method() != "search.query" {
            return Err(ProviderError::invalid_params(
                "SearchProvider method mismatch",
            ));
        }
        let request = match ctx.input()? {
            ProviderInput::Search(request) => request,
            _ => return Err(ProviderError::invalid_params("missing search request")),
        };
        let query = request.query.trim();
        let mode = request.mode;
        if !self.supports(mode) {
            return Err(ProviderError::invalid_params("unsupported search mode"));
        }
        let hidden = request.hidden;
        let ignores = &request.ignores;
        let limit = request.bounded_limit();
        let root = NativeProviderInvocation::resource(ctx, 0)?;
        if !root.is_dir {
            return Err(ProviderError::invalid_params(
                "search target must be a directory",
            ));
        }
        let truncated = crate::core::search::run_native_search_stream(
            &root.local,
            query,
            mode,
            hidden,
            ignores,
            limit,
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

fn registry() -> &'static NativeProviderRegistry<dyn SearchProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn SearchProvider>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut providers: NativeProviderRegistry<dyn SearchProvider> =
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

pub(super) fn provider(id: &str) -> Option<&'static dyn SearchProvider> {
    registry().get(id)
}

pub(super) fn provider_arc(id: &str) -> Option<Arc<dyn SearchProvider>> {
    registry().get_arc(id)
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
