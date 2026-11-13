use super::*;
use serde_json::json;

#[test]
fn declaration_follows_the_handshake() {
    let provider = register(
        "org.example.drive",
        &json!({"pluginId":"org.example.drive","ready":true,"vfs":{"discovery":true,"operations":["list","stat","read"],"preflightOps":[]}}),
    );
    assert!(provider.reported);
    assert_eq!(provider.operations.len(), 3);
    assert!(supports("org.example.drive", FsOperation::List));
    assert!(!supports("org.example.drive", FsOperation::Rename));
    forget("org.example.drive");
    assert!(!supports("org.example.drive", FsOperation::List));
}

#[test]
fn unknown_operations_are_ignored() {
    let provider = register(
        "org.example.future",
        &json!({"vfs":{"discovery":true,"operations":["list","teleport"],"preflightOps":[]}}),
    );
    assert_eq!(
        provider.operations.into_iter().collect::<Vec<_>>(),
        vec![FsOperation::List]
    );
    forget("org.example.future");
}

#[test]
fn undeclared_plugins_cannot_receive_file_requests() {
    forget("org.example.uninstalled");
    assert!(authorize_method("org.example.uninstalled", "vfs.list").is_err());
    register(
        "org.example.uninstalled",
        &json!({"pluginId":"org.example.uninstalled"}),
    );
    assert!(authorize_method("org.example.uninstalled", "vfs.list").is_err());
    assert!(authorize_method("org.example.uninstalled", "connection.open").is_ok());
    forget("org.example.uninstalled");
}

#[test]
fn static_capability_handshakes_are_rejected_without_discovery() {
    let plugin = "org.example.static-vfs";
    let provider = register(
        plugin,
        &json!({"vfs":{"operations":["list","read"],"preflightOps":[]}}),
    );
    assert!(!provider.reported);
    assert!(provider.operations.is_empty());
    assert!(authorize_method(plugin, "vfs.list").is_err());
    forget(plugin);
}

#[test]
fn discovery_gates_new_operations_without_orphaning_streams() {
    let plugin = "org.example.discovery";
    register(
        plugin,
        &json!({"vfs":{"operations":["read","write"],"discovery":true,"preflightOps":[]}}),
    );
    assert!(needs_discovery(plugin, "vfs.openRead"));
    assert!(needs_discovery(plugin, "vfs.beginWrite"));
    for method in [
        "vfs.discover",
        "vfs.closeRead",
        "vfs.abort",
        "vfs.commit",
        "vfs.readStream",
    ] {
        assert!(!needs_discovery(plugin, method));
    }
    assert!(authorize_discovered("vfs.beginWrite", &json!({"operations":["read"]})).is_err());
    assert!(authorize_discovered("vfs.openRead", &json!({"operations":[]})).is_err());
    assert!(authorize_discovered("vfs.openRead", &json!({"operations":["read"]})).is_ok());
    assert!(authorize_discovered("vfs.openRead", &Value::Null).is_err());
    forget(plugin);
}

#[test]
fn streaming_methods_map_to_their_capability() {
    assert_eq!(
        FsOperation::from_method("vfs.readStream"),
        Some(FsOperation::Read)
    );
    assert_eq!(
        FsOperation::from_method("vfs.commit"),
        Some(FsOperation::Write)
    );
    assert_eq!(FsOperation::from_method("fs.stat"), None);
}

#[test]
fn a_new_handshake_replaces_capabilities_after_plugin_restart() {
    let plugin = "org.example.restarted";
    register(
        plugin,
        &json!({"vfs":{
            "discovery":true,
            "operations":["read","stat"],
            "preflightOps":["rename"],
            "providers":[{"id":"drive","operations":["read"],"preflightOps":["rename"]}]
        }}),
    );
    assert!(authorize_method(plugin, "vfs.openRead").is_ok());
    assert!(get(plugin).unwrap().instances.contains_key("drive"));

    // The restarted process performs plugin.initialize again. Its new
    // declaration replaces the previous process's cached capabilities.
    let provider = register(
        plugin,
        &json!({"vfs":{
            "discovery":true,
            "operations":["stat"],
            "preflightOps":[],
            "providers":[{"id":"git","operations":["stat"],"preflightOps":[]}]
        }}),
    );

    assert!(provider.reported);
    assert!(!provider.supports(FsOperation::Read));
    assert!(provider.supports(FsOperation::Stat));
    assert!(provider.preflight_ops.is_empty());
    assert_eq!(
        provider
            .instances
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["git"]
    );
    assert!(authorize_method(plugin, "vfs.openRead").is_err());
    assert!(authorize_method(plugin, "vfs.stat").is_ok());
    forget(plugin);
}
