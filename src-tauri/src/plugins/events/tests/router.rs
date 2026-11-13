use super::super::*;

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
        "decorations.resolve",
        json!({}),
        &[],
        None,
    );
    let results = EventRouter::resolve_merge_parallel(
        "decorations.resolve",
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
        "decorations.resolve",
        json!({}),
        &[],
        None,
    );
    let outcomes = EventRouter::resolve_shared(
        "decorations.resolve",
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
