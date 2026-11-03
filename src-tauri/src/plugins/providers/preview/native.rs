//! Persistent, typed native PreviewProvider registrations. Every Core
//! preview implementation lives behind the same validated descriptor shape
//! as its IPC peer; only the transport-specific adapter differs.
use super::builtin::{builtin_view, Builtin};
use crate::plugins::events::Resolver;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
#[cfg(test)]
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{
    LocationMatch, MatchResource, PreviewProvider, PreviewView, ProviderError, ProviderInvocation,
    ProviderKind, ProviderMatchExclude, ProviderMatchRule, ResourceMetadata,
};
#[cfg(test)]
use serde_json::json;
use std::sync::{Arc, OnceLock};

struct BuiltinPreview(Builtin);
impl PreviewProvider for BuiltinPreview {
    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<PreviewView, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        let path = if self.0 == Builtin::Fallback {
            resource.local.clone()
        } else {
            let user_action = NativeProviderInvocation::user_action(ctx)?;
            resource
                .export_path(user_action)
                .map_err(ProviderError::internal)?
        };
        let view =
            builtin_view(self.0, &path, &resource.display).map_err(ProviderError::internal)?;
        serde_json::from_value(view).map_err(|error| ProviderError::internal(error.to_string()))
    }
}

fn application_view_exclude() -> ProviderMatchExclude {
    ProviderMatchExclude {
        view: Some(LocationMatch {
            provider_ids: Some(vec!["core:applications".into()]),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn PreviewProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn PreviewProvider>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn PreviewProvider> =
            NativeProviderRegistry::new(ProviderKind::Preview);
        for kind in [
            Builtin::Text,
            Builtin::Image,
            Builtin::Media,
            Builtin::Pdf,
            Builtin::Fallback,
        ] {
            let rule = match kind {
                Builtin::Text => ProviderMatchRule {
                    mime_types: vec![
                        "text/*".into(),
                        "application/json".into(),
                        "application/xml".into(),
                        "application/javascript".into(),
                    ],
                    kinds: vec!["file".into()],
                    exclude: Some(application_view_exclude()),
                    ..Default::default()
                },
                Builtin::Image => ProviderMatchRule {
                    mime_types: vec!["image/*".into()],
                    kinds: vec!["file".into()],
                    exclude: Some(application_view_exclude()),
                    ..Default::default()
                },
                Builtin::Media => ProviderMatchRule {
                    mime_types: vec!["audio/*".into(), "video/*".into()],
                    kinds: vec!["file".into()],
                    exclude: Some(application_view_exclude()),
                    ..Default::default()
                },
                Builtin::Pdf => ProviderMatchRule {
                    mime_types: vec!["application/pdf".into()],
                    kinds: vec!["file".into()],
                    exclude: Some(application_view_exclude()),
                    ..Default::default()
                },
                Builtin::Fallback => ProviderMatchRule {
                    exclude: Some(application_view_exclude()),
                    ..Default::default()
                },
            };
            values
                .register(
                    kind.id(),
                    kind.priority(),
                    rule,
                    Arc::new(BuiltinPreview(kind)),
                )
                .expect("invalid native PreviewProvider");
        }
        values
    })
}

pub(super) fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Resolver> {
    let inferred_mime = if mime.is_empty() || mime == "application/octet-stream" {
        crate::plugins::match_rules::mime_for(ext)
    } else {
        mime.to_owned()
    };
    let targets = [MatchResource {
        metadata: Some(ResourceMetadata {
            extension: Some(ext.into()),
            mime: Some(inferred_mime),
            kind: if is_dir { "directory" } else { "file" }.into(),
            ..Default::default()
        }),
        ..Default::default()
    }];
    candidates_for(&targets)
}

pub(super) fn candidates_for(targets: &[MatchResource]) -> Vec<Resolver> {
    let mut candidates = registry().matched_candidates(targets, None, targets.len());
    if candidates
        .iter()
        .any(|provider| provider.id != "builtin:fallback")
    {
        candidates.retain(|provider| provider.id != "builtin:fallback");
    }
    candidates
}

pub(super) fn fallback_matches(targets: &[MatchResource]) -> bool {
    registry()
        .matched_candidates(targets, None, targets.len())
        .iter()
        .any(|provider| provider.id == "builtin:fallback")
}

#[cfg(test)]
pub(super) fn open(id: &str, resource: &ResourceSnapshot) -> Result<PreviewView, ProviderError> {
    let provider = registry()
        .get(id)
        .ok_or_else(|| ProviderError::method_not_found("preview.open"))?;
    let invocation = NativeProviderInvocation::new(
        "preview.open",
        json!({}),
        std::slice::from_ref(resource),
        None,
    )
    .with_user_action_context(
        crate::core::fs::mapped_access::UserActionContext::from_desktop_command(),
    );
    provider.open(&invocation)
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn PreviewProvider>> {
    registry().get_arc(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_preview_registry_has_five_distinct_typed_implementations() {
        assert_eq!(registry().len(), 5);
        for descriptor in registry().descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::Preview);
            descriptor.validate().unwrap();
        }
        let image = candidates("png", "image/png", false);
        assert!(image.iter().any(|provider| provider.id == "builtin:image"));
        let directory = candidates("", "", true);
        assert_eq!(directory.len(), 1);
        assert_eq!(directory[0].id, "builtin:fallback");
        let resource = ResourceSnapshot::from_metadata("/unavailable", Default::default());
        assert!(open("not-registered", &resource).is_err());
        assert_eq!(
            open("builtin:fallback", &resource).unwrap().kind,
            "fallback"
        );
    }

    #[test]
    fn native_previews_exclude_applications_but_keep_trash_and_recent() {
        let target = MatchResource {
            metadata: Some(ResourceMetadata {
                kind: "file".into(),
                extension: Some("png".into()),
                mime: Some("image/png".into()),
                ..Default::default()
            }),
            view: Some(fiex_provider_contracts::ResourceLocation {
                resource_type: fiex_provider_contracts::ResourceType::Vfs,
                provider_id: "core:applications".into(),
                mount_id: Some("core:applications".into()),
            }),
            ..Default::default()
        };
        assert!(candidates_for(std::slice::from_ref(&target)).is_empty());
        assert!(!fallback_matches(std::slice::from_ref(&target)));
        let recent = MatchResource {
            metadata: Some(ResourceMetadata {
                kind: "file".into(),
                extension: Some("png".into()),
                mime: Some("image/png".into()),
                ..Default::default()
            }),
            view: Some(fiex_provider_contracts::ResourceLocation {
                resource_type: fiex_provider_contracts::ResourceType::Vfs,
                provider_id: "core:recent".into(),
                mount_id: Some("core:recent".into()),
            }),
            ..Default::default()
        };
        assert!(candidates_for(std::slice::from_ref(&recent))
            .iter()
            .any(|provider| provider.id == "builtin:image"));
        assert!(fallback_matches(std::slice::from_ref(&recent)));
        let trash = MatchResource {
            view: Some(fiex_provider_contracts::ResourceLocation {
                resource_type: fiex_provider_contracts::ResourceType::Vfs,
                provider_id: "core:trash".into(),
                mount_id: Some("core:trash".into()),
            }),
            ..recent.clone()
        };
        assert!(candidates_for(std::slice::from_ref(&trash))
            .iter()
            .any(|provider| provider.id == "builtin:image"));
        assert!(fallback_matches(std::slice::from_ref(&trash)));
        let ordinary = MatchResource {
            metadata: Some(ResourceMetadata {
                kind: "file".into(),
                extension: Some("png".into()),
                mime: Some("image/png".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(fallback_matches(std::slice::from_ref(&ordinary)));
    }
}
