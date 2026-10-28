use super::*;

#[test]
fn vault_reads_and_denials_are_cached_without_reprompting() {
    let mut cache = None;
    assert!(cached_vault(&mut cache, || Err("access denied".into())).is_err());
    assert_eq!(
        cached_vault(&mut cache, || panic!("repeated OS access")),
        Err("access denied".into())
    );
    let mut cache = None;
    let values = BTreeMap::from([("p:s:k".into(), "secret".into())]);
    assert_eq!(
        cached_vault(&mut cache, || Ok(values.clone())).unwrap(),
        values
    );
    assert_eq!(
        cached_vault(&mut cache, || panic!("repeated OS access")).unwrap(),
        values
    );
}
#[test]
fn fallback_round_trips_sets_and_deletes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("secrets.json");
    let mut items = load_fallback(&path).unwrap();
    assert!(items.is_empty());
    items.insert(
        fallback_key("p.one", "connection-a", "password"),
        "s3cret".into(),
    );
    save_fallback(&path, &items).unwrap();
    assert_eq!(
        load_fallback(&path)
            .unwrap()
            .remove(&fallback_key("p.one", "connection-a", "password"))
            .unwrap(),
        "s3cret"
    );
    // Keys are scoped: another plugin, scope or field never matches.
    let store = load_fallback(&path).unwrap();
    assert!(!store.contains_key(&fallback_key("p.two", "connection-a", "password")));
    assert!(!store.contains_key(&fallback_key("p.one", "connection-b", "password")));
    assert!(!store.contains_key(&fallback_key("p.one", "connection-a", "username")));
    let empty = BTreeMap::new();
    save_fallback(&path, &empty).unwrap();
    assert!(load_fallback(&path).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn fallback_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("secrets.json");
    let mut items = BTreeMap::new();
    items.insert(fallback_key("p", "s", "k"), "v".into());
    save_fallback(&path, &items).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "{mode:o}");
}
