use super::put_back;

const FINDER_STYLE_STORE: &[u8] = include_bytes!("../../tests/data/putback_types.DS_Store");

#[test]
fn finds_put_back_records_among_unrelated_finder_record_keys() {
    let (location, name) = put_back(FINDER_STYLE_STORE, "a.jpg").unwrap();
    assert_eq!(location, "System/Volumes/Data/Users/u/Pics/");
    assert_eq!(name, "a.jpg");
}

#[test]
fn skips_unrelated_typed_records_before_the_put_back_entry() {
    let (location, name) = put_back(FINDER_STYLE_STORE, "b.jpg").unwrap();
    assert_eq!(location, "System/Volumes/Data/Users/u/Docs/");
    assert_eq!(name, "b.jpg");
}

#[test]
fn malformed_or_truncated_stores_do_not_panic_or_return_paths() {
    assert!(put_back(&[], "a.jpg").is_none());
    assert!(put_back(&FINDER_STYLE_STORE[..20], "a.jpg").is_none());
    let mut bad_magic = FINDER_STYLE_STORE.to_vec();
    bad_magic[4] = b'X';
    assert!(put_back(&bad_magic, "a.jpg").is_none());
}
