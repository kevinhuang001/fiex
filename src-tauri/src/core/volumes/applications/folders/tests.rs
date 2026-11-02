use super::*;

const FOLDER_A: &str = "applications://~Zm9sZGVyLWE";
const FOLDER_B: &str = "applications://~Zm9sZGVyLWI";
const APP_A: &str = "applications://YXBwLWE";
const APP_B: &str = "applications://YXBwLWI";

#[test]
fn legacy_folder_keys_are_not_accepted_as_canonical_entries() {
    let mut store = Store::default();
    store.folders.insert(
        "applications://user/Zm9sZGVyLWE".to_string(),
        vec![APP_A.to_string()],
    );
    store
        .folders
        .retain(|folder, _| folder == applications::URI || applications::is_user(folder));
    assert!(store.folders.is_empty());
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
