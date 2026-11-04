use super::*;

#[test]
fn router_resolves_builtin_selection_and_execute_uses_its_snapshot() {
    let (_temp, directory, paths) = temp_selection();
    let many = super::query(&directory, &paths).unwrap();
    assert!(many
        .iter()
        .any(|menu| menu["providerId"] == "builtin:batch-rename"));
    assert!(!many
        .iter()
        .any(|menu| menu["providerId"] == "builtin:rename"));
    let menu = action(&many, "batchRename");
    let session = menu["menuSessionId"].as_str().unwrap();
    let result = super::execute(session, "batchRename").unwrap();
    assert_eq!(result["kind"], "coreAction");
    assert_eq!(result["paths"], json!(paths));
    assert!(super::execute(session, "batchRename").is_err());
    let single = super::query(&directory, &paths[..1]).unwrap();
    assert!(single
        .iter()
        .any(|menu| menu["providerId"] == "builtin:rename"));
    assert!(!single
        .iter()
        .any(|menu| menu["providerId"] == "builtin:batch-rename"));
}
#[test]
fn sessions_cannot_invoke_another_provider_or_selection() {
    let (_temp, directory, paths) = temp_selection();
    let first = super::query(&directory, &paths[..1]).unwrap();
    let menu = action(&first, "copy");
    assert!(super::execute(menu["menuSessionId"].as_str().unwrap(), "trash").is_err());
    let second = super::query(&directory, &paths[1..]).unwrap();
    let menu = action(&second, "copy");
    let result = super::execute(menu["menuSessionId"].as_str().unwrap(), "copy").unwrap();
    assert_eq!(result["paths"], json!(&paths[1..]));
}
#[test]
fn expired_sessions_and_disabled_actions_are_rejected() {
    let (_temp, directory, paths) = temp_selection();
    let menus = super::query(&directory, &paths[..1]).unwrap();
    let session_id = action(&menus, "copy")["menuSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    super::sessions()
        .lock()
        .unwrap()
        .get_mut(&session_id)
        .unwrap()
        .created = std::time::Instant::now() - std::time::Duration::from_secs(91);
    assert!(super::execute(&session_id, "copy").is_err());
    let ctx = super::builtin::Context::for_test(&directory, paths, vec![]);
    assert!(super::builtin::execute("builtin:rename", "rename", &ctx).is_err());
}
#[test]
fn trash_and_permanent_delete_are_independent_core_actions() {
    let ctx = super::builtin::Context::for_test_caps(
        "/tmp",
        vec!["/tmp/item".into()],
        vec![],
        crate::plugins::resource::file_capabilities::FileCapabilities {
            trash: true,
            remove: true,
            ..Default::default()
        },
    );
    let trash = super::builtin::resolve("builtin:move-to-trash", &ctx).unwrap()["items"]
        .as_array()
        .unwrap()[0]
        .clone();
    let remove = super::builtin::resolve("builtin:delete-permanently", &ctx).unwrap()["items"]
        .as_array()
        .unwrap()[0]
        .clone();
    assert_eq!(trash["id"], "trash");
    assert_eq!(remove["id"], "delete");
    assert_eq!(trash["placement"], "tail");
    assert_eq!(remove["placement"], "tail");
    let registered = super::builtin::providers(&ctx)
        .into_iter()
        .map(|provider| provider.id)
        .collect::<Vec<_>>();
    assert!(registered.iter().any(|id| id == "builtin:move-to-trash"));
    assert!(registered
        .iter()
        .any(|id| id == "builtin:delete-permanently"));
    assert!(!super::builtin::providers(&ctx)
        .iter()
        .any(|provider| provider.id == "builtin:pin"));
}

#[test]
fn recent_location_still_offers_open_without_a_filesystem_directory() {
    let (_temp, _, paths) = temp_selection();
    let menus = super::query("recent://", &paths[..1]).unwrap();
    let menu = action(&menus, "open");
    assert_eq!(
        super::execute(menu["menuSessionId"].as_str().unwrap(), "open").unwrap()["path"],
        paths[0]
    );
}

#[test]
fn core_and_plugin_scopes_keep_their_owner() {
    let (_temp, directory, paths) = temp_selection();
    let core = super::query_core(&directory, &paths[..1], None).unwrap();
    assert!(!core.is_empty());
    assert!(core.iter().all(|menu| menu["kind"] == "builtin"));
    let plugins = super::query_plugins(&directory, &paths[..1], None).unwrap();
    assert!(plugins.iter().all(|menu| menu["kind"] == "plugin"));
    // The one-batch contract stays available for callers that still need it.
    let all = super::query(&directory, &paths[..1]).unwrap();
    assert!(all.len() >= core.len());
}
