use fiex_provider_contracts::{
    LocationMatch, ProviderError, ProviderInput, ProviderInvocation, ProviderMatchRule, SearchMode,
    SearchProvider, SearchQueryRequest, SearchResults,
};
use std::sync::Arc;

pub const MODULE_ID: &str = "core:search";
pub const PROVIDER_ID: &str = "core.search";

pub trait SearchModuleServices: Send + Sync {
    fn query(
        &self,
        ctx: &dyn ProviderInvocation,
        request: &SearchQueryRequest,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(fiex_provider_contracts::SearchEntry) -> Result<(), ProviderError>,
    ) -> Result<SearchResults, ProviderError>;
}

pub struct SearchProviderRegistration {
    pub id: &'static str,
    pub priority: i64,
    pub rule: ProviderMatchRule,
    pub provider: Arc<dyn SearchProvider>,
}

pub struct SearchModule {
    services: Arc<dyn SearchModuleServices>,
}

impl SearchModule {
    pub fn new(services: Arc<dyn SearchModuleServices>) -> Self {
        Self { services }
    }

    pub fn registrations(&self) -> Vec<SearchProviderRegistration> {
        vec![SearchProviderRegistration {
            id: PROVIDER_ID,
            priority: 10,
            rule: ProviderMatchRule {
                storage: Some(LocationMatch {
                    types: Some(vec!["fs".into()]),
                    ..Default::default()
                }),
                kinds: vec!["directory".into()],
                required_ops: vec![fiex_provider_contracts::FsOperation::List],
                ..Default::default()
            },
            provider: Arc::new(BuiltinSearch {
                services: self.services.clone(),
            }),
        }]
    }
}

struct BuiltinSearch {
    services: Arc<dyn SearchModuleServices>,
}

impl SearchProvider for BuiltinSearch {
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
        if !self.supports(request.mode) {
            return Err(ProviderError::invalid_params("unsupported search mode"));
        }
        self.services.query(ctx, request, cancelled, emit)
    }
}
