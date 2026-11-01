//! Persistent Core decorations, routed through the same verified Provider
//! Descriptor and EventRouter as SDK-backed DecorationProviders.
use super::{
    builtin::{self, NativeKind},
    types::{Decoration, DecorationEntry},
};
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{LocationMatch, ProviderKind, ProviderMatchRule};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

trait NativeDecorationProvider: Send + Sync {
    fn resolve(
        &self,
        directory: &str,
        entries: &HashMap<String, DecorationEntry>,
    ) -> Vec<Decoration>;
}

struct BuiltinDecoration(NativeKind);
impl NativeDecorationProvider for BuiltinDecoration {
    fn resolve(
        &self,
        directory: &str,
        entries: &HashMap<String, DecorationEntry>,
    ) -> Vec<Decoration> {
        builtin::resolve_in(self.0, directory, entries)
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn NativeDecorationProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn NativeDecorationProvider>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn NativeDecorationProvider> =
            NativeProviderRegistry::new(ProviderKind::Decoration);
        for (id, priority, kind) in [
            ("builtin:tags", -12, NativeKind::Tags),
            ("builtin:readonly", -11, NativeKind::Readonly),
            (
                "builtin:connection-status",
                -10,
                NativeKind::ConnectionStatus,
            ),
            ("builtin:application-icon", -9, NativeKind::ApplicationIcon),
        ] {
            let rule = match kind {
                NativeKind::Tags => ProviderMatchRule {
                    storage: Some(LocationMatch {
                        types: Some(vec!["fs".into()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                NativeKind::ConnectionStatus => ProviderMatchRule {
                    storage: Some(LocationMatch {
                        types: Some(vec!["vfs".into()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                NativeKind::ApplicationIcon => ProviderMatchRule {
                    view: Some(LocationMatch {
                        provider_ids: Some(vec!["core:applications".into()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                NativeKind::Readonly => ProviderMatchRule::default(),
            };
            values
                .register(id, priority, rule, Arc::new(BuiltinDecoration(kind)))
                .expect("invalid native DecorationProvider");
        }
        values
    })
}

pub(super) fn providers(targets: &[serde_json::Value], directory: &str) -> Vec<Resolver> {
    let directory = crate::plugins::match_rules::directory_metadata(directory);
    registry().matched_candidates(targets, directory.as_ref(), targets.len())
}

pub(super) fn resolve(
    id: &str,
    directory: &str,
    entries: &HashMap<String, DecorationEntry>,
) -> Result<Vec<Decoration>, String> {
    Ok(registry()
        .get(id)
        .ok_or("unknown native DecorationProvider")?
        .resolve(directory, entries))
}

#[cfg(test)]
mod tests {
    use super::super::types::{DecorationTag, NativeBadge};
    use super::*;

    #[test]
    fn all_native_decorations_are_typed_and_priority_sorted() {
        let descriptors = registry().descriptors().collect::<Vec<_>>();
        assert_eq!(descriptors.len(), 4);
        for descriptor in descriptors {
            assert_eq!(descriptor.kind, ProviderKind::Decoration);
            descriptor.validate().unwrap();
        }
        let local = providers(&[], "/tmp");
        assert_eq!(local.len(), 2);
        assert_eq!(local[0].id, "builtin:readonly");
        assert_eq!(local[1].id, "builtin:tags");
        let applications = providers(&[], crate::core::volumes::applications::URI);
        assert!(applications
            .iter()
            .any(|provider| provider.id == "builtin:application-icon"));
        assert!(!applications
            .iter()
            .any(|provider| provider.id == "builtin:tags"));
    }

    #[test]
    fn unregistered_decorations_are_not_executable() {
        assert!(resolve("builtin:unknown", "/", &HashMap::new()).is_err());
    }

    #[test]
    fn each_registered_native_kind_keeps_its_operation() {
        let path = "/no-io/entry".to_string();
        let mut entry = DecorationEntry {
            path: path.clone(),
            readonly: true,
            tags: vec![DecorationTag {
                name: "project".into(),
                color: 2,
                hex: None,
            }],
            sync: Some("cloud".into()),
            ..Default::default()
        };
        let mut entries = HashMap::new();
        entries.insert(path.clone(), entry.clone());
        assert_eq!(
            resolve("builtin:readonly", "/", &entries).unwrap()[0].native_badges[0].text,
            crate::i18n::t("只读")
        );
        assert!(resolve("builtin:tags", "/", &entries).unwrap()[0]
            .native_badges
            .iter()
            .all(|badge: &NativeBadge| badge.dot));
        assert_eq!(
            resolve("builtin:connection-status", "/", &entries).unwrap()[0].native_badges[0]
                .status
                .as_deref(),
            Some("cloud")
        );
        entry.readonly = false;
        entries.insert(path, entry);
        assert!(resolve("builtin:readonly", "/", &entries)
            .unwrap()
            .is_empty());
    }
}
