use fiex_provider_contracts::{
    MatchResource, ProviderDescriptor, ProviderKind, ProviderMatchRule, ProviderMatcher,
};
use fiex_storage::CoreOwnerKey;
use std::{
    any::Any,
    collections::{BTreeSet, HashMap},
    sync::{Arc, OnceLock, RwLock},
};

type ErasedImplementation = Arc<dyn Any + Send + Sync>;

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

    fn validate(&self) -> Result<(), String> {
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

struct Registration {
    descriptor: ProviderDescriptor,
    module: ModuleDescriptor,
    events: BTreeSet<String>,
    implementation: ErasedImplementation,
}

struct RegistrationRequest<T: ?Sized> {
    kind: ProviderKind,
    module: ModuleDescriptor,
    id: String,
    uri_route: Option<(&'static str, &'static str)>,
    event: Option<String>,
    priority: i64,
    rule: ProviderMatchRule,
    implementation: Arc<T>,
}

#[derive(Default)]
struct RegistryState {
    modules: HashMap<String, ModuleDescriptor>,
    providers: HashMap<(ProviderKind, String), Registration>,
    uri_routes: HashMap<&'static str, &'static str>,
}

/// Process-local registry shared by typed provider views.
#[derive(Default)]
pub struct RuntimeRegistry {
    state: RwLock<RegistryState>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn register<T: ?Sized + Send + Sync + 'static>(
        &self,
        request: RegistrationRequest<T>,
    ) -> Result<(), String> {
        let RegistrationRequest {
            kind,
            module,
            id,
            uri_route,
            event,
            priority,
            rule,
            implementation,
        } = request;
        module.validate()?;
        if event.as_deref().is_some_and(str::is_empty) {
            return Err("provider event is empty".into());
        }

        let descriptor = ProviderDescriptor {
            id: id.clone(),
            kind,
            priority,
            r#match: rule,
        };
        descriptor.validate()?;
        if let Some((scheme, provider_id)) = uri_route {
            validate_uri_scheme(scheme)?;
            if kind != ProviderKind::Vfs || provider_id != id {
                return Err("URI routes can only be registered by their VFS provider".into());
            }
        }

        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(existing) = state.modules.get(&module.id) {
            if existing.owner != module.owner || existing.version != module.version {
                return Err(format!("conflicting module descriptor: {}", module.id));
            }
        }

        let key = (kind, descriptor.id.clone());
        if state.providers.contains_key(&key) {
            return Err(format!("duplicate provider ID for {kind:?}: {id}"));
        }
        if let Some((scheme, _)) = uri_route {
            if state.uri_routes.contains_key(scheme) {
                return Err(format!("duplicate VFS URI scheme: {scheme}"));
            }
        }

        let mut events = BTreeSet::new();
        if let Some(event) = event {
            events.insert(event);
        }
        let implementation: ErasedImplementation = Arc::new(implementation);
        state
            .modules
            .entry(module.id.clone())
            .or_insert_with(|| module.clone());
        state.providers.insert(
            key,
            Registration {
                descriptor,
                module,
                events,
                implementation,
            },
        );
        if let Some((scheme, provider_id)) = uri_route {
            state.uri_routes.insert(scheme, provider_id);
        }
        Ok(())
    }

    fn register_uri_alias(
        &self,
        kind: ProviderKind,
        module_id: &str,
        provider_id: &'static str,
        scheme: &'static str,
    ) -> Result<(), String> {
        validate_uri_scheme(scheme)?;
        if kind != ProviderKind::Vfs {
            return Err("URI routes can only be registered for VFS providers".into());
        }
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let key = (kind, provider_id.to_owned());
        let registration = state
            .providers
            .get(&key)
            .ok_or_else(|| format!("cannot route an unregistered VFS provider: {provider_id}"))?;
        if registration.module.id != module_id {
            return Err("a module can route only its own VFS providers".into());
        }
        if state.uri_routes.contains_key(scheme) {
            return Err(format!("duplicate VFS URI scheme: {scheme}"));
        }
        state.uri_routes.insert(scheme, provider_id);
        Ok(())
    }

    fn resolve_uri(&self, path: &str) -> Option<&'static str> {
        let (scheme, _) = path.split_once("://")?;
        self.state
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .uri_routes
            .get(scheme)
            .copied()
    }
}

fn validate_uri_scheme(scheme: &str) -> Result<(), String> {
    if scheme.is_empty()
        || !scheme.as_bytes()[0].is_ascii_lowercase()
        || !scheme.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')
        })
    {
        return Err("invalid VFS URI scheme".into());
    }
    Ok(())
}

/// Return the one process-wide registry used by native provider modules.
pub fn global_runtime_registry() -> &'static Arc<RuntimeRegistry> {
    static REGISTRY: OnceLock<Arc<RuntimeRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(RuntimeRegistry::new()))
}

