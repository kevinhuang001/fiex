//! Process-local registry storage and validation.
use super::types::{ErasedImplementation, Registration, RegistrationRequest, RegistryState};
use fiex_provider_contracts::{ProviderDescriptor, ProviderKind};
use std::{
    collections::BTreeSet,
    sync::{Arc, OnceLock, RwLock},
};

/// Process-local registry shared by typed provider views.
#[derive(Default)]
pub struct RuntimeRegistry {
    pub(super) state: RwLock<RegistryState>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn register<T: ?Sized + Send + Sync + 'static>(
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
        if let Some(existing) = state.modules.get(&module.id)
            && (existing.owner != module.owner || existing.version != module.version)
        {
            return Err(format!("conflicting module descriptor: {}", module.id));
        }

        let key = (kind, descriptor.id.clone());
        if state.providers.contains_key(&key) {
            return Err(format!("duplicate provider ID for {kind:?}: {id}"));
        }
        if let Some((scheme, _)) = uri_route
            && state.uri_routes.contains_key(scheme)
        {
            return Err(format!("duplicate VFS URI scheme: {scheme}"));
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

    pub(super) fn register_uri_alias(
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

    pub(super) fn resolve_uri(&self, path: &str) -> Option<&'static str> {
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
