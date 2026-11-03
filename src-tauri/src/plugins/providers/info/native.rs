//! Persistent, per-kind Core InfoProvider registry. Native handlers are
//! registered once, then selected with the same validated shared descriptors
//! used to describe SDK provider instances. No native ID dispatch table lives
//! inside the EventRouter or the per-request endpoint.
use super::{media::media_sections, metadata::metadata_section, InfoSection};
use crate::plugins::events::Resolver;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{
    InfoProvider, ProviderError, ProviderInvocation, ProviderKind, ProviderMatchRule,
};
use std::sync::{Arc, OnceLock};

struct MetadataInfo;
impl InfoProvider for MetadataInfo {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        Ok(vec![metadata_section(
            &resource.display,
            &resource.local,
            resource.stat.as_ref(),
            resource.metadata(),
        )])
    }
}

struct MediaInfo;
impl InfoProvider for MediaInfo {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        let user_action = NativeProviderInvocation::user_action(ctx)?;
        let path = resource
            .export_path(user_action)
            .map_err(ProviderError::internal)?;
        Ok(media_sections(&path))
    }
}

/// Each registered native implementation has one persistent, typed Arc.
/// The request-scoped ResourceSnapshot never enters this global registry.
fn registry() -> &'static NativeProviderRegistry<dyn InfoProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn InfoProvider>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn InfoProvider> =
            NativeProviderRegistry::new(ProviderKind::Info);
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
                ProviderMatchRule {
                    mime_types: vec!["audio/*".into(), "video/*".into()],
                    kinds: vec!["file".into()],
                    ..Default::default()
                },
                Arc::new(MediaInfo),
            )
            .expect("invalid media InfoProvider");
        values
    })
}

pub(super) fn candidates(resource: &ResourceSnapshot) -> Vec<Resolver> {
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(resource));
    registry().matched_candidates(&targets, None, 1)
}

#[cfg(test)]
pub(super) fn invoke(
    id: &str,
    resource: &ResourceSnapshot,
) -> Result<Vec<InfoSection>, ProviderError> {
    let provider = registry()
        .get(id)
        .ok_or_else(|| ProviderError::method_not_found("info.resolve"))?;
    let invocation = NativeProviderInvocation::new(
        "info.resolve",
        serde_json::json!({}),
        std::slice::from_ref(resource),
        None,
    )
    .with_user_action_context(
        crate::core::fs::mapped_access::UserActionContext::from_desktop_command(),
    );
    provider.resolve(&invocation)
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn InfoProvider>> {
    registry().get_arc(id)
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
        assert!(!candidates(&directory)
            .iter()
            .any(|provider| provider.id == "core.media"));
        assert!(invoke("unknown", &directory).is_err());
        assert_eq!(invoke("core.metadata", &directory).unwrap().len(), 1);
    }

    #[test]
    fn native_info_providers_remain_available_in_core_special_views() {
        for view_id in ["core:applications", "core:trash", "core:recent"] {
            let target = fiex_provider_contracts::MatchResource {
                metadata: Some(crate::plugins::contracts::ResourceMetadata {
                    kind: "file".into(),
                    mime: Some("video/mp4".into()),
                    ..Default::default()
                }),
                view: Some(fiex_provider_contracts::ResourceLocation {
                    resource_type: fiex_provider_contracts::ResourceType::Vfs,
                    provider_id: view_id.into(),
                    mount_id: Some(view_id.into()),
                }),
                ..Default::default()
            };
            let candidates = registry().matched_candidates(&[target], None, 1);
            assert!(candidates
                .iter()
                .any(|provider| provider.id == "core.metadata"));
            assert!(candidates
                .iter()
                .any(|provider| provider.id == "core.media"));
        }
    }
}
