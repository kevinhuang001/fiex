//! Persisted folder membership for installed applications. System apps are not
//! real files, so moving them into an Applications folder is represented here;
//! real user-store entries continue to use the normal filesystem move.
//!
//! The mapping is keyed by folder URI and preserves the user's order. Its
//! refresh key covers only the installed application set: creating, renaming or
//! deleting folders never invalidates the application enumeration/cache.
use super::applications;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

const VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    apps_fingerprint: String,
    #[serde(default)]
    folders: BTreeMap<String, Vec<String>>,
}

fn store_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("application-folders.json"))
}

fn load() -> Store {
    let Some(path) = store_path() else {
        return Store::default();
    };
    let mut store = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Store>(&bytes).ok())
        .filter(|store: &Store| store.version <= VERSION)
        .unwrap_or_default();
    migrate_legacy_folders(&mut store);
    store
}

/// Older builds stored every user folder under `applications://user/<id>`.
/// Rewrite those keys to the single-segment form so mapped system apps stay
/// visible after the URI layout changed.
fn migrate_legacy_folders(store: &mut Store) {
    if !store
        .folders
        .keys()
        .any(|folder| folder.starts_with("applications://user/"))
    {
        return;
    }
    let folders = std::mem::take(&mut store.folders);
    for (folder, apps) in folders {
        let key = folder
            .strip_prefix("applications://user/")
            .map(|payload| format!("{}{payload}", applications::USER_PREFIX))
            .unwrap_or(folder);
        let entry = store.folders.entry(key).or_default();
        for app in apps {
            if !entry.iter().any(|candidate| candidate == &app) {
                entry.push(app);
            }
        }
    }
}

fn save(store: &Store) -> Result<(), String> {
    let path = store_path().ok_or("cannot locate application-folder store")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut store = store.clone();
    store.version = VERSION;
    let bytes = serde_json::to_vec_pretty(&store).map_err(|error| error.to_string())?;
    // Windows cannot replace an existing file with rename; the mapping is
    // small and only written on explicit moves/refreshes.
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn cache() -> &'static Mutex<Store> {
    static CACHE: OnceLock<Mutex<Store>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(load()))
}

fn lock() -> std::sync::MutexGuard<'static, Store> {
    cache().lock().unwrap_or_else(|error| error.into_inner())
}

fn normalized(path: &str) -> String {
    path.trim_end_matches(['/', '\\']).to_string()
}

fn is_folder_key(folder: &str) -> bool {
    folder == applications::URI || applications::is_user(folder)
}

fn is_same_or_child(folder: &str, candidate: &str) -> bool {
    candidate == folder
        || candidate
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('\\'))
}

fn remove_from_all(store: &mut Store, app: &str) {
    for apps in store.folders.values_mut() {
        apps.retain(|candidate| candidate != app);
    }
    store.folders.retain(|_, apps| !apps.is_empty());
}

/// Folder URI containing `app`, if any.
pub fn folder_for(app: &str) -> Option<String> {
    lock()
        .folders
        .iter()
        .find(|(_, apps)| apps.iter().any(|candidate| candidate == app))
        .map(|(folder, _)| folder.clone())
}

/// Ordered app ids mapped into `folder`.
pub fn apps_in_folder(folder: &str) -> Vec<String> {
    lock()
        .folders
        .get(&normalized(folder))
        .cloned()
        .unwrap_or_default()
}

pub fn is_mapped(app: &str) -> bool {
    folder_for(app).is_some()
}

pub fn mapped_apps() -> HashSet<String> {
    lock()
        .folders
        .values()
        .flat_map(|apps| apps.iter().cloned())
        .collect()
}

/// Move system applications into `destination` (`applications://` means the
/// root, which removes the mapping). User-store files are moved by the caller
/// through the regular filesystem transfer path.
pub fn move_apps(apps: &[String], destination: &str) -> Result<(), String> {
    let destination = normalized(destination);
    if !is_folder_key(&destination) {
        return Err("applications can only be organized inside the Applications view".into());
    }
    if destination != applications::URI {
        let path = applications::stored_path(&destination)?;
        if !path.is_dir() {
            return Err("application destination is not a folder".into());
        }
    }
    for app in apps {
        if !applications::is_system_app(app) {
            return Err("not a system application".into());
        }
        if applications::system_entry(app).is_err() {
            return Err(crate::i18n::t("应用不存在").to_string());
        }
    }
    let mut store = lock();
    if move_in_store(&mut store, apps, &destination)? {
        save(&store)?;
    }
    Ok(())
}

