//! Shared registry descriptors and erased implementation records.
use fiex_provider_contracts::{ProviderDescriptor, ProviderKind, ProviderMatchRule};
use fiex_storage::CoreOwnerKey;
use std::{
    any::Any,
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

pub(super) type ErasedImplementation = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Debug)]
pub struct ModuleDescriptor {
    pub id: String,
    pub owner: CoreOwnerKey,
    pub version: String,
}

impl ModuleDescriptor {
    pub fn new(id: impl Into<String>, owner: CoreOwnerKey, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            owner,
            version: version.into(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
        {
            return Err("invalid module ID".into());
        }
        if self.version.is_empty() || self.version.len() > 64 {
            return Err("invalid module version".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCandidate {
    pub id: String,
    pub kind: ProviderKind,
    pub priority: i64,
    pub module_id: String,
}

pub(super) struct Registration {
    pub(super) descriptor: ProviderDescriptor,
    pub(super) module: ModuleDescriptor,
    pub(super) events: BTreeSet<String>,
    pub(super) implementation: ErasedImplementation,
}

pub(super) struct RegistrationRequest<T: ?Sized> {
    pub(super) kind: ProviderKind,
    pub(super) module: ModuleDescriptor,
    pub(super) id: String,
    pub(super) uri_route: Option<(&'static str, &'static str)>,
    pub(super) event: Option<String>,
    pub(super) priority: i64,
    pub(super) rule: ProviderMatchRule,
    pub(super) implementation: Arc<T>,
}

#[derive(Default)]
pub(super) struct RegistryState {
    pub(super) modules: HashMap<String, ModuleDescriptor>,
    pub(super) providers: HashMap<(ProviderKind, String), Registration>,
    pub(super) uri_routes: HashMap<&'static str, &'static str>,
}
