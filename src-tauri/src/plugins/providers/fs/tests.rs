use super::*;
use crate::plugins::provider_registry::FsOperation;
use serde_json::json;

fn entry(value: Value) -> Result<FsEntry, String> {
    super::types::parse_entry(value)
}

#[test]
fn entries_require_a_stable_node_id_and_a_safe_name() {
    let base = json!({"nodeId":"n1","name":"a.txt","kind":"file","size":4});
    assert_eq!(entry(base.clone()).unwrap().node_id, "n1");
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("nodeId");
    assert!(entry(missing).is_err());
    let mut traversal = base.clone();
    traversal["name"] = json!("../etc");
    assert!(entry(traversal).is_err());
    let mut nul = base;
    nul["nodeId"] = json!("n\0");
    assert!(entry(nul).is_err());
}

#[test]
fn node_capabilities_narrow_the_provider() {
    let read_limited = entry(json!({
        "nodeId":"n1","name":"a.txt","kind":"file","allowedOps":["read","stat"]
    }))
    .unwrap();
    assert!(read_limited.allows(FsOperation::Read));
    assert!(!read_limited.allows(FsOperation::Rename));
    let writable = entry(json!({
        "nodeId":"n2","name":"b.txt","kind":"file","allowedOps":["read","write","rename","remove"]
    }))
    .unwrap();
    assert!(writable.allows(FsOperation::Write));
}

#[test]
fn write_requests_validate_the_leaf_name() {
    let mount = Mount {
        mount_id: "m".into(),
        plugin_id: "org.example".into(),
        source: Source::Connection(Value::Null),
        scope: None,
    };
    let error = match mount.begin_write(WriteRequest {
        node_id: None,
        parent_id: Some("root"),
        name: Some("bad/name"),
        size: None,
        if_revision: None,
    }) {
        Err(error) => error,
        Ok(_) => panic!("expected a rejected name"),
    };
    assert!(error.contains("segment"));
}

#[test]
fn malformed_node_restrictions_cannot_become_unrestricted() {
    for restrictions in [json!("read"), json!([42])] {
        assert!(entry(json!({"nodeId":"n","name":"a","allowedOps":restrictions})).is_err());
    }
    assert!(entry(json!({"nodeId":"n","name":"a","readonly":true})).is_err());
    let unknown = entry(json!({"nodeId":"n","name":"a","allowedOps":["future"]})).unwrap();
    assert!(unknown.allowed_ops.as_ref().unwrap().is_empty());
}

#[test]
fn vfs_pagination_requires_canonical_bounded_shared_pages() {
    use fiex_provider_contracts::VfsPage;

    let listing: VfsPage = serde_json::from_value(json!({"entries":[]})).unwrap();
    assert!(!listing.truncated);
    assert!(listing.validate_page(0).is_ok());
    let truncated: VfsPage =
        serde_json::from_value(json!({"entries":[],"truncated":true})).unwrap();
    assert!(truncated.truncated);

    for bad in [
        json!({"entries":[],"truncated":null}),
        json!({"entries":[],"truncated":"false"}),
        json!({"entries":[],"nextCursor":42}),
        json!({"entries":[],"warnings":"error"}),
    ] {
        assert!(serde_json::from_value::<VfsPage>(bad).is_err());
    }
    let empty_cursor: VfsPage =
        serde_json::from_value(json!({"entries":[],"nextCursor":""})).unwrap();
    assert!(empty_cursor.validate_page(500).is_err());
    let oversize: VfsPage = serde_json::from_value(json!({"entries":[{},{}]})).unwrap();
    assert!(oversize.validate_page(1).is_err());

    let search: VfsPage =
        serde_json::from_value(json!({"entries":[],"nextCursor":"bad\n"})).unwrap();
    assert!(search.validate_page(500).is_err());
}