fn move_in_store(store: &mut Store, apps: &[String], destination: &str) -> Result<bool, String> {
    if apps.is_empty() {
        return Ok(false);
    }
    let mut changed = false;
    for app in apps {
        if !applications::is_system_app(app) {
            return Err("not a system application".into());
        }
        let current = store
            .folders
            .iter()
            .find(|(_, list)| list.iter().any(|candidate| candidate == app))
            .map(|(folder, _)| folder.clone());
        if current.as_deref() == Some(destination) {
            continue;
        }
        remove_from_all(store, app);
        if destination != applications::URI {
            let list = store.folders.entry(destination.to_string()).or_default();
            if !list.iter().any(|candidate| candidate == app) {
                list.push(app.clone());
            }
        }
        changed = true;
    }
    Ok(changed)
}

/// Remove mappings for `apps` and return them to the root.
pub fn unmap(apps: &[String]) -> Result<Vec<String>, String> {
    let mut store = lock();
    let mut removed = Vec::new();
    for app in apps {
        if store
            .folders
            .values()
            .any(|list| list.iter().any(|candidate| candidate == app))
        {
            remove_from_all(&mut store, app);
            removed.push(app.clone());
        }
    }
    if !removed.is_empty() {
        save(&store)?;
    }
    Ok(removed)
}

/// Forget a folder and every mapped descendant. Returns the app ids that must
/// be visible at the root after the folder is deleted.
pub fn forget_folder(folder: &str) -> Result<Vec<String>, String> {
    let folder = normalized(folder);
    let mut store = lock();
    let keys: Vec<String> = store
        .folders
        .keys()
        .filter(|key| is_same_or_child(&folder, key))
        .cloned()
        .collect();
    let mut apps = Vec::new();
    let changed = !keys.is_empty();
    for key in &keys {
        if let Some(list) = store.folders.remove(key) {
            for app in list {
                if !apps.iter().any(|candidate| candidate == &app) {
                    apps.push(app);
                }
            }
        }
    }
    if changed {
        save(&store)?;
    }
    Ok(apps)
}

/// Rename a folder key (and its descendants) after the backing directory moved.
pub fn rename_folder(old: &str, new: &str) -> Result<(), String> {
    let old = normalized(old);
    let new = normalized(new);
    let mut store = lock();
    let mut changed = false;
    let keys: Vec<String> = store
        .folders
        .keys()
        .filter(|key| is_same_or_child(&old, key))
        .cloned()
        .collect();
    for key in keys {
        let Some(list) = store.folders.remove(&key) else {
            continue;
        };
        let target = if key == old {
            new.clone()
        } else {
            format!("{new}{}", &key[old.len()..])
        };
        store.folders.insert(target, list);
        changed = true;
    }
    if changed {
        save(&store)?;
    }
    Ok(())
}

fn fingerprint(apps: &[String]) -> String {
    let mut ids: Vec<&str> = apps.iter().map(String::as_str).collect();
    ids.sort_unstable();
    ids.dedup();
    let digest = Sha256::digest(ids.join("\0").as_bytes());
    hex::encode(&digest[..16])
}

