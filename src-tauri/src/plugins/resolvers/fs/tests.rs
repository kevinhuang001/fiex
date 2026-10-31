use super::*;

fn entry(value: Value) -> Result<FsEntry, String> {
    FsEntry::parse(value)
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
    let read_only = entry(json!({
        "nodeId":"n1","name":"a.txt","kind":"file","allowedOps":["read","stat"]
    }))
    .unwrap();
    assert!(read_only.allows(FsOperation::Read));
    assert!(!read_only.allows(FsOperation::Rename));
    assert!(read_only.is_readonly());
    let writable = entry(json!({
        "nodeId":"n2","name":"b.txt","kind":"file","allowedOps":["read","write","rename","remove"]
    }))
    .unwrap();
    assert!(!writable.is_readonly());
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
    assert!(entry(json!({"nodeId":"n","name":"a","readonly":"true"})).is_err());
    let unknown = entry(json!({"nodeId":"n","name":"a","allowedOps":["future"]})).unwrap();
    assert!(unknown.is_readonly());
}

#[test]
fn vfs_pagination_preserves_truncation_without_requiring_false_flag() {
    assert_eq!(truncated_flag(&json!({"entries":[]})).unwrap(), false);
    assert_eq!(truncated_flag(&json!({"truncated":false})).unwrap(), false);
    assert_eq!(truncated_flag(&json!({"truncated":true})).unwrap(), true);
    assert!(truncated_flag(&json!({"truncated":null})).is_err());
    assert!(truncated_flag(&json!({"truncated":"false"})).is_err());
}
