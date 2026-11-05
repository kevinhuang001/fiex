use super::*;

#[test]
fn merged_providers_run_concurrently_but_collect_in_priority_order() {
    use std::sync::{Arc, Barrier};
    let barrier = Arc::new(Barrier::new(2));
    let providers = vec!["first", "second"]
        .into_iter()
        .map(|id| ProviderCandidate {
            resolver: Resolver::builtin(id, 100),
            implementation: Arc::new(id.to_owned()),
        })
        .collect();
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "info.resolve",
        json!({}),
        &[],
        None,
    );
    let results = EventRouter::resolve_merge_parallel(
        "info.resolve",
        providers,
        &invocation,
        |_, value, _| {
            barrier.wait();
            Ok(value.clone())
        },
        |_, value| Ok(value),
        |outcomes| {
            outcomes
                .into_iter()
                .map(|(_, value)| value.unwrap())
                .collect::<Vec<_>>()
        },
    );
    assert_eq!(results, vec!["first", "second"]);
}

#[test]
fn preview_skips_invalid_provider_and_stops_after_success() {
    use std::sync::{Arc, Mutex};

    let providers = vec!["invalid", "working", "unused"]
        .into_iter()
        .map(|id| ProviderCandidate {
            resolver: Resolver::builtin(id, 100),
            implementation: Arc::new(id.to_owned()),
        })
        .collect();
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "preview.open",
        json!({}),
        &[],
        None,
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    let result = EventRouter::resolve_shared(
        "preview.open",
        providers,
        &invocation,
        |provider, _, _| {
            calls.lock().unwrap().push(provider.id.clone());
            Ok(if provider.id == "invalid" {
                json!({"kind":"external-url"})
            } else {
                json!({"kind":"text","text":"ok"})
            })
        },
        |_, value| {
            if matches!(
                value["kind"].as_str(),
                Some("text" | "image" | "media" | "pdf" | "fallback")
            ) {
                Ok(value)
            } else {
                Err("unsupported preview kind".into())
            }
        },
        |_| true,
        |outcomes| {
            outcomes
                .into_iter()
                .find_map(|(_, result)| result.ok())
                .unwrap()
        },
    );
    assert_eq!(result["text"], "ok");
    assert_eq!(*calls.lock().unwrap(), vec!["invalid", "working"]);
}

#[test]
fn resource_identity_is_not_an_operation_grant() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("selected.txt");
    std::fs::write(&file, b"abc").unwrap();
    let resource = crate::plugins::resource::resource_context::new_resource_snapshot(
        file.to_str().unwrap(),
        &file,
    );
    let value = json!(resource.project("plugin", "session", true, false));
    let mut operation = crate::plugins::resource::fs_access::Scope::selected(
        dir.path(),
        std::slice::from_ref(&file),
    )
    .unwrap();
    let other = operation.clone();
    operation.bind_resource(&value, Some(&file)).unwrap();
    assert!(operation.resolve_reference(&value["ref"]).is_ok());
    assert!(other.resolve_reference(&value["ref"]).is_err());
    operation.restrict_to_metadata();
    for method in [
        "host.fs.openRead",
        "host.fs.beginWrite",
        "host.process.run",
        "host.process.run",
        "host.vfs.openView",
    ] {
        assert!(operation.clone().authorize_method(method).is_err());
    }
    assert!(operation.authorize_method("host.fs.stat").is_ok());
    assert!(operation.authorize_method("fs.stat").is_err());
}