/// Reconcile mappings with the currently installed application set. Folder
/// structure is intentionally not part of this decision.
pub fn refresh(apps: &[String]) {
    let next = fingerprint(apps);
    let mut store = lock();
    if store.apps_fingerprint == next {
        return;
    }
    let installed: HashSet<&str> = apps.iter().map(String::as_str).collect();
    for list in store.folders.values_mut() {
        list.retain(|app| installed.contains(app.as_str()));
    }
    store.folders.retain(|_, list| !list.is_empty());
    store.apps_fingerprint = next;
    // Fingerprint changes are persisted even when no mapping needed pruning,
    // so the next enumeration skips the comparison.
    let _ = save(&store);
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOLDER_A: &str = "applications://user/Zm9sZGVyLWE";
    const FOLDER_B: &str = "applications://user/Zm9sZGVyLWI";
    const APP_A: &str = "applications://YXBwLWE";
    const APP_B: &str = "applications://YXBwLWI";

    #[test]
    fn legacy_folder_keys_are_rewritten_to_the_single_segment_form() {
        let mut store = Store::default();
        store
            .folders
            .insert(FOLDER_A.to_string(), vec![APP_A.to_string()]);
        migrate_legacy_folders(&mut store);
        assert_eq!(
            store.folders.get("applications://~Zm9sZGVyLWE"),
            Some(&vec![APP_A.to_string()])
        );
        assert!(!store.folders.contains_key(FOLDER_A));
    }

    #[test]
    fn moves_preserve_order_and_replace_the_previous_folder() {
        let mut store = Store::default();
        assert!(move_in_store(&mut store, &[APP_A.to_string()], FOLDER_A).unwrap());
        assert!(move_in_store(&mut store, &[APP_B.to_string()], FOLDER_A).unwrap());
        assert_eq!(
            store.folders.get(FOLDER_A),
            Some(&vec![APP_A.to_string(), APP_B.to_string()])
        );
        assert!(move_in_store(&mut store, &[APP_A.to_string()], FOLDER_B).unwrap());
        assert_eq!(store.folders.get(FOLDER_B), Some(&vec![APP_A.to_string()]));
        assert_eq!(store.folders.get(FOLDER_A), Some(&vec![APP_B.to_string()]));
    }

    #[test]
    fn forgetting_a_folder_returns_descendants_and_nested_keys() {
        let mut store = Store::default();
        let nested = format!("{FOLDER_A}/child");
        move_in_store(&mut store, &[APP_A.to_string()], FOLDER_A).unwrap();
        move_in_store(&mut store, &[APP_B.to_string()], &nested).unwrap();
        let apps = forget_in_store(&mut store, FOLDER_A);
        assert_eq!(apps, vec![APP_A.to_string(), APP_B.to_string()]);
        assert!(!store.folders.contains_key(FOLDER_A));
        assert!(!store.folders.contains_key(&nested));
    }

    #[test]
    fn renaming_a_folder_moves_nested_keys_too() {
        let mut store = Store::default();
        let nested = format!("{FOLDER_A}/child");
        move_in_store(&mut store, &[APP_A.to_string()], FOLDER_A).unwrap();
        move_in_store(&mut store, &[APP_B.to_string()], &nested).unwrap();
        rename_in_store(&mut store, FOLDER_A, FOLDER_B);
        assert!(store.folders.contains_key(FOLDER_B));
        assert!(store.folders.contains_key(&format!("{FOLDER_B}/child")));
        assert!(!store.folders.contains_key(FOLDER_A));
    }

    #[test]
    fn refresh_only_prunes_removed_applications() {
        let mut store = Store::default();
        move_in_store(&mut store, &[APP_A.to_string()], FOLDER_A).unwrap();
        move_in_store(&mut store, &[APP_B.to_string()], FOLDER_B).unwrap();
        let first = fingerprint(&[APP_A.to_string(), APP_B.to_string()]);
        assert_eq!(
            first,
            fingerprint(&[APP_B.to_string(), APP_A.to_string(), APP_A.to_string()])
        );
        refresh_in_store(&mut store, &[APP_A.to_string(), APP_B.to_string()]);
        assert_eq!(store.folders.len(), 2);
        refresh_in_store(&mut store, &[APP_B.to_string()]);
        assert!(!store.folders.contains_key(FOLDER_A));
        assert!(store.folders.contains_key(FOLDER_B));
    }

    fn forget_in_store(store: &mut Store, folder: &str) -> Vec<String> {
        let keys: Vec<String> = store
            .folders
            .keys()
            .filter(|key| is_same_or_child(folder, key))
            .cloned()
            .collect();
        let mut apps = Vec::new();
        for key in keys {
            if let Some(list) = store.folders.remove(&key) {
                apps.extend(list);
            }
        }
        apps
    }

    fn refresh_in_store(store: &mut Store, apps: &[String]) {
        let installed: HashSet<&str> = apps.iter().map(String::as_str).collect();
        for list in store.folders.values_mut() {
            list.retain(|app| installed.contains(app.as_str()));
        }
        store.folders.retain(|_, list| !list.is_empty());
        store.apps_fingerprint = fingerprint(apps);
    }

    fn rename_in_store(store: &mut Store, old: &str, new: &str) {
        let keys: Vec<String> = store
            .folders
            .keys()
            .filter(|key| is_same_or_child(old, key))
            .cloned()
            .collect();
        for key in keys {
            let Some(list) = store.folders.remove(&key) else {
                continue;
            };
            let target = if key == old {
                new.to_string()
            } else {
                format!("{new}{}", &key[old.len()..])
            };
            store.folders.insert(target, list);
        }
    }
}