/// Typed view over a registry. It prevents retrieving an instance under the
/// wrong ProviderKind and keeps the implementation trait object typed.
pub struct TypedProviderRegistry<T: ?Sized + Send + Sync + 'static> {
    runtime: Arc<RuntimeRegistry>,
    kind: ProviderKind,
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

    pub fn register(
        &mut self,
        id: &str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.register_owned(
            id,
            CoreOwnerKey::new("core"),
            priority,
            rule,
            implementation,
        )
    }

    pub fn register_owned(
        &mut self,
        id: &str,
        owner: CoreOwnerKey,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        let module = ModuleDescriptor::new(id, owner, env!("CARGO_PKG_VERSION"));
        self.runtime.register(RegistrationRequest {
            kind: self.kind,
            module,
            id: id.into(),
            uri_route: None,
            event: None,
            priority,
            rule,
            implementation,
        })
    }

    pub fn register_for_event(
        &mut self,
        id: &str,
        event: &str,
        owner: CoreOwnerKey,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        let module = ModuleDescriptor::new(id, owner, env!("CARGO_PKG_VERSION"));
        self.runtime.register(RegistrationRequest {
            kind: self.kind,
            module,
            id: id.into(),
            uri_route: None,
            event: Some(event.into()),
            priority,
            rule,
            implementation,
        })
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

pub struct ModuleRegistrar<'a, T: ?Sized + Send + Sync + 'static> {
    registry: &'a mut TypedProviderRegistry<T>,
    descriptor: ModuleDescriptor,
}

impl<T: ?Sized + Send + Sync + 'static> ModuleRegistrar<'_, T> {
    pub fn register(
        &mut self,
        id: &str,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.register_for_event(id, None, priority, rule, implementation)
    }

    pub fn register_for_event(
        &mut self,
        id: &str,
        event: Option<&str>,
        priority: i64,
        rule: ProviderMatchRule,
        implementation: Arc<T>,
    ) -> Result<(), String> {
        self.registry.runtime.register(RegistrationRequest {
            kind: self.registry.kind,
            module: self.descriptor.clone(),
            id: id.into(),
            uri_route: None,
            event: event.map(Into::into),
            priority,
            rule,
            implementation,
        })
    }

    /// Declare a URI scheme for one of this module's VFS providers.
    pub fn register_vfs_uri(
        &mut self,
        provider_id: &'static str,
        scheme: &'static str,
    ) -> Result<(), String> {
        self.registry.runtime.register_uri_alias(
            self.registry.kind,
            &self.descriptor.id,
            provider_id,
            scheme,
        )
    }
}

fn sort_candidates(candidates: &mut [ProviderCandidate]) {
    candidates.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Handler: Send + Sync {
        fn call(&self) -> u32;
    }

    struct TestHandler(u32);

    impl Handler for TestHandler {
        fn call(&self) -> u32 {
            self.0
        }
    }

    #[test]
    fn module_registrar_shares_owner_and_keeps_provider_types() {
        let owner = CoreOwnerKey::new("recent");
        let mut hooks: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::new(ProviderKind::Hook);
        let module = ModuleDescriptor::new("core:recent", owner, "1");
        let mut registrar = hooks.module(module);
        registrar
            .register_for_event(
                "core:recent.hook",
                Some("afterOpen"),
                100,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(7)),
            )
            .unwrap();

        let value = hooks.get("core:recent.hook").unwrap();
        assert_eq!(value.call(), 7);
        assert_eq!(hooks.owner("core:recent.hook"), Some(owner));
        assert!(hooks
            .event_candidates("beforeOpen", &[], None, 1)
            .is_empty());
        let candidates = hooks.event_candidates("afterOpen", &[], None, 1);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].module_id, "core:recent");
    }

    #[test]
    fn typed_registry_rejects_duplicate_ids_and_isolates_kinds() {
        let runtime = Arc::new(RuntimeRegistry::new());
        let mut hooks: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::from_runtime(Arc::clone(&runtime), ProviderKind::Hook);
        let mut info: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::from_runtime(runtime, ProviderKind::Info);
        hooks
            .register(
                "builtin:test",
                1,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(1)),
            )
            .unwrap();
        assert!(hooks
            .register(
                "builtin:test",
                2,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(2))
            )
            .is_err());
        info.register(
            "builtin:test",
            1,
            ProviderMatchRule::default(),
            Arc::new(TestHandler(3)),
        )
        .unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(info.len(), 1);
        assert_eq!(info.get("builtin:test").unwrap().call(), 3);
    }

    #[test]
    fn vfs_uri_routes_are_declared_by_registered_modules() {
        let mut vfs: TypedProviderRegistry<dyn Handler> =
            TypedProviderRegistry::new(ProviderKind::Vfs);
        {
            let mut module = vfs.module(ModuleDescriptor::new(
                "core:recent",
                CoreOwnerKey::new("recent"),
                "1",
            ));
            module
                .register(
                    "core:recent",
                    100,
                    ProviderMatchRule::default(),
                    Arc::new(TestHandler(1)),
                )
                .unwrap();
            module.register_vfs_uri("core:recent", "recent").unwrap();
            assert!(module
                .register_vfs_uri("core:recent", "bad/scheme")
                .is_err());
        }
        assert_eq!(vfs.route_uri("recent://item"), Some("core:recent"));
        assert_eq!(vfs.route_uri("unknown://item"), None);

        let mut second = vfs.module(ModuleDescriptor::new(
            "core:other",
            CoreOwnerKey::new("other"),
            "1",
        ));
        second
            .register(
                "core:other",
                100,
                ProviderMatchRule::default(),
                Arc::new(TestHandler(2)),
            )
            .unwrap();
        assert!(second.register_vfs_uri("core:other", "recent").is_err());
    }
}
