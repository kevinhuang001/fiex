//! Persistent Core decorations, routed through the same verified Provider
//! Descriptor and EventRouter as SDK-backed DecorationProviders.
#[cfg(test)]
use super::types::Decoration;
#[cfg(test)]
use super::types::DecorationEntry;
use super::{
    builtin::{self, NativeKind},
    invocation,
};
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{
    DecorationContribution, DecorationProvider, LocationMatch, MatchResource, ProviderError,
    ProviderInvocation, ProviderKind, ProviderMatchRule,
};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

struct BuiltinDecoration(NativeKind);
impl DecorationProvider for BuiltinDecoration {
    fn resolve(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<Vec<DecorationContribution>, ProviderError> {
        let services = invocation::services(ctx).map_err(ProviderError::internal)?;
        let resources_by_path = services
            .resources
            .iter()
            .map(|resource| (resource.display.as_str(), resource))
            .collect::<HashMap<_, _>>();
        builtin::resolve_in(self.0, &services.directory, services.entries.as_ref())
            .into_iter()
            .map(|decoration| {
                let target = resources_by_path
                    .get(decoration.path.as_str())
                    .ok_or_else(|| {
                        ProviderError::invalid_params(
                            "native decoration target is outside the scoped request",
                        )
                    })?;
                Ok(DecorationContribution {
                    target: target.reference.clone(),
                    native_badges: decoration.native_badges,
                    badges: Vec::new(),
                    label: decoration.label,
                    description: decoration.description,
                    style: Default::default(),
                    fields: decoration.fields,
                    icon: decoration.icon,
                    icon_path: decoration.icon_path,
                })
            })
            .collect()
    }
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn DecorationProvider>> {
    registry().get_arc(id)
}

pub(super) fn invocation(
    id: &str,
    method: impl Into<String>,
    params: serde_json::Value,
    resources: &[crate::plugins::resource::resource_context::ResourceSnapshot],
    directory: Option<&crate::plugins::resource::resource_context::ResourceSnapshot>,
) -> Result<crate::plugins::native_invocation::NativeProviderInvocation, String> {
    registry().invocation(id, method, params, resources, directory)
}

fn registry() -> &'static NativeProviderRegistry<dyn DecorationProvider> {
    static REGISTRY: OnceLock<NativeProviderRegistry<dyn DecorationProvider>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn DecorationProvider> =
            NativeProviderRegistry::new(ProviderKind::Decoration);
        for (id, priority, kind) in [
            ("builtin:tags", -12, NativeKind::Tags),
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
            };
            values
                .register(id, priority, rule, Arc::new(BuiltinDecoration(kind)))
                .expect("invalid native DecorationProvider");
        }
        values
    })
}

pub(super) fn providers(targets: &[MatchResource], directory: &str) -> Vec<Resolver> {
    let directory = crate::plugins::match_rules::directory_match_resource(directory);
    registry().matched_candidates(targets, directory.as_ref(), targets.len())
}

#[cfg(test)]
pub(super) fn resolve(
    id: &str,
    directory: &str,
    entries: &HashMap<String, DecorationEntry>,
) -> Result<Vec<Decoration>, String> {
    let entries = Arc::new(entries.clone());
    let resources = Arc::new(
        entries
            .values()
            .map(|entry| {
                crate::plugins::resource::resource_context::from_metadata(
                    &entry.path,
                    crate::plugins::contracts::ResourceMetadata {
                        name: entry.name.clone(),
                        kind: if entry.is_dir { "directory" } else { "file" }.into(),
                        extension: Some(entry.extension.clone()),
                        mime: Some(entry.mime.clone()),
                        ..Default::default()
                    },
                )
            })
            .collect(),
    );
    let invocation =
        invocation::CoreDecorationInvocation::native(id, directory.into(), entries, resources)?;
    invoke(id, &invocation)
}

#[cfg(test)]
pub(super) fn invoke(id: &str, ctx: &dyn ProviderInvocation) -> Result<Vec<Decoration>, String> {
    let provider = registry()
        .get(id)
        .ok_or("unknown native DecorationProvider")?;
    let values = provider.resolve(ctx).map_err(|error| error.to_string())?;
    let services = invocation::services(ctx)?;
    let resources_by_ref = super::dynamic::resource_path_index(services.resources.as_slice());
    values
        .into_iter()
        .map(|value| {
            let path = resources_by_ref
                .get(&(
                    value.target.mount_id.as_str(),
                    value.target.node_id.as_str(),
                ))
                .ok_or("native decoration target is outside the scoped request")?;
            Ok(Decoration {
                path: (*path).to_owned(),
                native_badges: value.native_badges,
                label: value.label,
                description: value.description,
                fields: super::dynamic::fields_with_name_contribution(
                    value.fields,
                    value.badges,
                    value.style,
                ),
                icon: value.icon,
                icon_path: value.icon_path,
                providers: Vec::new(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::types::{DecorationTag, NativeBadge};
    use super::*;

    #[test]
    fn all_native_decorations_are_typed_and_priority_sorted() {
        let descriptors = registry().descriptors().collect::<Vec<_>>();
        assert_eq!(descriptors.len(), 3);
        for descriptor in descriptors {
            assert_eq!(descriptor.kind, ProviderKind::Decoration);
            descriptor.validate().unwrap();
        }
        let local = providers(&[], "/tmp");
        assert_eq!(local.len(), 1);
        assert_eq!(local[0].id, "builtin:tags");
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
    fn connection_status_and_tags_keep_their_native_badges() {
        let path = "/no-io/entry".to_string();
        let entry = DecorationEntry {
            path: path.clone(),
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
        entries.insert(path, entry);
    }
}
