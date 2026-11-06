//! Typed registry views keep provider kinds and implementation types aligned.
use super::{
    module_registrar::ModuleRegistrar,
    runtime::{global_runtime_registry, RuntimeRegistry},
    types::{ModuleDescriptor, ProviderCandidate},
};
use fiex_provider_contracts::{MatchResource, ProviderDescriptor, ProviderKind, ProviderMatcher};
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;

/// Typed view over a registry. It prevents retrieving an instance under the
/// wrong ProviderKind and keeps the implementation trait object typed.
pub struct TypedProviderRegistry<T: ?Sized + Send + Sync + 'static> {
    pub(super) runtime: Arc<RuntimeRegistry>,
    pub(super) kind: ProviderKind,
    marker: std::marker::PhantomData<fn() -> T>,
}

impl<T: ?Sized + Send + Sync + 'static> TypedProviderRegistry<T> {
    /// Create an isolated registry, primarily useful for unit tests.
    pub fn new(kind: ProviderKind) -> Self {
        Self::from_runtime(Arc::new(RuntimeRegistry::new()), kind)
    }

    /// Create a typed view over the process-wide runtime registry.
    pub fn shared(kind: ProviderKind) -> Self {
        Self::from_runtime(Arc::clone(global_runtime_registry()), kind)
    }

    pub fn from_runtime(runtime: Arc<RuntimeRegistry>, kind: ProviderKind) -> Self {
        Self {
            runtime,
            kind,
            marker: std::marker::PhantomData,
        }
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn module<'a>(&'a mut self, descriptor: ModuleDescriptor) -> ModuleRegistrar<'a, T> {
        ModuleRegistrar {
            registry: self,
            descriptor,
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<T>> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let value = state.providers.get(&(self.kind, id.to_owned()))?;
        value
            .implementation
            .as_ref()
            .downcast_ref::<Arc<T>>()
            .cloned()
    }

    pub fn get_arc(&self, id: &str) -> Option<Arc<T>> {
        self.get(id)
    }

    pub fn owner(&self, id: &str) -> Option<CoreOwnerKey> {
        self.module_descriptor(id).map(|module| module.owner)
    }

    pub fn module_descriptor(&self, id: &str) -> Option<ModuleDescriptor> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        state
            .providers
            .get(&(self.kind, id.to_owned()))
            .map(|registration| registration.module.clone())
    }

    pub fn descriptor(&self, id: &str) -> Option<ProviderDescriptor> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        state
            .providers
            .get(&(self.kind, id.to_owned()))
            .map(|registration| registration.descriptor.clone())
    }

    pub fn matched_candidates(
        &self,
        event: Option<&str>,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<ProviderCandidate> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let mut candidates = state
            .providers
            .iter()
            .filter_map(|((kind, id), registration)| {
                if *kind != self.kind {
                    return None;
                }
                let event_matches = match event {
                    Some(event) => registration.events.contains(event),
                    None => registration.events.is_empty(),
                };
                (event_matches
                    && ProviderMatcher::matches(
                        &registration.descriptor.r#match,
                        targets,
                        directory,
                        selection_count,
                    ))
                .then(|| ProviderCandidate {
                    id: id.clone(),
                    kind: *kind,
                    priority: registration.descriptor.priority,
                    module_id: registration.module.id.clone(),
                })
            })
            .collect::<Vec<_>>();
        sort_candidates(&mut candidates);
        candidates
    }

    pub fn event_candidates(
        &self,
        event: &str,
        targets: &[MatchResource],
        directory: Option<&MatchResource>,
        selection_count: usize,
    ) -> Vec<ProviderCandidate> {
        self.matched_candidates(Some(event), targets, directory, selection_count)
    }

    pub fn candidates(&self, mut eligible: impl FnMut(&T) -> bool) -> Vec<ProviderCandidate> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let mut candidates = state
            .providers
            .iter()
            .filter_map(|((kind, id), registration)| {
                if *kind != self.kind {
                    return None;
                }
                let implementation = registration
                    .implementation
                    .as_ref()
                    .downcast_ref::<Arc<T>>()?;
                eligible(implementation.as_ref()).then(|| ProviderCandidate {
                    id: id.clone(),
                    kind: *kind,
                    priority: registration.descriptor.priority,
                    module_id: registration.module.id.clone(),
                })
            })
            .collect::<Vec<_>>();
        sort_candidates(&mut candidates);
        candidates
    }

    pub fn len(&self) -> usize {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        state
            .providers
            .keys()
            .filter(|(kind, _)| *kind == self.kind)
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the registered VFS provider ID for a declared `scheme://` URI.
    pub fn route_uri(&self, path: &str) -> Option<&'static str> {
        (self.kind == ProviderKind::Vfs)
            .then(|| self.runtime.resolve_uri(path))
            .flatten()
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        let state = self
            .runtime
            .state
            .read()
            .unwrap_or_else(|error| error.into_inner());
        state
            .providers
            .iter()
            .filter(|((kind, _), _)| *kind == self.kind)
            .map(|(_, registration)| registration.descriptor.clone())
            .collect()
    }
}

fn sort_candidates(candidates: &mut [ProviderCandidate]) {
    candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
}
