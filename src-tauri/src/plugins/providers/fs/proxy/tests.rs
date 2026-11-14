use super::{IpcVfsProvider, MountInvocation};
use crate::plugins::providers::fs::{Mount, Source};
use fiex_provider_contracts::{FsOperation, FsRequest, VfsProvider};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn provider_ipc_failure_is_returned_and_a_later_call_can_recover() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let provider = IpcVfsProvider::with_remote_invoke(
        "org.example.vfs-fixture".into(),
        move |mount, method, _params| {
            assert_eq!(mount.plugin_id, "org.example.vfs-fixture");
            assert_eq!(method, "vfs.stat");
            match observed.fetch_add(1, Ordering::SeqCst) {
                0 => Err("fixture IPC disconnected".into()),
                1 => Ok(json!({"nodeId":"node-1","name":"entry.txt"})),
                _ => Ok(json!({"nodeId":"node-2"})),
            }
        },
    );
    let mount = Mount {
        mount_id: "mount-fixture".into(),
        plugin_id: "org.example.vfs-fixture".into(),
        source: Source::Connection(Value::Null),
        scope: None,
    };
    let invocation = MountInvocation::new(mount, "vfs.stat", json!({"nodeId":"node-1"}));
    let request = FsRequest::new(&invocation, FsOperation::Stat).unwrap();

    assert!(provider.stat(&request).is_err());
    assert_eq!(provider.stat(&request).unwrap().node_id, "node-1");
    assert!(provider.stat(&request).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}
