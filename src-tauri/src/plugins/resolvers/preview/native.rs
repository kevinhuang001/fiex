//! Persistent, typed native PreviewProvider registrations. Every Core
//! preview implementation lives behind the same validated descriptor shape
//! as its IPC peer; only the transport-specific adapter differs.
use super::builtin::{builtin_candidates, builtin_view, Builtin};
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

trait NativePreviewProvider: Send + Sync {
    fn supports(&self, kinds: &[Builtin]) -> bool;
    fn open(&self, resource: &ResourceSnapshot) -> Result<Value, String>;
}

struct BuiltinPreview(Builtin);
impl NativePreviewProvider for BuiltinPreview {
    fn supports(&self, kinds: &[Builtin]) -> bool {
        kinds.contains(&self.0)
    }

    fn open(&self, resource: &ResourceSnapshot) -> Result<Value, String> {
        let path = if self.0 == Builtin::Fallback {
            resource.local.clone()
        } else {
            resource.export_path()?
        };
        builtin_view(self.0, &path, &resource.display)
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn NativePreviewProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn NativePreviewProvider>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values = NativeProviderRegistry::new(ProviderKind::Preview);
        for kind in [
            Builtin::Text,
            Builtin::Image,
            Builtin::Media,
            Builtin::Pdf,
            Builtin::Fallback,
        ] {
            values
                .register(
                    kind.id(),
                    kind.priority(),
                    ProviderMatchRule::default(),
                    Arc::new(BuiltinPreview(kind)),
                )
                .expect("invalid native PreviewProvider");
        }
        values
    })
}

pub(super) fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Resolver> {
    let selected = builtin_candidates(ext, mime, is_dir);
    registry().candidates(|provider| {
        // BuiltinPreview is the registered implementation; its eligibility
        // remains a native capability, never a supplied provider identity.
        provider.supports(&selected)
    })
}

pub(super) fn open(id: &str, resource: &ResourceSnapshot) -> Result<Value, String> {
    registry()
        .get(id)
        .ok_or("unknown native PreviewProvider")?
        .open(resource)
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
        let resource = ResourceSnapshot::from_metadata(
            "/unavailable",
            Default::default(),
        );
        assert!(open("not-registered", &resource).is_err());
        assert_eq!(open("builtin:fallback", &resource).unwrap()["kind"], "fallback");
    }
}
