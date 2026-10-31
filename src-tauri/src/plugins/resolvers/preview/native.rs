//! Persistent, typed native PreviewProvider registrations. Every Core
//! preview implementation lives behind the same validated descriptor shape
//! as its IPC peer; only the transport-specific adapter differs.
use super::builtin::{builtin_candidates, builtin_view, Builtin};
use crate::plugins::events::Resolver;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderDescriptor, ProviderKind, ProviderMatchRule};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

trait NativePreviewProvider: Send + Sync {
    fn open(&self, resource: &ResourceSnapshot) -> Result<Value, String>;
}

struct BuiltinPreview(Builtin);
impl NativePreviewProvider for BuiltinPreview {
    fn open(&self, resource: &ResourceSnapshot) -> Result<Value, String> {
        let path = if self.0 == Builtin::Fallback {
            resource.local.clone()
        } else {
            resource.export_path()?
        };
        builtin_view(self.0, &path, &resource.display)
    }
}

struct Registration {
    descriptor: ProviderDescriptor,
    implementation: Arc<dyn NativePreviewProvider>,
}

fn registry() -> &'static BTreeMap<&'static str, Registration> {
    static REGISTRY: OnceLock<BTreeMap<&'static str, Registration>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values = BTreeMap::new();
        for kind in [
            Builtin::Text,
            Builtin::Image,
            Builtin::Media,
            Builtin::Pdf,
            Builtin::Fallback,
        ] {
            let descriptor = ProviderDescriptor {
                id: kind.id().into(),
                kind: ProviderKind::Preview,
                priority: kind.priority(),
                r#match: ProviderMatchRule::default(),
            };
            descriptor.validate().expect("invalid native PreviewProvider");
            assert!(values
                .insert(kind.id(), Registration {
                    descriptor,
                    implementation: Arc::new(BuiltinPreview(kind)),
                })
                .is_none(), "duplicate native PreviewProvider");
        }
        values
    })
}

pub(super) fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Resolver> {
    builtin_candidates(ext, mime, is_dir)
        .into_iter()
        .filter_map(|kind| {
            registry()
                .get(kind.id())
                .map(|provider| Resolver::builtin(kind.id(), provider.descriptor.priority))
        })
        .collect()
}

pub(super) fn open(id: &str, resource: &ResourceSnapshot) -> Result<Value, String> {
    registry()
        .get(id)
        .ok_or("unknown native PreviewProvider")?
        .implementation
        .open(resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_preview_registry_has_five_distinct_typed_implementations() {
        assert_eq!(registry().len(), 5);
        for (id, entry) in registry() {
            assert_eq!(entry.descriptor.id, *id);
            assert_eq!(entry.descriptor.kind, ProviderKind::Preview);
            entry.descriptor.validate().unwrap();
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
