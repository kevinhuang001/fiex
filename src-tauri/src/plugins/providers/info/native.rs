//! Persistent, per-kind Core InfoProvider registry. Native handlers are
//! registered once, then selected with the same validated shared descriptors
//! used to describe SDK provider instances. No native ID dispatch table lives
//! inside the EventRouter or the per-request endpoint.
use crate::plugins::events::Resolver;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::resource_context::{ResourceSnapshot, ResourceSnapshotProjection};
use fiex_provider_contracts::{
    InfoProvider, InfoSectionDescriptor, ProviderError, ProviderInvocation, ProviderKind,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;

struct NativeInfoServices;

impl fiex_info::InfoModuleServices for NativeInfoServices {
    fn metadata_input(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<fiex_info::InfoMetadataInput, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        Ok(fiex_info::InfoMetadataInput {
            display: resource.display.clone(),
            local: resource.local.clone(),
            stat: resource.stat.clone(),
            metadata: resource.metadata().cloned(),
            is_local: crate::core::fs::provider_registry::resource_access(&resource.display)
                .is_ok_and(|access| {
                    access == crate::core::fs::provider_registry::ResourceAccess::HostPath
                }),
            cancelled: NativeProviderInvocation::cancellation(ctx)
                .unwrap_or_else(|| Arc::new(std::sync::atomic::AtomicBool::new(false))),
        })
    }

    fn media_descriptor_path(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<std::path::PathBuf, ProviderError> {
        Ok(NativeProviderInvocation::resource(ctx, 0)?.local.clone())
    }

    fn media_path(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<std::path::PathBuf, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        let user_action = NativeProviderInvocation::user_action(ctx)?;
        resource
            .export_path(user_action)
            .map_err(ProviderError::internal)
    }

    fn localization(&self) -> fiex_info::Localization {
        *super::localization()
    }
}

/// Each registered native implementation has one persistent, typed Arc.
/// The request-scoped ResourceSnapshot never enters this global registry.
pub(super) fn register(values: &mut NativeProviderRegistry<dyn InfoProvider>) {
    let mut module = values.module(ModuleDescriptor::new(
        fiex_info::MODULE_ID,
        CoreOwnerKey::new("core"),
        env!("CARGO_PKG_VERSION"),
    ));
    for registration in fiex_info::InfoModule::new(Arc::new(NativeInfoServices)).registrations() {
        module
            .register(
                registration.id,
                registration.priority,
                registration.rule,
                registration.provider,
            )
            .expect("invalid native InfoProvider");
    }
}

fn registry() -> NativeProviderRegistry<dyn InfoProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::Info)
}

pub(super) fn candidates(resource: &ResourceSnapshot) -> Vec<Resolver> {
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(resource));
    registry().matched_candidates(&targets, None, 1)
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn InfoProvider>> {
    registry().get_arc(id)
}

pub(super) fn sections(
    id: &str,
    resource: &ResourceSnapshot,
) -> Result<Vec<InfoSectionDescriptor>, String> {
    let invocation = NativeProviderInvocation::new(
        "info.compute",
        serde_json::json!({}),
        std::slice::from_ref(resource),
        None,
    );
    fiex_info::InfoModule::new(Arc::new(NativeInfoServices))
        .sections_for(id, &invocation)
        .map_err(|error| error.to_string())
}

pub(super) fn invocation_with_cancel(
    id: &str,
    method: impl Into<String>,
    params: serde_json::Value,
    resource: &ResourceSnapshot,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
) -> Result<NativeProviderInvocation, String> {
    Ok(registry()
        .invocation(id, method, params, std::slice::from_ref(resource), None)?
        .with_cancellation(cancelled))
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
        let directory = crate::plugins::resource::resource_context::from_metadata(
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
        let descriptions = sections("core.metadata", &directory).unwrap();
        assert!(descriptions.iter().any(|section| section.id == "basic"));
        assert!(descriptions
            .iter()
            .any(|section| section.id == "directoryStats"));
        assert!(!descriptions
            .iter()
            .flat_map(|section| &section.fields)
            .any(|field| matches!(field.id.as_str(), "extension" | "mime" | "size")));
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
