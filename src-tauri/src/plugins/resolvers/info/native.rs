//! Persistent, per-kind Core InfoProvider registry. Native handlers are
//! registered once, then selected with the same validated shared descriptors
//! used to describe SDK provider instances. No native ID dispatch table lives
//! inside the EventRouter or the per-request endpoint.
use super::{media::media_sections, metadata::metadata_section, InfoSection};
use crate::plugins::events::Resolver;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderDescriptor, ProviderKind, ProviderMatchRule};
use std::{collections::BTreeMap, sync::{Arc, OnceLock}};

trait NativeInfoProvider: Send + Sync {
    fn eligible(&self, _resource: &ResourceSnapshot) -> bool {
        true
    }
    fn resolve(
        &self,
        display: &str,
        resource: &ResourceSnapshot,
    ) -> Result<Vec<InfoSection>, String>;
}

struct MetadataInfo;
impl NativeInfoProvider for MetadataInfo {
    fn resolve(
        &self,
        display: &str,
        resource: &ResourceSnapshot,
    ) -> Result<Vec<InfoSection>, String> {
        Ok(vec![metadata_section(
            display,
            &resource.local,
            resource.stat.as_ref(),
            resource.metadata(),
        )])
    }
}

struct MediaInfo;
impl NativeInfoProvider for MediaInfo {
    fn eligible(&self, resource: &ResourceSnapshot) -> bool {
        !resource.is_dir
            && (resource.mime.starts_with("audio/") || resource.mime.starts_with("video/"))
    }
    fn resolve(
        &self,
        _display: &str,
        resource: &ResourceSnapshot,
    ) -> Result<Vec<InfoSection>, String> {
        Ok(media_sections(&resource.export_path()?))
    }
}

struct NativeInfoRegistration {
    descriptor: ProviderDescriptor,
    implementation: Arc<dyn NativeInfoProvider>,
}

/// Each registered native implementation has one persistent, typed Arc.
/// The request-scoped ResourceSnapshot never enters this global registry.
fn registry() -> &'static BTreeMap<&'static str, NativeInfoRegistration> {
    static REGISTRY: OnceLock<BTreeMap<&'static str, NativeInfoRegistration>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values = BTreeMap::new();
        for (id, priority, implementation) in [
            ("core.metadata", 1000, Arc::new(MetadataInfo) as Arc<dyn NativeInfoProvider>),
            ("core.media", 100, Arc::new(MediaInfo) as Arc<dyn NativeInfoProvider>),
        ] {
            let descriptor = ProviderDescriptor {
                id: id.into(),
                kind: ProviderKind::Info,
                priority,
                r#match: ProviderMatchRule::default(),
            };
            descriptor.validate().expect("invalid native InfoProvider registration");
            assert!(values.insert(id, NativeInfoRegistration {
                descriptor,
                implementation,
            }).is_none(), "duplicate native InfoProvider");
        }
        values
    })
}

pub(super) fn candidates(resource: &ResourceSnapshot) -> Vec<Resolver> {
    registry()
        .iter()
        .filter(|(_, registration)| registration.implementation.eligible(resource))
        .map(|(id, registration)| Resolver::builtin(id, registration.descriptor.priority))
        .collect()
}

pub(super) fn invoke(
    id: &str,
    display: &str,
    resource: &ResourceSnapshot,
) -> Result<Vec<InfoSection>, String> {
    let provider = registry().get(id).ok_or("unknown native Info provider")?;
    if !provider.implementation.eligible(resource) {
        return Err("native Info provider does not match resource".into());
    }
    provider.implementation.resolve(display, resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_registry_keeps_distinct_typed_handlers_and_rejects_ineligible_media() {
        let values = registry();
        assert_eq!(values.len(), 2);
        for (id, value) in values {
            assert_eq!(value.descriptor.id, *id);
            assert_eq!(value.descriptor.kind, ProviderKind::Info);
            value.descriptor.validate().unwrap();
        }
        let directory = ResourceSnapshot::from_metadata(
            "/no-such-directory",
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        assert_eq!(candidates(&directory).len(), 1);
        assert!(invoke("core.media", "/no-such-directory", &directory).is_err());
        assert!(invoke("unknown", "/no-such-directory", &directory).is_err());
        assert_eq!(invoke("core.metadata", "/no-such-directory", &directory).unwrap().len(), 1);
    }
}
