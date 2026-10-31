//! Native ContextMenu providers use the same registered descriptor, view
//! matcher, execution and session validation pipeline as external providers.
//! View-specific actions live in implementations, not in EventRouter.
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{LocationMatch, ProviderKind, ProviderMatchRule};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

mod implementations;
use implementations::{ApplicationsMenu, RecentMenu, TrashMenu};

trait NativeViewMenuProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn owner(&self) -> &'static str;
    fn claimed_actions(&self) -> &'static [&'static str];
    fn items(&self, path: &str) -> Vec<Value>;
    fn execute(&self, item_id: &str, paths: &[String]) -> Result<NativeAction, String>;
}

pub(super) struct NativeAction {
    pub command: &'static str,
    pub paths: Vec<String>,
}

fn registry() -> &'static NativeProviderRegistry<dyn NativeViewMenuProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn NativeViewMenuProvider>> =
        OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn NativeViewMenuProvider> =
            NativeProviderRegistry::new(ProviderKind::ContextMenu);
        let implementations: [Arc<dyn NativeViewMenuProvider>; 3] = [
            Arc::new(RecentMenu),
            Arc::new(ApplicationsMenu),
            Arc::new(TrashMenu),
        ];
        for implementation in implementations {
            let owner = implementation.owner();
            values
                .register(
                    implementation.id(),
                    100,
                    ProviderMatchRule {
                        view: Some(LocationMatch {
                            provider_ids: Some(vec![owner.into()]),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    implementation,
                )
                .expect("invalid native ViewMenu provider");
        }
        values
    })
}

fn eligible(provider: &dyn NativeViewMenuProvider, directory: &str, paths: &[String]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let Some(descriptor) = registry().descriptor(provider.id()) else {
        return false;
    };
    let rule = &descriptor.r#match;
    let (directory_view, _) = crate::plugins::match_rules::locations_for(directory);
    if !rule.matches_targets(&[json!({"view": directory_view})], 1) {
        return false;
    }
    // A mixed-source selection must match *every* target. Never inherit
    // elevated actions from the first item or from a plugin-supplied path.
    let targets = paths
        .iter()
        .map(|path| {
            let (view, _) = crate::plugins::match_rules::locations_for(path);
            json!({"view": view})
        })
        .collect::<Vec<_>>();
    rule.matches_targets(&targets, paths.len())
}

fn provider(
    id: &str,
    directory: &str,
    paths: &[String],
) -> Result<&'static dyn NativeViewMenuProvider, String> {
    registry()
        .get(id)
        .filter(|implementation| eligible(*implementation, directory, paths))
        .ok_or_else(|| "native menu provider does not match selection".into())
}

pub(super) fn providers(directory: &str, paths: &[String]) -> Vec<Resolver> {
    registry().candidates(|implementation| eligible(implementation, directory, paths))
}

pub(super) fn owns(id: &str) -> bool {
    registry().get(id).is_some()
}

/// Native providers only claim the identifiers they actually own. Common
/// actions such as Info and Preview remain available from generic providers.
pub(super) fn claimed_actions(directory: &str, paths: &[String]) -> &'static [&'static str] {
    providers(directory, paths)
        .first()
        .and_then(|candidate| registry().get(&candidate.id))
        .map_or(&[], |provider| provider.claimed_actions())
}

pub(super) fn resolve(id: &str, directory: &str, paths: &[String]) -> Result<Value, String> {
    let implementation = provider(id, directory, paths)?;
    let offered = implementation.items(&paths[0]);
    let intersection = offered
        .into_iter()
        .filter(|item| {
            paths.iter().skip(1).all(|path| {
                implementation
                    .items(path)
                    .iter()
                    .any(|other| other["id"] == item["id"])
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"items": intersection}))
}

pub(super) fn execute(
    id: &str,
    item_id: &str,
    directory: &str,
    paths: &[String],
) -> Result<NativeAction, String> {
    provider(id, directory, paths)?.execute(item_id, paths)
}

#[cfg(test)]
mod tests;
