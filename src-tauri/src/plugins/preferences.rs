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
        let saved = crate::infrastructure::state::get("providers", "ordering")
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value::<Store>(value).ok());
        let store = if let Some(saved) = saved {
            saved
        } else {
            let old: Store = file()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            if !old.features.is_empty() {
                if crate::infrastructure::state::put(
                    "providers",
                    "ordering",
                    serde_json::to_value(&old).unwrap_or_default(),
                )
                .is_ok()
                {
                    if let Some(path) = file() {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
            old
        };
        Mutex::new(store)
    })
}

fn persist(store: &Store) -> Result<(), String> {
    let value = serde_json::to_value(store).map_err(|e| e.to_string())?;
    crate::infrastructure::state::put("providers", "ordering", value)
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

/// Features that expose provider ordering. The names match the manifest
/// extension keys, so one catalog entry can never drift from the router.
pub const FEATURES: [&str; 6] = [
    "search",
    "preview",
    "info",
    "decorations",
    "contextMenus",
    "hooks",
];

fn builtin_entries(feature: &str) -> Vec<serde_json::Value> {
    use serde_json::json;
    match feature {
        "search" => vec![json!({
            "id": "core.search",
            "name": "Fiex",
            "defaultPriority": 10,
            "builtin": true,
            "locked": false,
            "modes": ["name", "content"],
        })],
        // Preview builtins are part of the rendering ladder, not removable
        // contributions; only the fallback is locked in place.
        "preview" => [
            ("builtin:text", 10, false),
            ("builtin:image", 20, false),
            ("builtin:media", 30, false),
            ("builtin:pdf", 40, false),
            ("builtin:fallback", 0, true),
        ]
        .into_iter()
        .map(|(id, priority, locked)| {
            json!({
                "id": id,
                "name": "Fiex",
                "defaultPriority": priority,
                "builtin": true,
                "locked": locked,
            })
        })
        .collect(),
        _ => Vec::new(),
    }
}

fn search_modes(contribution: &serde_json::Value) -> Vec<&str> {
    match contribution
        .get("modes")
        .and_then(serde_json::Value::as_array)
    {
        Some(modes) => modes
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|mode| matches!(*mode, "name" | "content"))
            .collect(),
        // A legacy single-RPC search contribution is filename search only.
        None => vec!["name"],
    }
}

/// Installed providers per feature plus the Core builtins. Locked entries are
/// shown but cannot be reordered or disabled by the settings surface.
pub fn catalog() -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut features = Map::new();
    for feature in FEATURES {
        features.insert(feature.into(), Value::Array(builtin_entries(feature)));
    }
    for plugin in crate::plugins::host::catalog() {
        for feature in FEATURES {
            let Some(contribution) = plugin.extensions.get(feature) else {
                continue;
            };
            if !contribution.is_object() && contribution.as_bool() != Some(true) {
                continue;
            }
            let available = plugin.enabled && plugin.approved && plugin.backend_available;
            let mut entry = json!({
                "id": format!("plugin:{}", plugin.id),
                "name": plugin.name,
                "pluginId": plugin.id,
                "defaultPriority": contribution
                    .get("priority")
                    .and_then(Value::as_i64)
                    .unwrap_or(50),
                "builtin": false,
                "locked": false,
                "available": available,
            });
            if feature == "search" {
                entry["modes"] = json!(search_modes(contribution));
            }
            if let Some(entries) = features.get_mut(feature).and_then(Value::as_array_mut) {
                entries.push(entry);
            }
        }
    }
    json!({ "features": features })
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
    let previous = store.clone();
    if order.is_empty() && disabled.is_empty() {
        store.features.remove(feature);
    } else {
        store
            .features
            .insert(feature.to_string(), ProviderPreference { order, disabled });
    }
    if let Err(error) = persist(&store) {
        *store = previous;
        return Err(error);
    }
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
