//! Persistent, typed native PreviewProvider registrations. Every Core
//! preview implementation lives behind the same validated descriptor shape
//! as its IPC peer; only the transport-specific adapter differs.
use super::builtin::{builtin_view, Builtin};
use crate::plugins::events::Resolver;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{
    MatchResource, PreviewProvider, PreviewView, ProviderError, ProviderInvocation,
    ProviderKind, ProviderMatchRule, ResourceMetadata,
};
use serde_json::json;
use std::sync::{Arc, OnceLock};

struct BuiltinPreview(Builtin);
impl PreviewProvider for BuiltinPreview {
    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<PreviewView, ProviderError> {
        let resource = NativeProviderInvocation::resource(ctx, 0)?;
        let path = if self.0 == Builtin::Fallback {
            resource.local.clone()
        } else {
            resource.export_path().map_err(ProviderError::internal)?
        };
        let view =
            builtin_view(self.0, &path, &resource.display).map_err(ProviderError::internal)?;
        serde_json::from_value(view).map_err(|error| ProviderError::internal(error.to_string()))
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
                    ..Default::default()
                },
                Builtin::Image => ProviderMatchRule {
                    mime_types: vec!["image/*".into()],
                    kinds: vec!["file".into()],
                    ..Default::default()
                },
                Builtin::Media => ProviderMatchRule {
                    mime_types: vec!["audio/*".into(), "video/*".into()],
                    kinds: vec!["file".into()],
                    ..Default::default()
                },
                Builtin::Pdf => ProviderMatchRule {
                    mime_types: vec!["application/pdf".into()],
                    kinds: vec!["file".into()],
                    ..Default::default()
                },
                Builtin::Fallback => ProviderMatchRule::default(),
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
    let mut candidates = registry().matched_candidates(&targets, None, 1);
    if candidates
        .iter()
        .any(|provider| provider.id != "builtin:fallback")
    {
        candidates.retain(|provider| provider.id != "builtin:fallback");
    }
    candidates
}

pub(super) fn open(id: &str, resource: &ResourceSnapshot) -> Result<PreviewView, ProviderError> {
    let provider = registry()
        .get(id)
        .ok_or_else(|| ProviderError::method_not_found("preview.open"))?;
    let invocation = NativeProviderInvocation::new(
        "preview.open",
        json!({}),
        std::slice::from_ref(resource),
        None,
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
}
