//! Native ContextMenu providers use the same registered descriptor, view
//! matcher, execution and session validation pipeline as external providers.
//! View-specific actions live in implementations, not in EventRouter.
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{LocationMatch, ProviderKind, ProviderMatchRule, ProviderMatcher};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

mod implementations;
use implementations::{ApplicationsMenu, RecentMenu, TrashMenu};

trait NativeViewMenuProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn owner(&self) -> &'static str;
    fn claimed_actions(&self) -> &'static [&'static str];
    fn items(&self, path: &str) -> Vec<Value>;
    /// A provider may contribute actions for the directory when selection is
    /// empty. Generic dispatch never needs to know which view owns them.
    fn blank_items(&self, _directory: &str) -> Vec<Value> {
        Vec::new()
    }
    fn selection_items(&self, directory: &str, paths: &[String]) -> Vec<Value> {
        if paths.is_empty() {
            self.blank_items(directory)
        } else {
            intersection(self, paths)
        }
    }
    fn execute(&self, item_id: &str, paths: &[String]) -> Result<NativeAction, String>;
}

/// Compute an action intersection using the same native provider for every
/// selected node. The empty selection is a valid directory target.
fn intersection<P: NativeViewMenuProvider + ?Sized>(
    implementation: &P,
    paths: &[String],
) -> Vec<Value> {
    let Some(first) = paths.first() else {
        return Vec::new();
    };
    implementation
        .items(first)
        .into_iter()
        .filter(|item| {
            paths.iter().skip(1).all(|path| {
                implementation
                    .items(path)
                    .iter()
                    .any(|other| other["id"] == item["id"])
            })
        })
        .collect()
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
    if paths.is_empty() && provider.blank_items(directory).is_empty() {
        return false;
    }
    let Some(descriptor) = registry().descriptor(provider.id()) else {
        return false;
    };
    let targets = paths
        .iter()
        .map(|path| {
            let (view, storage) = crate::plugins::match_rules::locations_for(path);
            json!({"view": view, "storage": storage})
        })
        .collect::<Vec<_>>();
    let directory_context = crate::plugins::match_rules::directory_metadata(directory);
    ProviderMatcher::matches(
        &descriptor.r#match,
        &targets,
        directory_context.as_ref(),
        paths.len(),
    )
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
    let targets = paths
        .iter()
        .map(|path| {
            let (view, storage) = crate::plugins::match_rules::locations_for(path);
            json!({"view": view, "storage": storage})
        })
        .collect::<Vec<_>>();
    let directory_context = crate::plugins::match_rules::directory_metadata(directory);
    registry()
        .matched_candidates(&targets, directory_context.as_ref(), paths.len())
        .into_iter()
        .filter(|candidate| {
            paths.len() > 0
                || registry()
                    .get(&candidate.id)
                    .is_some_and(|provider| !provider.blank_items(directory).is_empty())
        })
        .collect()
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
    Ok(json!({"items": implementation.selection_items(directory, paths)}))
}

pub(super) fn execute(
    id: &str,
    item_id: &str,
    directory: &str,
    paths: &[String],
) -> Result<NativeAction, String> {
    // The caller's session already checks what was offered; still enforce
    // this invariant at the native provider boundary for direct invocations.
    let offered = resolve(id, directory, paths)?;
    if !offered["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == item_id && item["disabled"] != true)
    }) {
        return Err("native menu action was not offered".into());
    }
    provider(id, directory, paths)?.execute(item_id, paths)
}

#[cfg(test)]
mod tests;
