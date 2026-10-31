//! Persistent, per-kind Core InfoProvider registry. Native handlers are
//! registered once, then selected with the same validated shared descriptors
//! used to describe SDK provider instances. No native ID dispatch table lives
//! inside the EventRouter or the per-request endpoint.
use super::{media::media_sections, metadata::metadata_section, InfoSection};
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use std::sync::{Arc, OnceLock};

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

/// Each registered native implementation has one persistent, typed Arc.
/// The request-scoped ResourceSnapshot never enters this global registry.
fn registry() -> &'static NativeProviderRegistry<dyn NativeInfoProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn NativeInfoProvider>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values = NativeProviderRegistry::new(ProviderKind::Info);
        values
            .register(
                "core.metadata",
                1000,
                ProviderMatchRule::default(),
                Arc::new(MetadataInfo),
            )
            .expect("invalid metadata InfoProvider");
        values
            .register(
                "core.media",
                100,
                ProviderMatchRule::default(),
                Arc::new(MediaInfo),
            )
            .expect("invalid media InfoProvider");
        values
    })
}

pub(super) fn candidates(resource: &ResourceSnapshot) -> Vec<Resolver> {
    registry().candidates(|provider| provider.eligible(resource))
}

pub(super) fn invoke(
    id: &str,
    display: &str,
    resource: &ResourceSnapshot,
) -> Result<Vec<InfoSection>, String> {
    let provider = registry().get(id).ok_or("unknown native Info provider")?;
    if !provider.eligible(resource) {
        return Err("native Info provider does not match resource".into());
    }
    provider.resolve(display, resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_registry_keeps_distinct_typed_handlers_and_rejects_ineligible_media() {
        let values = registry();
        assert_eq!(values.len(), 2);
        for descriptor in values.descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::Info);
            descriptor.validate().unwrap();
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
        assert_eq!(
            invoke("core.metadata", "/no-such-directory", &directory)
                .unwrap()
                .len(),
            1
        );
    }
}
