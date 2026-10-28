use super::*;
use serde_json::json;
fn binding(dir: &std::path::Path, path: &std::path::Path) -> (Scope, Value) {
    let mut scope = Scope::selected(dir, &[path.to_path_buf()]).unwrap();
    let snapshot = crate::plugins::resource::resource_context::ResourceSnapshot::new(
        path.to_str().unwrap(),
        path,
    );
    let target = json!({"ref":snapshot.reference,"metadata":snapshot.metadata()});
    scope.bind_resource(&target, Some(path)).unwrap();
    (scope, target["ref"].clone())
}
#[test]
fn unbound_refs_and_forged_mounts_cannot_resolve() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    std::fs::write(&path, b"secret").unwrap();
    let (scope, reference) = binding(dir.path(), &path);
    assert!(ResourceBroker::resolve(&scope, "owner", &reference).is_ok());
    let mut forged = reference.clone();
    forged["mountId"] = json!("different");
    assert!(ResourceBroker::resolve(&scope, "owner", &forged).is_err());
    let empty = Scope::selected(dir.path(), &[]).unwrap();
    assert!(ResourceBroker::resolve(&empty, "owner", &reference).is_err());
}
#[test]
fn stream_ownership_seek_and_cleanup_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    std::fs::write(&path, b"abcdef").unwrap();
    let (scope, reference) = binding(dir.path(), &path);
    let broker = ResourceBroker::resolve(&scope, "stream-test", &reference).unwrap();
    let opened = broker.open_read().unwrap();
    let params = json!({"handle":opened["handle"],"offset":2,"length":2});
    assert!(stream_call("other", "fs.readOpen", &params).is_err());
    assert_eq!(
        stream_call("stream-test", "fs.readOpen", &params).unwrap()["base64"],
        "Y2Q="
    );
    cleanup("stream-test");
    assert!(stream_call("stream-test", "fs.readOpen", &params).is_err());
}
#[test]
fn transactional_writes_abort_and_commit_without_exposing_paths() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    std::fs::write(&path, b"before").unwrap();
    let (scope, reference) = binding(dir.path(), &path);
    let broker = ResourceBroker::resolve(&scope, "write-test", &reference).unwrap();
    let handle = broker.begin_write(false).unwrap();
    let params = json!({"handle":handle,"base64":STANDARD.encode(b"after")});
    stream_call("write-test", "fs.writeChunk", &params).unwrap();
    stream_call("write-test", "fs.abort", &params).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"before");
    let handle = broker.begin_write(false).unwrap();
    let params = json!({"handle":handle,"base64":STANDARD.encode(b"after")});
    stream_call("write-test", "fs.writeChunk", &params).unwrap();
    assert!(stream_call("write-test", "fs.commit", &params)
        .unwrap()
        .is_null());
    assert_eq!(std::fs::read(&path).unwrap(), b"after");
}
#[test]
fn readonly_view_rejects_stream_mutation_and_unrelated_destinations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    std::fs::write(&path, b"before").unwrap();
    let (mut scope, reference) = binding(dir.path(), &path);
    scope.restrict_to_readonly();
    assert!(scope.authorize_method("fs.beginWrite").is_err());
    assert!(scope.authorize_method("fs.seekWrite").is_err());
    let other = dir.path().join("b");
    assert!(scope
        .issue_child(&reference, other.to_str().unwrap(), None)
        .is_err());
}
#[test]
fn resource_identity_is_stable_across_fresh_snapshots() {
    use crate::plugins::resource::resource_context::ResourceSnapshot;
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    assert_eq!(
        ResourceSnapshot::new(a.to_str().unwrap(), &a).reference,
        ResourceSnapshot::resolve(a.to_str().unwrap())
            .unwrap()
            .reference
    );
    assert_ne!(
        ResourceSnapshot::new(a.to_str().unwrap(), &a).reference,
        ResourceSnapshot::new(b.to_str().unwrap(), &b).reference
    );
}

#[test]
fn application_user_children_use_builtin_broker_and_streamed_writes() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let root = crate::core::volumes::applications::URI;
    let parent = super::binding::trusted(root, "core:test-app").unwrap();
    let name = format!("fiex-test-{}.txt", crate::plugins::mounts::new_id().unwrap());
    let child = super::binding::child(&parent, &name).unwrap();
    assert!(child.stat().is_err());
    let handle = child.begin_write(true).unwrap();
    let request = json!({"handle":handle,"base64":STANDARD.encode(b"broker")});
    stream_call("core:test-app", "fs.writeChunk", &request).unwrap();
    stream_call("core:test-app", "fs.commit", &request).unwrap();
    assert_eq!(child.preview_prefix(32).unwrap(), b"broker");
    child.remove().unwrap();
    assert!(child.stat().is_err());
}

#[test]
fn trash_view_denies_regular_storage_mutations() {
    if let Some(path) = crate::core::volumes::trash_path() {
        if path.is_dir() {
            let view = super::binding::trusted(&path.to_string_lossy(), "core:test-trash").unwrap();
            assert!(view.require(FsOperation::Write, false).is_err());
            assert!(view.require(FsOperation::Remove, false).is_err());
        }
    }
}