#[test]
fn explicit_open_actions_may_create_results_next_to_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("selected.zip");
    std::fs::write(&file, b"x").unwrap();
    let file = crate::plugins::resource::fs_access::canonical_selection(&file).unwrap();
    let folder = crate::plugins::resource::fs_access::canonical_selection(dir.path()).unwrap();
    let declared = std::collections::BTreeSet::from([
        "files.create".to_string(),
        "folders.create".to_string(),
    ]);
    assert_eq!(
        action_folders("hooks.invoke", None, std::slice::from_ref(&file), &declared).unwrap(),
        vec![folder.clone()]
    );
    let menu = action_folders(
        "contextMenu.execute",
        Some(dir.path().to_str().unwrap()),
        std::slice::from_ref(&file),
        &declared,
    )
    .unwrap();
    assert!(menu.contains(&folder));
    for (method, declared) in [
        (
            "hooks.invoke",
            std::collections::BTreeSet::from(["files.read".to_string()]),
        ),
        ("preview.open", declared.clone()),
        ("contextMenu.resolve", declared.clone()),
    ] {
        assert!(
            action_folders(method, None, std::slice::from_ref(&file), &declared)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn info_rejects_duplicate_sections_and_non_string_rows() {
    assert!(validate_result(
        "info.resolve",
        &json!({"sections":[{"id":"x","title":"X","rows":[{"label":"Size","value":3}]}]})
    )
    .is_err());
    assert!(validate_result(
        "info.resolve",
        &json!({"sections":[{"id":"x","title":"X","rows":[]},{"id":"x","title":"Other","rows":[]}]})
    )
    .is_err());
}

#[test]
fn connection_validation_rejects_untyped_or_legacy_fields() {
    assert!(validate_result("connection.validate", &json!({"valid": true})).is_ok());
    assert!(validate_result("connection.validate", &json!({"valid": "true"})).is_err());
    assert!(validate_result(
        "connection.validate",
        &json!({"valid": true, "credentials": {}, "legacy": true})
    )
    .is_err());
}

#[test]
fn fixed_events_are_core_owned_custom_methods_remain_available() {
    for method in [
        "info.resolve",
        "preview.open",
        "contextMenu.resolve",
        "search.query",
        "toolbar.invoke",
        "hooks.invoke",
        "fs.openRead",
    ] {
        assert!(is_protected_method(method), "{method}");
    }
    for method in [
        "checksum.calculate",
        "git.stage",
        "archive.compress",
        "search.status",
        "search.rebuild",
    ] {
        assert!(!is_protected_method(method), "{method}");
    }
}

#[test]
fn shared_router_merges_native_and_proxy_candidates_and_rejects_method_spoofing() {
    use std::sync::Arc;

    let providers = vec!["native", "ipc-proxy"]
        .into_iter()
        .map(|id| ProviderCandidate {
            resolver: Resolver::builtin(id, 100),
            implementation: Arc::new(id.to_owned()),
        })
        .collect::<Vec<_>>();
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "contextMenu.resolve",
        json!({}),
        &[],
        None,
    );
    let outcomes = EventRouter::resolve_shared(
        "contextMenu.resolve",
        providers.clone(),
        &invocation,
        |provider, _, _| {
            Ok(json!({"items":[{
                "id": provider.id, "kind": "action", "title": provider.name
            }]}))
        },
        |_, value| Ok(value),
        |_| true,
        |results| results,
    );
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(|(_, result)| result.is_ok()));

    let spoof = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "contextMenu.execute",
        json!({}),
        &[],
        None,
    );
    let outcomes = EventRouter::resolve_shared(
        "contextMenu.resolve",
        providers,
        &spoof,
        |_, _, _| Ok(json!({"items":[]})),
        |_, value| Ok(value),
        |_| true,
        |results| results,
    );
    assert!(outcomes.iter().all(|(_, result)| result.is_err()));
}

#[test]
fn typed_event_router_merges_provider_results_without_json_endpoints() {
    use std::sync::Arc;

    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        "info.resolve",
        json!({}),
        &[],
        None,
    );
    let outcomes = EventRouter::resolve_shared(
        "info.resolve",
        vec![
            Resolver::builtin("core.metadata", 100),
            Resolver::builtin("plugin:org.example:metadata", 50),
        ]
        .into_iter()
        .map(|resolver| ProviderCandidate {
            implementation: Arc::new(resolver.id.clone()),
            resolver,
        })
        .collect(),
        &invocation,
        |provider, _, _| Ok(provider.id.clone()),
        |_, result| Ok(result),
        |_| true,
        |results| {
            results
                .into_iter()
                .map(|(_, result)| result.unwrap())
                .collect::<Vec<_>>()
        },
    );

    assert_eq!(
        outcomes,
        vec!["core.metadata", "plugin:org.example:metadata"]
    );
}
