use super::execute::native_action_authorized;
use super::*;

fn ctx(plugin_id: &str, kind: &str, mount: &str, saved: bool, autostart: bool) -> Context {
    let plugin_view = mount.starts_with("fiex-plugin-view://");
    Context {
        info: ConnectionInfo {
            id: "c1".into(),
            name: "Connection".into(),
            kind: kind.into(),
            plugin_id: plugin_id.into(),
            mount_path: mount.into(),
            saved,
            autostart,
            can_read: true,
            can_write: false,
        },
        can_sync: !plugin_view && mount.starts_with("fiex://mount/"),
    }
}

fn ids(items: Vec<Value>) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn vfs_connections_offer_an_immediate_sync() {
    let menu = ctx(
        "org.fiex.openlist",
        "org.fiex.openlist",
        "fiex://mount/m/",
        true,
        false,
    );
    assert_eq!(
        ids(builtin_items("builtin:connection-sync", &menu).unwrap()),
        ["connection.syncNow"]
    );
    assert!(
        ids(builtin_items("builtin:connection-session", &menu).unwrap())
            .contains(&"connection.saveSession".into())
    );
}

#[test]
fn terminal_and_plugin_views_do_not_offer_sync() {
    for menu in [
        ctx(
            "org.example.other",
            "other",
            "fiex-plugin-view://c1",
            true,
            false,
        ),
        ctx(
            "org.example.view",
            "ssh",
            "fiex-plugin-view://c1",
            true,
            false,
        ),
    ] {
        assert!(
            builtin_items("builtin:connection-sync", &menu)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn unsaved_sessions_can_be_saved_and_autostart_toggles() {
    let menu = ctx(
        "org.fiex.openlist",
        "org.fiex.openlist",
        "fiex://mount/m/",
        false,
        true,
    );
    let session = ids(builtin_items("builtin:connection-session", &menu).unwrap());
    assert_eq!(
        session,
        ["connection.saveSession", "connection.toggleAutostart"]
    );
    let close = builtin_items("builtin:connection-close", &menu).unwrap();
    assert_eq!(close[0]["placement"], "tail");
    assert_eq!(close[0]["danger"], true);
}

#[test]
fn connection_matches_are_core_owned_and_storage_sensitive() {
    use crate::plugins::events::matching;
    let connection = ctx(
        "org.fiex.drive",
        "drive",
        "fiex-plugin-view://c1",
        true,
        false,
    );
    let metadata = connection.matcher_resource();
    assert_eq!(metadata.metadata.as_ref().unwrap().kind, "connection");
    assert_eq!(
        metadata.view.as_ref().unwrap().provider_id,
        "plugin:org.fiex.drive:drive"
    );
    let matched = json!({"connectionContextMenus":{"match":{
        "kinds":["connection"],"view":{"providerIds":["plugin:org.fiex.drive:drive"]}
    }}});
    assert!(matching::matches(
        "connectionContextMenu.resolve",
        &matched,
        std::slice::from_ref(&metadata),
        &connection.info.mount_path
    ));
    let denied = json!({"connectionContextMenus":{"match":{
        "kinds":["connection"],"storage":{"types":["vfs"]}
    }}});
    assert!(!matching::matches(
        "connectionContextMenu.resolve",
        &denied,
        &[metadata],
        &connection.info.mount_path
    ));
}

#[test]
fn flat_plugin_items_are_validated_before_rendering() {
    let good = json!({"items":[{"id":"openlist.sync","kind":"action","title":"Sync"}]});
    assert_eq!(valid_items(&good).unwrap().len(), 1);
    for bad in [
        json!({"items":[{"id":"x","kind":"submenu","title":"X"}]}),
        json!({"items":[{"id":"x","kind":"action","title":"X","parentId":"p"}]}),
        json!({"items":[{"id":"x","kind":"action","title":"X"},{"id":"x","kind":"action","title":"Y"}]}),
    ] {
        assert!(valid_items(&bad).is_err(), "{bad}");
    }
}

#[test]
fn plugin_connection_menu_result_cannot_impersonate_a_core_action() {
    let native_close = MenuExecuteResult::native_action("connection.close");
    let native_edit = MenuExecuteResult::native_action("connection.edit");
    let core_action = MenuExecuteResult::core_action("open", Vec::new(), "", "");
    let handled = MenuExecuteResult::handled();
    assert!(native_action_authorized(false, "connection.close", &native_close).is_err());
    assert!(native_action_authorized(false, "connection.close", &core_action).is_err());
    assert!(native_action_authorized(true, "connection.close", &native_close).unwrap());
    assert!(native_action_authorized(true, "connection.close", &native_edit).is_err());
    assert!(native_action_authorized(true, "connection.close", &handled).is_err());
    assert!(!native_action_authorized(false, "connection.close", &handled).unwrap());
}
