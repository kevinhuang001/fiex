//! Shared order and enablement preferences for event resolvers.
use super::Resolver;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::{Mutex, OnceLock}};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreference {
    #[serde(default)] pub order: Vec<String>,
    #[serde(default)] pub disabled: Vec<String>,
}

fn file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir().ok_or("configuration directory unavailable")?
        .join("fiex").join("provider-preferences.json"))
}
fn state() -> &'static Mutex<BTreeMap<String, ProviderPreference>> {
    static STATE: OnceLock<Mutex<BTreeMap<String, ProviderPreference>>> = OnceLock::new();
    STATE.get_or_init(|| {
        let saved = file().ok().and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Mutex::new(saved)
    })
}
pub fn all() -> Result<BTreeMap<String, ProviderPreference>, String> {
    Ok(state().lock().map_err(|_| "provider preferences unavailable")?.clone())
}
pub fn set(feature: &str, preference: ProviderPreference) -> Result<(), String> {
    if !matches!(feature, "search" | "preview" | "info" | "decorations" |
        "contextMenus" | "hooks" | "connectionContextMenus") {
        return Err("unknown provider feature".into());
    }
    if preference.order.len() > 256 || preference.disabled.len() > 256 ||
        preference.order.iter().chain(&preference.disabled).any(|id| id.len() > 256) {
        return Err("provider preference exceeds limit".into());
    }
    let mut guard = state().lock().map_err(|_| "provider preferences unavailable")?;
    let mut next = guard.clone();
    if preference.order.is_empty() && preference.disabled.is_empty() {
        next.remove(feature);
    } else {
        next.insert(feature.into(), preference);
    }
    let path = file()?;
    fs::create_dir_all(path.parent().ok_or("invalid settings path")?)
        .map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&next).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    *guard = next;
    Ok(())
}
pub fn apply(feature: &str, providers: &mut Vec<Resolver>) {
    let Some(preference) = state().lock().ok().and_then(|store| store.get(feature).cloned())
        else { return; };
    // Mandatory Core providers remain available. The search builtin stays the
    // final Auto fallback even if a user order contains it.
    providers.retain(|provider| !provider.id.starts_with("plugin:") ||
        !preference.disabled.contains(&provider.id));
    providers.sort_by(|a, b| {
        let position = |provider: &Resolver| preference.order.iter().position(|id| id == &provider.id);
        match (position(a), position(b)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)),
        }
    });
    if feature == "search" {
        if let Some(index) = providers.iter().position(|provider| provider.id == "core.search") {
            let builtin = providers.remove(index);
            providers.push(builtin);
        }
    }
}
