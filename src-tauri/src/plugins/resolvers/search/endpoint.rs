//! Search plugin transport. Core supplies only a bound ResourceRef and a
//! scope-narrowed request; no plugin-provided path can select its own owner.
use super::proxy::ScopedSearchProxy;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::Value;

impl ProviderEndpoint for ScopedSearchProxy<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.query(request)
    }
}
