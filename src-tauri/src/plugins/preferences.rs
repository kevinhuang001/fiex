//! One shared provider preference store for every priority-aware resolver.
//!
//! Search, Preview, Info, Decorations, Context Menus and Hooks all use the same
//! model: a per-feature user order (stable provider ids) plus a disabled list.
//! The manifest `priority` stays the default; an explicit user order always
//! wins and new providers keep their default position until the user moves
//! them. Persistence lives in one JSON file instead of one store per feature.
use super::events::Resolver;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

/// Bounds keep a corrupt or hostile settings file from growing without limit.
const MAX_ENTRIES: usize = 256;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreference {
    /// Provider ids in the user's order. Ids missing from the order keep their
    /// default priority, so installing a provider never resets a custom order.
    #[serde(default)]
    pub order: Vec<String>,
    /// Contributions switched off without disabling the whole plugin.
    #[serde(default)]
    pub disabled: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    features: BTreeMap<String, ProviderPreference>,
}

fn file() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("provider-preferences.json"))
}

fn cache() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        let store = file()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Mutex::new(store)
    })
}

fn persist(store: &Store) {
    let Some(file) = file() else { return };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(store) {
        let _ = std::fs::write(file, bytes);
    }
}

/// Apply the stored preference for `feature` to an already priority-sorted
/// provider list. Unknown ids stay at their default position; the sort is
/// stable so equal defaults keep the registry's tie-breaker.
pub fn apply(feature: &str, providers: Vec<Resolver>) -> Vec<Resolver> {
    let Ok(store) = cache().lock() else {
        return providers;
    };
    apply_with(&store.features, feature, providers)
}

fn apply_with(
    features: &BTreeMap<String, ProviderPreference>,
    feature: &str,
    providers: Vec<Resolver>,
) -> Vec<Resolver> {
    let Some(preference) = features.get(feature) else {
        return providers;
    };
    let disabled: HashSet<&str> = preference.disabled.iter().map(String::as_str).collect();
    let mut providers: Vec<Resolver> = providers
        .into_iter()
        .filter(|provider| !disabled.contains(provider.id.as_str()))
        .collect();
    providers.sort_by_key(|provider| {
        preference
            .order
            .iter()
            .position(|id| id == &provider.id)
            .unwrap_or(usize::MAX)
    });
    providers
}

/// Every stored preference, keyed by feature, for the settings surface.
pub fn snapshot() -> serde_json::Value {
    let store = cache()
        .lock()
        .map(|store| store.clone())
        .unwrap_or_default();
    serde_json::to_value(&store.features).unwrap_or_else(|_| serde_json::json!({}))
}

fn clean(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.len() <= 200)
        .filter(|value| seen.insert(value.clone()))
        .take(MAX_ENTRIES)
        .collect()
}

/// Store one feature's preference. Empty order and disabled lists remove the
/// feature entirely, which is the documented "restore defaults" behaviour.
pub fn set(feature: &str, order: Vec<String>, disabled: Vec<String>) -> Result<(), String> {
    let feature = feature.trim();
    if feature.is_empty() || feature.len() > 64 {
        return Err("invalid provider feature".into());
    }
    let order = clean(order);
    let disabled = clean(disabled);
    let mut store = cache().lock().map_err(|_| "provider store poisoned")?;
    if order.is_empty() && disabled.is_empty() {
        store.features.remove(feature);
    } else {
        store
            .features
            .insert(feature.to_string(), ProviderPreference { order, disabled });
    }
    persist(&store);
    Ok(())
}

pub fn reset(feature: &str) -> Result<(), String> {
    set(feature, Vec::new(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::events::Handler;

    fn provider(id: &str, priority: i64) -> Resolver {
        Resolver {
            id: id.into(),
            name: id.into(),
            priority,
            handler: Handler::Builtin("core.test"),
        }
    }

    fn store(order: &[&str], disabled: &[&str]) -> BTreeMap<String, ProviderPreference> {
        BTreeMap::from([(
            "search".to_string(),
            ProviderPreference {
                order: order.iter().map(|id| id.to_string()).collect(),
                disabled: disabled.iter().map(|id| id.to_string()).collect(),
            },
        )])
    }

    #[test]
    fn user_order_wins_and_unknown_providers_keep_default_position() {
        let providers = vec![
            provider("plugin:a", 80),
            provider("plugin:b", 50),
            provider("core.search", 10),
        ];
        let ordered = apply_with(
            &store(&["plugin:b", "core.search"], &[]),
            "search",
            providers,
        );
        let ids: Vec<_> = ordered.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["plugin:b", "core.search", "plugin:a"]);
    }

    #[test]
    fn disabled_contributions_are_removed() {
        let providers = vec![provider("plugin:a", 80), provider("core.search", 10)];
        let filtered = apply_with(&store(&[], &["plugin:a"]), "search", providers);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "core.search");
    }

    #[test]
    fn other_features_are_untouched() {
        let providers = vec![provider("plugin:a", 80), provider("plugin:b", 50)];
        let untouched = apply_with(&store(&["plugin:b"], &[]), "preview", providers);
        let ids: Vec<_> = untouched.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["plugin:a", "plugin:b"]);
    }

    #[test]
    fn duplicate_ids_are_collapsed_and_blanks_dropped() {
        assert_eq!(
            clean(vec!["a".into(), " a ".into(), String::new(), "b".into()]),
            ["a", "b"]
        );
    }
}
