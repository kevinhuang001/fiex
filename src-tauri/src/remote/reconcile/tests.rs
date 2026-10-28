use super::*;
#[test]
fn subtree_boundaries() {
    assert!(in_tree("a/b", "a"));
    assert!(!in_tree("ab/file", "a"));
    assert!(in_tree("anything", ""));
}
#[test]
fn fingerprints_include_content() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"one").unwrap();
    let first = fingerprint(file.path()).unwrap();
    fs::write(file.path(), b"two").unwrap();
    assert_ne!(first, fingerprint(file.path()).unwrap());
}
#[test]
fn metadata_migration_defaults() {
    let file: RemoteMetaFile =
        serde_json::from_str(r#"{"size":1,"modified":0,"isDir":false,"downloaded":true}"#).unwrap();
    assert!(file.baseline.is_empty());
    assert!(!file.pinned);
}
